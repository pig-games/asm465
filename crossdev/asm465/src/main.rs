#[cfg(feature = "native-service")]
fn main() -> eframe::Result<()> {
    asm465::run_native()
}

#[cfg(not(feature = "native-service"))]
fn main() {
    panic!("native-service feature is required for the native binary");
}
