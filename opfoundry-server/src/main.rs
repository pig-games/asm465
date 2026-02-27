//! Bridge server that lets external tooling talk to the opFoundry Bevy/wgpu
//! frontend.
//!
//! The binary fans out commands from a local TCP socket to all connected
//! websocket clients (e.g. the wasm viewer) and ships responses back to the
//! originator. This mirrors the behaviour implemented in the original opFoundry
//! desktop UI which exposed a JSON service API.

#![allow(clippy::items_after_test_module)]

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

use anyhow::Context;
use clap::Parser;
use futures::{SinkExt, StreamExt as FuturesStreamExt};
use log::{error, info, warn};
use opfoundry_api::{ServiceRequestPayload, ServiceResponseMessage};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{timeout, Duration};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt as TokioStreamExt;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_util::codec::{Framed, LinesCodec};

#[derive(Parser, Debug)]
#[command(author, version, about = "opFoundry bridge server", long_about = None)]
struct Opts {
    /// Host/interface for the TCP command listener (JSON over newline).
    #[arg(long, default_value = "127.0.0.1")]
    tcp_host: String,
    /// TCP port for inbound commands from tooling (defaults to opFoundry native).
    #[arg(long, default_value_t = 7465)]
    tcp_port: u16,
    /// Host/interface for the WebSocket broadcast endpoint.
    #[arg(long, default_value = "127.0.0.1")]
    ws_host: String,
    /// WebSocket port for browser clients.
    #[arg(long, default_value_t = 8800)]
    ws_port: u16,
    /// Optional directory restriction for `run_prg` file paths.
    #[arg(long)]
    allowed_dir: Option<PathBuf>,
}

const RESPONSE_TIMEOUT_SECS: u64 = 30;

/// Shared state tracking all currently-connected websocket clients.
#[derive(Default)]
struct ServerState {
    clients: Mutex<Vec<mpsc::UnboundedSender<String>>>,
    pending: Mutex<PendingResponses>,
    next_id: AtomicU64,
}

#[derive(Default)]
struct PendingResponses {
    by_id: HashMap<String, oneshot::Sender<ServiceResponseMessage>>,
    fifo: VecDeque<oneshot::Sender<ServiceResponseMessage>>,
}

impl ServerState {
    fn lock_or_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        mutex.lock().unwrap_or_else(|err| err.into_inner())
    }

    /// Register a freshly connected client.
    fn add_client(&self, tx: mpsc::UnboundedSender<String>) {
        Self::lock_or_recover(&self.clients).push(tx);
    }

    /// Broadcast a message to every client, pruning dropped connections and
    /// returning the number of recipients that successfully consumed the text.
    fn broadcast(&self, msg: &str) -> usize {
        let mut clients = Self::lock_or_recover(&self.clients);
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

    fn next_request_id(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        format!("{id:016x}")
    }

    fn register_pending(&self, id: Option<String>) -> oneshot::Receiver<ServiceResponseMessage> {
        Self::lock_or_recover(&self.pending).register(id)
    }

    fn fulfill_pending(&self, response: ServiceResponseMessage) -> bool {
        Self::lock_or_recover(&self.pending).fulfill(response)
    }

    fn cancel_pending(&self, id: Option<&str>) -> bool {
        Self::lock_or_recover(&self.pending).cancel(id)
    }
}

impl PendingResponses {
    fn register(&mut self, id: Option<String>) -> oneshot::Receiver<ServiceResponseMessage> {
        let (tx, rx) = oneshot::channel();
        if let Some(id) = id {
            self.by_id.insert(id, tx);
        } else {
            self.fifo.push_back(tx);
        }
        rx
    }

    fn fulfill(&mut self, response: ServiceResponseMessage) -> bool {
        if let Some(ref id) = response.bridge_id {
            if let Some(tx) = self.by_id.remove(id) {
                let _ = tx.send(response);
                return true;
            }
        }
        if let Some(tx) = self.fifo.pop_front() {
            let _ = tx.send(response);
            return true;
        }
        false
    }

