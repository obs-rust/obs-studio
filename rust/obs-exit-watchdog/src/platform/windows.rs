//! Windows: `TerminateProcess` on the own process ends it without the
//! DLLs' detach code and the CRT's `atexit` handlers.

/// Ends the process now with `code`.
pub fn terminate(code: i32) -> ! {
    let _ = code;
    todo!("#4065")
}
