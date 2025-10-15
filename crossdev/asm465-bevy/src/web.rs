#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::App;
use futures::{channel::mpsc, SinkExt, StreamExt};
use gloo_file::callbacks::read_as_bytes;
use gloo_file::File;
use gloo_net::websocket::futures::WebSocket;
use gloo_net::websocket::Message;
use js_sys::{Array, Function, Promise, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{HtmlButtonElement, HtmlElement, HtmlInputElement, UrlSearchParams};

use crate::{
    run_app, AppConfig, ProgramSource, ServiceCommand, ServiceRequestPayload,
    ServiceResponseMessage,
};

const DEFAULT_MAX_CYCLES: u64 = 5_000_000;
const DEFAULT_WS_URL: &str = "ws://127.0.0.1:8800";
const CANVAS_ID: &str = "#asm465-canvas";

thread_local! {
    static FILE_QUEUE: RefCell<Vec<(Vec<u8>, Option<String>)>> = RefCell::new(Vec::new());
    static FILE_INPUT: RefCell<Option<HtmlInputElement>> = RefCell::new(None);
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
    if try_show_open_file_picker() {
        return;
    }

    FILE_INPUT.with(|slot| {
        let binding = slot.borrow();
        let Some(input) = binding.as_ref() else {
            log::error!("request_file_dialog: file input not initialised");
            return;
        };

        let element: &HtmlElement = input.unchecked_ref::<HtmlElement>();
        element.click();
    });
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
    let input_el = document
        .get_element_by_id("prg-input")
        .ok_or_else(|| JsValue::from_str("missing #prg-input"))?;
    let button_el = document
        .get_element_by_id("load-prg-btn")
        .ok_or_else(|| JsValue::from_str("missing #load-prg-btn"))?;
    let input: HtmlInputElement = input_el.dyn_into()?;
    let button: HtmlButtonElement = button_el.dyn_into()?;

    FILE_INPUT.with(|slot| {
        *slot.borrow_mut() = Some(input.clone());
    });

    let trigger_input = input.clone();
    let click_closure = Closure::wrap(Box::new(move || {
        let element: &HtmlElement = trigger_input.unchecked_ref::<HtmlElement>();
        element.click();
    }) as Box<dyn FnMut()>);
    button.add_event_listener_with_callback("click", click_closure.as_ref().unchecked_ref())?;
    click_closure.forget();

    let input_for_change = input.clone();
    let change_closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
        if let Some(files) = input_for_change.files() {
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
        input_for_change.set_value("");
    }) as Box<dyn FnMut(_)>);
    input.add_event_listener_with_callback("change", change_closure.as_ref().unchecked_ref())?;
    change_closure.forget();

    Ok(())
}

fn try_show_open_file_picker() -> bool {
    let window = match web_sys::window() {
        Some(window) => window,
        None => return false,
    };

    let show_picker = match Reflect::get(&window, &JsValue::from_str("showOpenFilePicker")) {
        Ok(value) => value,
        Err(_) => return false,
    };

    let function = match show_picker.dyn_into::<Function>() {
        Ok(func) => func,
        Err(_) => return false,
    };

    let promise_value = match function.call0(&JsValue::from(window.clone())) {
        Ok(value) => value,
        Err(err) => {
            log::error!("showOpenFilePicker call failed: {err:?}");
            return false;
        }
    };

    let promise = match promise_value.dyn_into::<Promise>() {
        Ok(promise) => promise,
        Err(_) => return false,
    };

    spawn_local(async move {
        match JsFuture::from(promise).await {
            Ok(handles_value) => {
                let handles = Array::from(&handles_value);
                for handle_value in handles.iter() {
                    let get_file = match Reflect::get(&handle_value, &JsValue::from_str("getFile"))
                    {
                        Ok(value) => value,
                        Err(err) => {
                            log::error!("getFile missing on handle: {err:?}");
                            continue;
                        }
                    };
                    let get_file_fn = match get_file.dyn_into::<Function>() {
                        Ok(func) => func,
                        Err(_) => continue,
                    };

                    let file_promise_value = match get_file_fn.call0(&handle_value) {
                        Ok(value) => value,
                        Err(err) => {
                            log::error!("getFile call failed: {err:?}");
                            continue;
                        }
                    };

                    let file_promise = match file_promise_value.dyn_into::<Promise>() {
                        Ok(promise) => promise,
                        Err(_) => continue,
                    };

                    let file_value = match JsFuture::from(file_promise).await {
                        Ok(value) => value,
                        Err(err) => {
                            log::error!("getFile promise rejected: {err:?}");
                            continue;
                        }
                    };

                    let web_file = match file_value.dyn_into::<web_sys::File>() {
                        Ok(file) => file,
                        Err(_) => continue,
                    };

                    let display_name = if web_file.name().is_empty() {
                        None
                    } else {
                        Some(web_file.name())
                    };

                    let buffer_value = match JsFuture::from(web_file.array_buffer()).await {
                        Ok(buffer) => buffer,
                        Err(err) => {
                            log::error!("array_buffer failed: {err:?}");
                            continue;
                        }
                    };

                    let array = Uint8Array::new(&buffer_value);
                    let mut bytes = vec![0u8; array.length() as usize];
                    array.copy_to(&mut bytes);

                    FILE_QUEUE.with(|queue| queue.borrow_mut().push((bytes, display_name)));
                }
            }
            Err(err) => {
                log::error!("showOpenFilePicker promise rejected: {err:?}");
            }
        }
    });

    true
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
