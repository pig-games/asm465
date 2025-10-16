//! Bridge server that lets external tooling talk to the asm465 Bevy/wgpu
//! frontend.
//!
//! The binary fans out commands from a local TCP socket to all connected
//! websocket clients (e.g. the wasm viewer) and ships responses back to the
//! originator. This mirrors the behaviour implemented in the original asm465
//! desktop UI which exposed a JSON service API.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use anyhow::Context;
use asm465::{ServiceRequestPayload, ServiceResponseMessage};
use clap::Parser;
use futures::{SinkExt, StreamExt as FuturesStreamExt};
use log::{error, info, warn};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt as TokioStreamExt;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_util::codec::{Framed, LinesCodec};

#[derive(Parser, Debug)]
#[command(author, version, about = "asm465 bridge server", long_about = None)]
struct Opts {
    /// Host/interface for the TCP command listener (JSON over newline).
    #[arg(long, default_value = "0.0.0.0")]
    tcp_host: String,
    /// TCP port for inbound commands from tooling (defaults to asm465 native).
    #[arg(long, default_value_t = 7465)]
    tcp_port: u16,
    /// Host/interface for the WebSocket broadcast endpoint.
    #[arg(long, default_value = "0.0.0.0")]
    ws_host: String,
    /// WebSocket port for browser clients.
    #[arg(long, default_value_t = 8800)]
    ws_port: u16,
}

/// Shared state tracking all currently-connected websocket clients.
#[derive(Default)]
struct ServerState {
    clients: Mutex<Vec<mpsc::UnboundedSender<String>>>,
}

impl ServerState {
    /// Register a freshly connected client.
    fn add_client(&self, tx: mpsc::UnboundedSender<String>) {
        self.clients.lock().unwrap().push(tx);
    }

    /// Broadcast a message to every client, pruning dropped connections and
    /// returning the number of recipients that successfully consumed the text.
    fn broadcast(&self, msg: &str) -> usize {
        let mut clients = self.clients.lock().unwrap();
        let mut alive = Vec::with_capacity(clients.len());
        for tx in clients.drain(..) {
            if tx.send(msg.to_owned()).is_ok() {
                alive.push(tx);
            }
        }
        let count = alive.len();
        *clients = alive;
        count
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let opts = Opts::parse();
    let state = Arc::new(ServerState::default());

    let tcp_addr = format!("{}:{}", opts.tcp_host, opts.tcp_port);
    let ws_addr = format!("{}:{}", opts.ws_host, opts.ws_port);

    info!("Starting TCP listener on {tcp_addr}");
    info!("Starting WebSocket listener on {ws_addr}");

    let tcp_state = state.clone();
    let tcp_task = tokio::spawn(async move {
        if let Err(err) = run_tcp_listener(&tcp_addr, tcp_state).await {
            error!("TCP listener terminated: {err:?}");
        }
    });

    let ws_state = state.clone();
    let ws_task = tokio::spawn(async move {
        if let Err(err) = run_ws_listener(&ws_addr, ws_state).await {
            error!("WebSocket listener terminated: {err:?}");
        }
    });

    tokio::select! {
        _ = tcp_task => {}
        _ = ws_task => {}
    }

    Ok(())
}

/// Accept newline-delimited JSON commands over TCP, pushing them into the
/// websocket fan-out and replying with a short status payload.
async fn run_tcp_listener(addr: &str, state: Arc<ServerState>) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, peer) = listener.accept().await?;
        let st = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_tcp_connection(stream, peer, st).await {
                error!("TCP connection {peer} closed with error: {err:?}");
            }
        });
    }
}

/// Handle a single TCP client session, streaming commands until EOF.
async fn handle_tcp_connection(
    stream: TcpStream,
    peer: SocketAddr,
    state: Arc<ServerState>,
) -> anyhow::Result<()> {
    info!("TCP client connected: {peer}");
    let framed = Framed::new(stream, LinesCodec::new());
    let (mut writer, mut reader) = framed.split();

    while let Some(line) = TokioStreamExt::next(&mut reader).await {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<ServiceRequestPayload>(&line) {
            Ok(payload) => {
                let command = payload
                    .into_command()
                    .map_err(|err| anyhow::anyhow!("invalid command: {err}"))?;
                let targets = state.broadcast(&line);
                if targets == 0 {
                    warn!("no websocket clients connected; dropping command");
                    let resp =
                        ServiceResponseMessage::error("no websocket clients connected".to_string());
                    let resp_line = serde_json::to_string(&resp)?;
                    writer.send(resp_line).await?;
                } else {
                    info!("forwarded command from {peer} to {targets} clients");
                    let resp = ServiceResponseMessage::ok(format!(
                        "forwarded to {targets} client{}",
                        if targets == 1 { "" } else { "s" }
                    ));
                    let resp_line = serde_json::to_string(&resp)?;
                    writer.send(resp_line).await?;
                    // keep `command` alive until after broadcast for logging
                    drop(command);
                }
            }
            Err(err) => {
                let msg = format!("invalid JSON payload: {err}");
                warn!("{msg}");
                let resp = ServiceResponseMessage::error(msg);
                let resp_line = serde_json::to_string(&resp)?;
                writer.send(resp_line).await?;
            }
        }
    }

    info!("TCP client disconnected: {peer}");
    Ok(())
}

/// Accept websocket clients and register them with the shared [`ServerState`].
async fn run_ws_listener(addr: &str, state: Arc<ServerState>) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, peer) = listener.accept().await?;
        let st = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_ws_connection(stream, peer, st).await {
                error!("websocket connection {peer} error: {err:?}");
            }
        });
    }
}

/// Handle one websocket peer, relaying broadcast messages until it disconnects.
async fn handle_ws_connection(
    stream: TcpStream,
    peer: SocketAddr,
    state: Arc<ServerState>,
) -> anyhow::Result<()> {
    let ws_stream = accept_async(stream)
        .await
        .context("websocket handshake failed")?;
    info!("websocket client connected: {peer}");

    let (ws_writer, mut ws_reader) = ws_stream.split();
    let (tx, rx) = mpsc::unbounded_channel::<String>();
    state.add_client(tx);

    let mut rx_stream = UnboundedReceiverStream::new(rx);
    let write_task = tokio::spawn(async move {
        let mut ws_writer = ws_writer;
        while let Some(msg) = TokioStreamExt::next(&mut rx_stream).await {
            if ws_writer.send(WsMessage::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    while let Some(msg) = FuturesStreamExt::next(&mut ws_reader).await {
        match msg {
            Ok(WsMessage::Text(text)) => info!("response from {peer}: {text}"),
            Ok(WsMessage::Binary(_)) => warn!("ignoring binary websocket message from {peer}"),
            Ok(WsMessage::Ping(_)) | Ok(WsMessage::Pong(_)) => {}
            Ok(WsMessage::Close(_)) | Ok(WsMessage::Frame(_)) => break,
            Err(err) => {
                warn!("websocket read error from {peer}: {err}");
                break;
            }
        }
    }

    write_task.abort();
    info!("websocket client disconnected: {peer}");
    Ok(())
}
