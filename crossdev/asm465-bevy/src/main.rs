#[cfg(feature = "native-service")]
fn main() {
    if let Err(err) = asm465_bevy::run_native() {
        eprintln!("asm465-bevy error: {err}");
    }
}

#[cfg(not(feature = "native-service"))]
fn main() {
    panic!("native-service feature is required for the native binary");
}
