#![cfg(target_arch = "wasm32")]
//! WASM-specific plumbing for the Bevy viewer.
//!
//! The WASM build mirrors the native desktop app but swaps the file-picker and
//! bridge integrations for browser-friendly equivalents.  In particular we:
//! - expose [`request_file_dialog`] which uses `rfd::AsyncFileDialog` under the
//!   hood so Safari/Chrome/Firefox all open the picker in response to the egui
//!   button click;
//! - maintain a list of pending service commands gathered from the websocket
//!   bridge so the main Bevy systems can drain them each frame.

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::App;
use futures::{channel::mpsc, SinkExt, StreamExt};
use gloo_net::websocket::futures::WebSocket;
use gloo_net::websocket::Message;
use js_sys::{Array, Function, Promise, Reflect, Uint8Array};
use rfd::AsyncFileDialog;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::UrlSearchParams;

use crate::{
    run_app, AppConfig, ServiceCommand, ServiceRequestPayload, ServiceResponseMessage,
    VirtualResolution,
};

const DEFAULT_MAX_CYCLES: u64 = 5_000_000;
const DEFAULT_WS_PORT: u16 = 8_800;
#[cfg(feature = "dev-loopback")]
const DEV_LOOPBACK_WS_URL: &str = "ws://127.0.0.1:8800";
const CANVAS_ID: &str = "#asm465-canvas";
const DEFAULT_LOOPBACK_HOSTS: &[&str] = &["127.0.0.1", "localhost", "::1"];

thread_local! {
    static FILE_QUEUE: RefCell<Vec<(Vec<u8>, Option<String>)>> = RefCell::new(Vec::new());
}

fn extend_status(status: &mut Option<String>, message: String) {
    match status.take() {
        Some(existing) => *status = Some(format!("{existing} | {message}")),
        None => *status = Some(message),
    }
}

/// Shared state used to bridge websocket commands/responses between the async
/// browser tasks and the Bevy schedule.
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

/// Install browser-specific resources (file picker + websocket bridge) into
/// the Bevy app, returning a status message that can be displayed in the UI.
pub fn configure_app(app: &mut App) -> Option<String> {
    let mut status = None;

    if let Err(err) = setup_file_loader() {
        let message = format!("File picker unavailable: {err:?}");
        log::error!("{message}");
        extend_status(&mut status, message);
    }

    let urls = resolve_ws_urls();
    if urls.is_empty() {
        let message = "Bridge disabled".to_string();
        log::warn!("{message}");
        extend_status(&mut status, message);
        return status;
    }

    let mut last_error: Option<String> = None;
    for url in urls {
        match WebSocketBridge::connect(&url) {
            Ok(service) => {
                let message = format!("Bridge connected ({url})");
                log::info!("{message}");
                app.insert_non_send_resource(service);
                extend_status(&mut status, message);
                return status;
            }
            Err(err) => {
                let message = format!("Bridge connection failed for {url}: {err:?}");
                log::error!("{message}");
                last_error = Some(message);
            }
        }
    }

    if let Some(message) = last_error {
        extend_status(&mut status, message);
    }

    status
}

/// Prompt the user for a PRG via the browser's file picker and enqueue the
/// result for the emulator thread.
pub fn request_file_dialog() {
    if try_show_open_file_picker() {
        return;
    }

    spawn_local(async move {
        if let Some(handle) = AsyncFileDialog::new()
            .add_filter("PRG", &["prg", "bin"])
            .pick_file()
            .await
        {
            let file_name = handle.file_name();
            let bytes = handle.read().await;
            FILE_QUEUE.with(|queue| {
                queue.borrow_mut().push((
                    bytes,
                    if file_name.is_empty() {
                        None
                    } else {
                        Some(file_name)
                    },
                ));
            });
        }
    });
}

/// Drain any files selected since the last frame. Each tuple contains the raw
/// bytes plus an optional display name reported by the host platform.
pub fn drain_pending_files() -> Vec<(Vec<u8>, Option<String>)> {
    FILE_QUEUE.with(|queue| queue.borrow_mut().drain(..).collect())
}

pub fn start_web_app() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    run_app(AppConfig {
        startup: None,
        default_max_cycles: DEFAULT_MAX_CYCLES,
        virtual_resolution: VirtualResolution::default(),
        #[cfg(feature = "native-service")]
        service: None,
    });

    Ok(())
}

pub fn canvas_id() -> &'static str {
    CANVAS_ID
}

