#![cfg(target_arch = "wasm32")]
//! WASM entry point for the Bevy viewer.
//!
//! The bulk of the implementation lives inside `opfoundry_gui`; this crate simply
//! exposes the `wasm_bindgen` start hook so the generated JavaScript can spin up
//! the app with minimal glue.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
/// Initialise the Bevy wasm app. This is invoked by the generated JS glue code
/// and forwards straight into [`opfoundry_gui::start_web_app`].
pub fn start() -> Result<(), JsValue> {
    opfoundry_gui::start_web_app()
}
