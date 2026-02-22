use bevy::prelude::Resource;
use crossbeam_channel::{Receiver, Sender};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use crate::{into_service_command, ServiceCommand, ServiceRequestPayload, ServiceResponseMessage};

#[derive(Resource)]
pub(crate) struct ServiceListener {
    pub(crate) receiver: Receiver<ServiceEnvelope>,
}

pub(crate) struct ServiceEnvelope {
    pub(crate) command: ServiceCommand,
    pub(crate) respond_to: Sender<ServiceResponseMessage>,
}

pub(crate) fn start_service_listener(
    host: &str,
    port: u16,
) -> std::io::Result<Receiver<ServiceEnvelope>> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let listener = TcpListener::bind((host, port))?;
    thread::spawn(move || {
        if let Err(err) = run_service_listener(listener, tx) {
            eprintln!("service listener error: {err}");
        }
    });
    Ok(rx)
}

fn run_service_listener(listener: TcpListener, tx: Sender<ServiceEnvelope>) -> std::io::Result<()> {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let tx = tx.clone();
                thread::spawn(move || {
                    if let Err(err) = handle_service_connection(stream, tx) {
                        eprintln!("service client error: {err}");
                    }
                });
            }
            Err(err) => eprintln!("service accept error: {err}"),
        }
    }
    Ok(())
}

fn handle_service_connection(
    stream: TcpStream,
    tx: Sender<ServiceEnvelope>,
) -> std::io::Result<()> {
    let reader_stream = stream.try_clone()?;
    let mut reader = BufReader::new(reader_stream);
    let mut writer = BufWriter::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let payload: ServiceRequestPayload = match serde_json::from_str(trimmed) {
            Ok(payload) => payload,
            Err(err) => {
                write_service_response(
                    &mut writer,
                    ServiceResponseMessage::error(format!("invalid json: {err}")),
                )?;
                continue;
            }
        };
        let command = match into_service_command(payload) {
            Ok(command) => command,
            Err(err) => {
                write_service_response(&mut writer, ServiceResponseMessage::error(err))?;
                continue;
            }
        };
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        let envelope = ServiceEnvelope {
            command,
            respond_to: resp_tx,
        };
        if tx.send(envelope).is_err() {
            write_service_response(
                &mut writer,
                ServiceResponseMessage::error("service unavailable"),
            )?;
            break;
        }
        match resp_rx.recv() {
            Ok(response) => {
                write_service_response(&mut writer, response)?;
            }
            Err(_) => {
                write_service_response(
                    &mut writer,
                    ServiceResponseMessage::error("service unavailable"),
                )?;
                break;
            }
        }
    }

    writer.flush()?;
    Ok(())
}

fn write_service_response<W: Write>(
    writer: &mut W,
    response: ServiceResponseMessage,
) -> std::io::Result<()> {
    serde_json::to_writer(&mut *writer, &response).map_err(std::io::Error::other)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ServiceStatus;
    use std::io::Read;
    use std::net::SocketAddr;

    fn start_one_shot_server(tx: Sender<ServiceEnvelope>) -> (SocketAddr, thread::JoinHandle<()>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind test listener");
        let addr = listener.local_addr().expect("listener address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept connection");
            handle_service_connection(stream, tx).expect("handle connection");
        });
        (addr, handle)
    }

    fn read_single_line(mut stream: TcpStream) -> String {
        let mut out = String::new();
        let mut buf = [0u8; 4096];
        let read = stream.read(&mut buf).expect("read response");
        out.push_str(&String::from_utf8_lossy(&buf[..read]));
        out
    }

    #[test]
    fn responds_with_error_on_invalid_json() {
        let (tx, _rx) = crossbeam_channel::unbounded::<ServiceEnvelope>();
        let (addr, handle) = start_one_shot_server(tx);

        let mut client = TcpStream::connect(addr).expect("connect client");
        client
            .write_all(b"{not json}\n")
            .expect("write invalid json");
        client.flush().expect("flush invalid json");
        let raw = read_single_line(client);

        let response: ServiceResponseMessage =
            serde_json::from_str(raw.trim()).expect("parse response json");
        assert!(matches!(response.status, ServiceStatus::Error));
        assert!(response.message.contains("invalid json"));

        handle.join().expect("server thread join");
    }

    #[test]
    fn responds_service_unavailable_when_receiver_is_dropped() {
        let (tx, rx) = crossbeam_channel::unbounded::<ServiceEnvelope>();
        drop(rx);
        let (addr, handle) = start_one_shot_server(tx);

        let mut client = TcpStream::connect(addr).expect("connect client");
        client
            .write_all(br#"{"cmd":"read_mem","address":0,"length":1}"#)
            .expect("write payload");
        client.write_all(b"\n").expect("write newline");
        client.flush().expect("flush payload");
        let raw = read_single_line(client);

        let response: ServiceResponseMessage =
            serde_json::from_str(raw.trim()).expect("parse response json");
        assert!(matches!(response.status, ServiceStatus::Error));
        assert!(response.message.contains("service unavailable"));

        handle.join().expect("server thread join");
    }
}
