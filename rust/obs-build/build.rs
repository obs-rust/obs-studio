fn main() {
    println!(
        "cargo:rustc-env=OBS_BUILD_HOST={}",
        std::env::var("HOST").unwrap()
    );
    println!(
        "cargo:rustc-env=OBS_BUILD_TARGET={}",
        std::env::var("TARGET").unwrap()
    );
}
