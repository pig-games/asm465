#[cfg(feature = "native-service")]
fn main() {
    if let Err(err) = asm465::run_native() {
        eprintln!("asm465 error: {err}");
    }
}

#[cfg(not(feature = "native-service"))]
fn main() {
    panic!("native-service feature is required for the native binary");
}
