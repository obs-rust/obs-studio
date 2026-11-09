//! Linux and macOS: `_exit(2)` ends the process without `atexit` handlers
//! and static destructors.

/// Ends the process now with `code`.
pub fn terminate(code: i32) -> ! {
    // SAFETY: _exit takes any status and does not return.
    unsafe { libc::_exit(code) }
}
