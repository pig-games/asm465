#[cfg(feature = "native-service")]
fn main() {
    if let Err(err) = opfoundry_gui::run_native() {
        eprintln!("opFoundry error: {err}");
    }
}

#[cfg(not(feature = "native-service"))]
fn main() {
    panic!("native-service feature is required for the native binary");
}
