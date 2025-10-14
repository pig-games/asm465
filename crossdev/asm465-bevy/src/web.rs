#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::App;
use futures::{channel::mpsc, SinkExt, StreamExt};
use gloo_file::callbacks::read_as_bytes;
use gloo_file::File;
use gloo_net::websocket::futures::WebSocket;
use gloo_net::websocket::Message;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, UrlSearchParams};

use crate::{
    run_app, AppConfig, ProgramSource, ServiceCommand, ServiceRequestPayload,
    ServiceResponseMessage,
};

const DEFAULT_MAX_CYCLES: u64 = 5_000_000;
const DEFAULT_WS_URL: &str = "ws://127.0.0.1:8800";
const CANVAS_ID: &str = "#asm465-canvas";

thread_local! {
    static FILE_QUEUE: RefCell<Vec<(Vec<u8>, Option<String>)>> = RefCell::new(Vec::new());
}

fn extend_status(status: &mut Option<String>, message: String) {
    match status.take() {
        Some(existing) => *status = Some(format!("{existing} | {message}")),
        None => *status = Some(message),
    }
}

pub struct WebSocketBridge {
    pending: Rc<RefCell<Vec<ServiceCommand>>>,
    response_tx: mpsc::UnboundedSender<ServiceResponseMessage>,
}

impl WebSocketBridge {
    fn connect(url: &str) -> Result<Self, JsValue> {
        let ws = WebSocket::open(url).map_err(|err| JsValue::from_str(&format!("{err}")))?;
        let (mut write, mut read) = ws.split();

        let pending = Rc::new(RefCell::new(Vec::new()));
        let pending_reader = pending.clone();

        spawn_local(async move {
            while let Some(message) = read.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        match serde_json::from_str::<ServiceRequestPayload>(&text) {
                            Ok(payload) => match payload.into_command() {
                                Ok(command) => pending_reader.borrow_mut().push(command),
                                Err(err) => log::error!("invalid service command: {err}"),
                            },
                            Err(err) => log::error!("failed to parse command: {err}"),
                        }
                    }
                    Ok(Message::Bytes(bytes)) => {
                        log::warn!("ignoring binary websocket message (len={})", bytes.len());
                    }
                    Err(err) => {
                        log::error!("websocket receive error: {err:?}");
                        break;
                    }
                }
            }
        });

        let (response_tx, mut response_rx) = mpsc::unbounded::<ServiceResponseMessage>();
        spawn_local(async move {
            while let Some(response) = response_rx.next().await {
                match serde_json::to_string(&response) {
                    Ok(json) => {
                        if let Err(err) = write.send(Message::Text(json)).await {
                            log::error!("websocket send error: {err:?}");
                            break;
                        }
                    }
                    Err(err) => log::error!("serialize response error: {err}"),
                }
            }
        });

        Ok(Self {
            pending,
            response_tx,
        })
    }

    pub fn drain_commands(&self) -> Vec<ServiceCommand> {
        self.pending.borrow_mut().drain(..).collect()
    }

    pub fn send_response(&self, response: ServiceResponseMessage) {
        if let Err(err) = self.response_tx.unbounded_send(response) {
            log::error!("failed to send response to bridge: {err}");
        }
    }
}

pub fn configure_app(app: &mut App) -> Option<String> {
    let mut status = None;

    if let Err(err) = setup_file_loader() {
        let message = format!("File picker unavailable: {err:?}");
        log::error!("{message}");
        extend_status(&mut status, message);
    }

    match resolve_ws_url() {
        Some(url) => match WebSocketBridge::connect(&url) {
            Ok(service) => {
                let message = format!("Bridge connected ({url})");
                log::info!("{message}");
                app.insert_non_send_resource(service);
                extend_status(&mut status, message);
            }
            Err(err) => {
                let message = format!("Bridge connection failed: {err:?}");
                log::error!("{message}");
                extend_status(&mut status, message);
            }
        },
        None => {
            let message = "Bridge disabled".to_string();
            log::warn!("{message}");
            extend_status(&mut status, message);
        }
    }

    status
}

pub fn request_file_dialog() {
    let Some(window) = web_sys::window() else {
        log::error!("request_file_dialog: missing window");
        return;
    };
    let Some(document) = window.document() else {
        log::error!("request_file_dialog: missing document");
        return;
    };

    let Ok(input_el) = document.create_element("input") else {
        log::error!("request_file_dialog: failed to create input element");
        return;
    };

    let Ok(input) = input_el.dyn_into::<HtmlInputElement>() else {
        log::error!("request_file_dialog: failed to cast element to HtmlInputElement");
        return;
    };

    input.set_type("file");
    input.set_accept(".prg,.bin,*/*");
    let style = input.style();
        let _ = style.set_property("display", "none");

    if let Some(body) = document.body() {
        let _ = body.append_child(&input);
    }

    let input_ref = Rc::new(input);
    let change_input = input_ref.clone();
    let change_closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
        if let Some(files) = change_input.files() {
            for idx in 0..files.length() {
                if let Some(file) = files.get(idx) {
                    let name = file.name();
                    let file = File::from(file);
                    let reader_state: Rc<RefCell<Option<_>>> = Rc::new(RefCell::new(None));
                    let reader_state_clone = reader_state.clone();
                    let name_clone = if name.is_empty() {
                        None
                    } else {
                        Some(name.clone())
                    };
                    let reader = read_as_bytes(&file, move |result| {
                        match result {
                            Ok(bytes) => {
                                FILE_QUEUE.with(|queue| {
                                    queue.borrow_mut().push((bytes, name_clone.clone()));
                                });
                            }
                            Err(err) => {
                                log::error!("failed to read file: {err}");
                            }
                        }
                        reader_state_clone.borrow_mut().take();
                    });
                    *reader_state.borrow_mut() = Some(reader);
                }
            }
        }

        if let Some(parent) = change_input.parent_node() {
            let node: &web_sys::Node = change_input.as_ref().unchecked_ref();
            let _ = parent.remove_child(node);
        }
    }) as Box<dyn FnMut(_)>);

    if input_ref
        .add_event_listener_with_callback("change", change_closure.as_ref().unchecked_ref())
        .is_err()
    {
        log::warn!("request_file_dialog: failed to register change listener");
    }
    change_closure.forget();

    input_ref.click()
}

pub fn drain_pending_files() -> Vec<(Vec<u8>, Option<String>)> {
    FILE_QUEUE.with(|queue| queue.borrow_mut().drain(..).collect())
}

pub fn start_web_app() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    run_app(AppConfig {
        startup: None,
        default_max_cycles: DEFAULT_MAX_CYCLES,
        #[cfg(feature = "native-service")]
        service: None,
    });

    Ok(())
}

pub fn canvas_id() -> &'static str {
    CANVAS_ID
}

fn setup_file_loader() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("missing window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("missing document"))?;

    document
        .body()
        .ok_or_else(|| JsValue::from_str("missing document body"))?;

    Ok(())
}

fn resolve_ws_url() -> Option<String> {
    let window = web_sys::window()?;
    let location = window.location();
    if let Ok(search) = location.search() {
        if !search.is_empty() {
            if let Ok(params) = UrlSearchParams::new_with_str(&search) {
                if let Some(url) = params.get("ws") {
                    return Some(url);
                }
            }
        }
    }
    Some(DEFAULT_WS_URL.to_string())
}