    fn cancel(&mut self, id: Option<&str>) -> bool {
        if let Some(id) = id {
            self.by_id.remove(id).is_some()
        } else {
            self.fifo.pop_front().is_some()
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let opts = Opts::parse();
    let state = Arc::new(ServerState::default());
    let allowed_dir = opts.allowed_dir;

    let tcp_addr = format!("{}:{}", opts.tcp_host, opts.tcp_port);
    let ws_addr = format!("{}:{}", opts.ws_host, opts.ws_port);

    if allowed_dir.is_none() {
        warn!("No --allowed-dir set; run_prg accepts any readable path");
    }

    info!("Starting TCP listener on {tcp_addr}");
    info!("Starting WebSocket listener on {ws_addr}");

    let tcp_state = state.clone();
    let tcp_task = tokio::spawn(async move {
        if let Err(err) = run_tcp_listener(&tcp_addr, tcp_state, allowed_dir).await {
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
async fn run_tcp_listener(
    addr: &str,
    state: Arc<ServerState>,
    allowed_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, peer) = listener.accept().await?;
        let st = state.clone();
        let allowed_dir = allowed_dir.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_tcp_connection(stream, peer, st, allowed_dir).await {
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
    allowed_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    info!("TCP client connected: {peer}");
    let framed = Framed::new(stream, LinesCodec::new());
    let (mut writer, mut reader) = framed.split();

    while let Some(line) = TokioStreamExt::next(&mut reader).await {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let mut payload_value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(val) => val,
            Err(err) => {
                let msg = format!("invalid JSON payload: {err}");
                warn!("{msg}");
                let resp = ServiceResponseMessage::error(msg);
                let resp_line = serde_json::to_string(&resp)?;
                writer.send(resp_line).await?;
                continue;
            }
        };

        let request_id = state.next_request_id();
        if let serde_json::Value::Object(obj) = &mut payload_value {
            obj.insert(
                "bridge_id".to_string(),
                serde_json::Value::String(request_id.clone()),
            );
        } else {
            let resp = ServiceResponseMessage::error("payload must be a JSON object");
            let resp_line = serde_json::to_string(&resp)?;
            writer.send(resp_line).await?;
            continue;
        }

        match serde_json::from_value::<ServiceRequestPayload>(payload_value.clone()) {
            Ok(payload) => {
                if let Err(err) = payload.validate() {
                    let resp = ServiceResponseMessage::error(format!("invalid command: {err}"));
                    let resp_line = serde_json::to_string(&resp)?;
                    writer.send(resp_line).await?;
                    continue;
                }

                if let Some(base) = allowed_dir.as_ref() {
                    if let ServiceRequestPayload::RunPrg { path, .. } = &payload {
                        if !path_within_allowed_dir(path, base)? {
                            let resp = ServiceResponseMessage::error(format!(
                                "run_prg path is outside allowed directory: {}",
                                base.display()
                            ));
                            let resp_line = serde_json::to_string(&resp)?;
                            writer.send(resp_line).await?;
                            continue;
                        }
                    }
                }

                let serialized = serde_json::to_string(&payload_value)?;
                let targets = state.broadcast(&serialized);
                if targets == 0 {
                    warn!("no websocket clients connected; dropping command");
                    let resp =
                        ServiceResponseMessage::error("no websocket clients connected".to_string());
                    let resp_line = serde_json::to_string(&resp)?;
                    writer.send(resp_line).await?;
                } else {
                    let response_rx = state.register_pending(Some(request_id.clone()));
                    info!(
                        "forwarded command from {peer} to {targets} client{} (id={request_id})",
                        if targets == 1 { "" } else { "s" }
                    );
                    match timeout(Duration::from_secs(RESPONSE_TIMEOUT_SECS), response_rx).await {
                        Ok(Ok(response)) => {
                            let resp_line = serde_json::to_string(&response)?;
                            writer.send(resp_line).await?;
                        }
                        Ok(Err(_)) => {
                            let resp = ServiceResponseMessage::error(
                                "bridge connection closed before response".to_string(),
                            );
                            let resp_line = serde_json::to_string(&resp)?;
                            writer.send(resp_line).await?;
                        }
                        Err(_) => {
                            warn!(
                                "timeout waiting for response to request {request_id}; canceling"
                            );
                            state.cancel_pending(Some(&request_id));
                            let resp = ServiceResponseMessage::error(
                                "bridge timeout waiting for response".to_string(),
                            );
                            let resp_line = serde_json::to_string(&resp)?;
                            writer.send(resp_line).await?;
                        }
                    }
                }
            }
            Err(err) => {
                let msg = format!("invalid command payload: {err}");
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

fn path_within_allowed_dir(path: &str, allowed_dir: &Path) -> anyhow::Result<bool> {
    let requested = Path::new(path);
    let resolved = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        std::env::current_dir()?.join(requested)
    };
    let resolved = resolved.canonicalize().with_context(|| {
        format!(
            "unable to resolve requested run_prg path `{}`",
            requested.display()
        )
    })?;
    let allowed = allowed_dir
        .canonicalize()
        .with_context(|| format!("unable to resolve allowed_dir `{}`", allowed_dir.display()))?;
    Ok(resolved.starts_with(allowed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use opfoundry_api::ServiceStatus;
    use std::fs;

    #[test]
    fn allows_paths_under_allowed_dir() {
        let root = std::env::temp_dir().join(format!(
            "opfoundry_server_test_allow_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        ));
        let nested = root.join("programs");
        fs::create_dir_all(&nested).expect("should create test directory");
        let file = nested.join("demo.prg");
        fs::write(&file, [0u8, 1, 2]).expect("should create file");

        let allowed = path_within_allowed_dir(file.to_str().expect("utf8 path"), &root)
            .expect("path check should succeed");
        assert!(allowed);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_paths_outside_allowed_dir() {
        let root = std::env::temp_dir().join(format!(
            "opfoundry_server_test_reject_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        ));
        let outside_root = std::env::temp_dir().join(format!(
            "opfoundry_server_test_outside_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("should create allowed root");
        fs::create_dir_all(&outside_root).expect("should create outside root");
        let file = outside_root.join("demo.prg");
        fs::write(&file, [0u8, 1, 2]).expect("should create outside file");

        let allowed = path_within_allowed_dir(file.to_str().expect("utf8 path"), &root)
            .expect("path check should succeed");
        assert!(!allowed);

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside_root);
    }

    #[test]
    fn pending_responses_match_bridge_id_before_fifo() {
        let mut pending = PendingResponses::default();
        let _fifo_rx = pending.register(None);
        let _id_rx = pending.register(Some("abc123".to_string()));

        let matched = pending.fulfill(ServiceResponseMessage {
            status: ServiceStatus::Ok,
            message: "ok".to_string(),
            data: None,
            bridge_id: Some("abc123".to_string()),
            cycles: None,
        });
        assert!(matched);
        assert!(!pending.by_id.contains_key("abc123"));

        let fifo_matched = pending.fulfill(ServiceResponseMessage {
            status: ServiceStatus::Ok,
            message: "fallback".to_string(),
            data: None,
            bridge_id: None,
            cycles: None,
        });
        assert!(fifo_matched);
    }

    #[test]
    fn broadcast_prunes_disconnected_clients() {
        let state = ServerState::default();
        let (live_tx, mut live_rx) = mpsc::unbounded_channel::<String>();
        let (dead_tx, dead_rx) = mpsc::unbounded_channel::<String>();
        drop(dead_rx);

        state.add_client(live_tx);
        state.add_client(dead_tx);

        let delivered = state.broadcast("hello");
        assert_eq!(delivered, 1);

        let msg = live_rx
            .try_recv()
            .expect("live receiver should get message");
        assert_eq!(msg, "hello");
        assert_eq!(ServerState::lock_or_recover(&state.clients).len(), 1);
    }
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
            Ok(WsMessage::Text(text)) => {
                match serde_json::from_str::<ServiceResponseMessage>(&text) {
                    Ok(response) => {
                        if !state.fulfill_pending(response) {
                            warn!("response from {peer} without pending request: {text}");
                        }
                    }
                    Err(err) => warn!("invalid response from {peer}: {err}"),
                }
            }
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
