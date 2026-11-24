use std::ffi::{CStr, c_char, c_int};
use std::path::Path;
use std::process::ExitCode;

/// CMake keeps its native executable target for code signing and installation.
/// The entry point passes a UTF-8 cache path, or null for a missing argument.
#[unsafe(no_mangle)]
unsafe extern "C" fn vst3_scanner_main(cache_path: *const c_char) -> c_int {
    let cache_path = if cache_path.is_null() {
        None
    } else {
        // SAFETY: The launcher supplies a NUL-terminated, live argv string.
        (unsafe { CStr::from_ptr(cache_path) })
            .to_str()
            .ok()
            .map(Path::new)
    };
    i32::from(crate::run_installed(cache_path) != ExitCode::SUCCESS)
}
