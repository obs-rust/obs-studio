//! Windows: `TerminateProcess` on the own process ends it without the
//! DLLs' detach code and the CRT's `atexit` handlers.

use std::ffi::c_void;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn TerminateProcess(process: *mut c_void, exit_code: u32) -> i32;
}

/// Ends the process now with `code`.
pub fn terminate(code: i32) -> ! {
    // SAFETY: the pseudo handle of the own process is always valid;
    // TerminateProcess on it does not return when it succeeds.
    unsafe {
        TerminateProcess(GetCurrentProcess(), code.cast_unsigned());
    }
    // Only when TerminateProcess failed.
    std::process::abort()
}
