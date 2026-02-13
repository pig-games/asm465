
# Safari-Safe File Picker Integration for asm465-wasm (Option A: rfd::AsyncFileDialog)

This document explains the clean, egui-native approach using **rfd::AsyncFileDialog**
to open the file picker directly from your Bevy/egui UI in asm465-wasm.
This approach works across browsers — including Safari — when implemented correctly.

---

## 🧭 Overview

- Uses `rfd`’s async file dialog API, which works on **native + WASM**.
- File dialog is opened **synchronously** from the user’s click event — this is critical for Safari.
- No async/await, timers, or event-loop hops *before* calling `.pick_file()`.
- Fully contained in Rust; no manual JS or overlay DOM elements required.

---

## 🧱 Cargo.toml Configuration

Add or update your dependencies in `crossdev/asm465-wasm/Cargo.toml`:

```toml
[dependencies]
rfd = { version = "0.14", default-features = false, features = ["wasm"] }
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
```

---

## 🧩 egui Button Integration

Place this inside your egui UI code (where you create the “Load PRG” button).

```rust
use rfd::AsyncFileDialog;
use wasm_bindgen_futures::spawn_local;

pub fn ui_load_button(ui: &mut egui::Ui) {
    if ui.button("Load PRG").clicked() {
        // ✅ Launch the dialog immediately in this click event turn
        spawn_local(async move {
            if let Some(handle) = AsyncFileDialog::new()
                .add_filter("PRG", &["prg"])
                .pick_file()
                .await
            {
                let data = handle.read().await;
                // TODO: call your PRG loader here
                // crate::host::load_prg(data);
            }
        });
    }
}
```

---

## 🧠 Why This Works (Even on Safari)

- Safari only allows file pickers to open **within the same user-activation context**.
- As long as `.pick_file()` is called **directly** from the button click (no async gaps), it passes the rule.
- Chrome/Firefox are more lenient, but this approach satisfies the strictest requirement.
- `rfd`’s WASM backend handles the DOM `<input type="file">` creation internally, correctly visible for Safari.

---

## ⚠️ Common Gotchas

| Issue | Cause | Fix |
|-------|-------|-----|
| File dialog doesn’t open | `pick_file()` is called after an async boundary (await/timer) | Move the call directly inside the `.clicked()` branch |
| Works in Chrome, fails in Safari | Hidden/unclickable DOM input | Avoid custom CSS that hides `<input type=file>` globally |
| Nothing happens when clicking | The call is deferred to “next frame” | Trigger `pick_file()` in the same UI event cycle |

---

## 🧰 Optional Helper Wrapper

If you want to reuse this in multiple UI spots:

```rust
pub fn request_load_prg() {
    use rfd::AsyncFileDialog;
    use wasm_bindgen_futures::spawn_local;

    spawn_local(async move {
        if let Some(handle) = AsyncFileDialog::new()
            .add_filter("PRG", &["prg"])
            .pick_file()
            .await
        {
            let data = handle.read().await;
            // crate::host::load_prg(data);
        }
    });
}
```

Then use it cleanly in your UI:

```rust
if ui.button("Load PRG").clicked() {
    request_load_prg();
}
```

---

## ✅ Summary

- Keep `.pick_file()` directly tied to the user’s click event.
- No need for JS bridges or DOM overlays unless Safari *still* blocks it (rare).
- Use `rfd::AsyncFileDialog` as the first choice — it’s the simplest, egui-native fix.

---

## 🧩 Next Step

If you want, integrate the following inside your loader pipeline:

```rust
// in your wasm-side loader
crate::host::load_prg(data);
```

Replace `crate::host::load_prg` with your actual function for pushing the PRG bytes
into cross465 or asm465’s memory space.
