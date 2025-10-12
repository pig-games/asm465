#![cfg(target_arch = "wasm32")]

use asm465::{
    Asm465App, ProgramSource, ServiceCommand, ServiceRequestPayload, ServiceResponseMessage,
};
use futures::{channel::mpsc, SinkExt, StreamExt};
use gloo_file::callbacks::read_as_bytes;
use gloo_file::File;
use gloo_net::websocket::futures::WebSocket;
use gloo_net::websocket::Message;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{self, HtmlButtonElement, HtmlInputElement, UrlSearchParams};

const DEFAULT_MAX_CYCLES: u64 = 5_000_000;
const CANVAS_ID: &str = "asm465-canvas";
const DEFAULT_WS_URL: &str = "ws://127.0.0.1:8800";

thread_local! {
    static FILE_QUEUE: RefCell<Vec<(Vec<u8>, Option<String>)>> = RefCell::new(Vec::new());
}

struct WebSocketService {
    pending: Rc<RefCell<Vec<ServiceCommand>>>,
    response_tx: mpsc::UnboundedSender<ServiceResponseMessage>,
}

impl WebSocketService {
    fn connect(url: &str) -> Result<Self, JsValue> {
        let ws = WebSocket::open(url).map_err(|err| JsValue::from_str(&format!("{err}")))?;
        let (mut write, mut read) = ws.split();

        let pending = Rc::new(RefCell::new(Vec::new()));
        let pending_reader = pending.clone();

        wasm_bindgen_futures::spawn_local(async move {
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
        wasm_bindgen_futures::spawn_local(async move {
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

    fn poll(&mut self, app: &mut Asm465App) {
        let mut queue = self.pending.borrow_mut();
        for command in queue.drain(..) {
            let response = app.handle_service_command(command);
            if let Err(err) = self.response_tx.unbounded_send(response) {
                log::error!("failed to queue websocket response: {err}");
            }
        }
    }

    fn send_response(&self, response: ServiceResponseMessage) {
        if let Err(err) = self.response_tx.unbounded_send(response) {
            log::error!("failed to send response to bridge: {err}");
        }
    }
}

struct WasmApp {
    inner: Asm465App,
    service: Option<WebSocketService>,
}

impl WasmApp {
    fn new(inner: Asm465App, service: Option<WebSocketService>) -> Self {
        Self { inner, service }
    }
}

impl eframe::App for WasmApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let mut queued_responses: Vec<Result<String, String>> = Vec::new();
        FILE_QUEUE.with(|queue| {
            let mut q = queue.borrow_mut();
            for (bytes, name) in q.drain(..) {
                let source = ProgramSource::Inline { name, data: bytes };
                queued_responses.push(self.inner.run_program(
                    source,
                    Some(DEFAULT_MAX_CYCLES),
                    None,
                ));
            }
        });
        if let Some(service) = self.service.as_mut() {
            for result in queued_responses.drain(..) {
                match result {
                    Ok(message) => service.send_response(ServiceResponseMessage::ok(message)),
                    Err(err) => service.send_response(ServiceResponseMessage::error(err)),
                }
            }
            service.poll(&mut self.inner);
        }
        self.inner.update_frame(ctx, frame);
    }
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

    let trigger_input = input.clone();
    let click_closure = Closure::wrap(Box::new(move || {
        trigger_input.click();
    }) as Box<dyn FnMut()>);
    button.add_event_listener_with_callback("click", click_closure.as_ref().unchecked_ref())?;
    click_closure.forget();

    let input_for_change = input.clone();
    let change_closure = Closure::wrap(Box::new(move |_: web_sys::Event| {
        if let Some(files) = input_for_change.files() {
            for idx in 0..files.length() {
                if let Some(file) = files.get(idx) {
                    let name = file.name();
                    let file = File::from(file);
                    let reader_state: Rc<RefCell<Option<_>>> = Rc::new(RefCell::new(None));
                    let reader_state_clone = reader_state.clone();
                    let name_clone = name.clone();
                    let reader = read_as_bytes(&file, move |result| {
                        match result {
                            Ok(bytes) => {
                                FILE_QUEUE.with(|queue| {
                                    queue.borrow_mut().push((bytes, Some(name_clone.clone())));
                                });
                            }
                            Err(err) => log::error!("failed to read file: {err}"),
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

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    if let Err(err) = setup_file_loader() {
        log::error!("file loader init failed: {err:?}");
    }

    wasm_bindgen_futures::spawn_local(async move {
        let web_options = eframe::WebOptions::default();
        let mut service_opt =
            resolve_ws_url().and_then(|url| match WebSocketService::connect(&url) {
                Ok(service) => Some(service),
                Err(err) => {
                    log::error!("failed to establish websocket {url}: {err:?}");
                    None
                }
            });

        if let Err(err) = eframe::WebRunner::new()
            .start(
                CANVAS_ID,
                web_options,
                Box::new(move |cc| {
                    let service = service_opt.take();
                    let app = Asm465App::new(cc, None, DEFAULT_MAX_CYCLES, String::new(), None);
                    Box::new(WasmApp::new(app, service)) as Box<dyn eframe::App>
                }),
            )
            .await
        {
            log::error!("failed to start asm465 wasm app: {err:?}");
        }
    });

    Ok(())
}
