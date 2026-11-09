//! Linux and macOS: `_exit(2)` ends the process without `atexit` handlers
//! and static destructors.

/// Ends the process now with `code`.
pub fn terminate(code: i32) -> ! {
    let _ = code;
    todo!("#4065")
}
