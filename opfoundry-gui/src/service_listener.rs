use bevy::prelude::Resource;
use crossbeam_channel::{Receiver, Sender};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use crate::{ServiceCommand, ServiceRequestPayload, ServiceResponseMessage};

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
        let command = match payload.into_command() {
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
