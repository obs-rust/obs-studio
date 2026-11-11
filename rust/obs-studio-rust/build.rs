#[path = "../obs-build/registration_modules.rs"]
mod registration_modules;

fn main() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../obs-build/tests/native/registration");
    println!("cargo:rerun-if-changed={}", directory.display());
    let declarations = registration_modules::declarations(&directory)
        .expect("discover native test registration modules");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    std::fs::write(output.join("native-registration.rs"), declarations)
        .expect("write native test registration modules");
}