fn setup_file_loader() -> Result<(), JsValue> {
    Ok(())
}

/// Attempt to open the modern `showOpenFilePicker` API, returning `true` if a
/// picker was launched (even if no file was ultimately selected).
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

fn resolve_ws_urls() -> Vec<String> {
    let mut urls = Vec::new();
    let window = match web_sys::window() {
        Some(window) => window,
        None => return urls,
    };
    let location = window.location();

    fn push_url(urls: &mut Vec<String>, url: String) {
        if !urls.contains(&url) {
            urls.push(url);
        }
    }

    if let Ok(search) = location.search() {
        if let Some(url) = parse_ws_override(&search) {
            urls.push(url);
            return urls;
        }
    }

    if let Ok(protocol) = location.protocol() {
        let default_port = DEFAULT_WS_PORT.to_string();
        let hostname = location.hostname().ok();
        let port = location.port().ok();
        let hostname = hostname.filter(|value| !value.is_empty());
        let port = port.filter(|value| !value.is_empty());

        if let Some(host) = hostname.as_deref() {
            let is_loopback = DEFAULT_LOOPBACK_HOSTS
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(host));

            if is_loopback {
                if let Some(authority) = combine_host_port(host, Some(default_port.as_str())) {
                    if let Some(url) = derive_ws_url(protocol.as_ref(), &authority) {
                        push_url(&mut urls, url);
                    }
                }
            }

            if let Some(authority) = combine_host_port(host, port.as_deref()) {
                if let Some(url) = derive_ws_url(protocol.as_ref(), &authority) {
                    push_url(&mut urls, url);
                }
            }

            if !is_loopback && port.as_deref() != Some(default_port.as_str()) {
                if let Some(authority) = combine_host_port(host, Some(default_port.as_str())) {
                    if let Some(url) = derive_ws_url(protocol.as_ref(), &authority) {
                        push_url(&mut urls, url);
                    }
                }
            }
        }

        if let Ok(host) = location.host() {
            if let Some(url) = derive_ws_url(protocol.as_ref(), host.as_ref()) {
                push_url(&mut urls, url);
            }
        }
    }

    #[cfg(feature = "dev-loopback")]
    {
        if !urls.iter().any(|url| url == DEV_LOOPBACK_WS_URL) {
            urls.push(DEV_LOOPBACK_WS_URL.to_string());
        }
    }

    urls
}

fn parse_ws_override(search: &str) -> Option<String> {
    if search.is_empty() {
        return None;
    }
    let params = UrlSearchParams::new_with_str(search)
        .or_else(|_| UrlSearchParams::new_with_str(search.trim_start_matches('?')))
        .ok()?;
    let url = params.get("ws")?;
    if url.is_empty() {
        None
    } else {
        Some(url)
    }
}

fn derive_ws_url(protocol: &str, host: &str) -> Option<String> {
    let scheme = match protocol {
        "https:" | "wss:" => "wss",
        "http:" | "ws:" => "ws",
        other => {
            if let Some(stripped) = other.strip_suffix(':') {
                return derive_ws_url(stripped, host);
            }
            return None;
        }
    };

    let host = host.trim();
    if host.is_empty() {
        return None;
    }

    Some(format!("{scheme}://{host}"))
}

fn combine_host_port(host: &str, port: Option<&str>) -> Option<String> {
    let host = host.trim();
    if host.is_empty() {
        return None;
    }

    let formatted_host = if host.contains(':') && !host.starts_with('[') && !host.ends_with(']') {
        format!("[{host}]")
    } else {
        host.to_string()
    };

    match port {
        Some(port) if !port.is_empty() => Some(format!("{formatted_host}:{port}")),
        _ => Some(formatted_host),
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::derive_ws_url;

    #[test]
    fn maps_http_locations_to_ws() {
        assert_eq!(
            derive_ws_url("http:", "example.com"),
            Some("ws://example.com".to_string())
        );
    }

    #[test]
    fn maps_https_locations_to_wss() {
        assert_eq!(
            derive_ws_url("https:", "example.com:443"),
            Some("wss://example.com:443".to_string())
        );
    }

    #[test]
    fn returns_none_for_unknown_protocol() {
        assert_eq!(derive_ws_url("file:", ""), None);
    }

    #[test]
    fn trims_trailing_colon_variants() {
        assert_eq!(
            derive_ws_url("https", "example.com"),
            Some("wss://example.com".to_string())
        );
    }
}
