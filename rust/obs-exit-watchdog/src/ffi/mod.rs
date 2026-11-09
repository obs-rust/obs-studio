//! C ABI for the frontend (#4065), declared in
//! `frontend/utility/obs-exit-watchdog.h`. It converts types and calls the
//! safe core; there is no logic here.

use std::ffi::{CStr, c_char};
use std::path::Path;

/// Arms the watchdog with [`crate::TIMEOUT`]; `log_path` (UTF-8, may be
/// null) is the log file to append the line to.
///
/// # Safety
///
/// `log_path` is null or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_exit_watchdog_arm(log_path: *const c_char) -> bool {
    // SAFETY: the caller passes null or a valid C string.
    let path = (!log_path.is_null()).then(|| unsafe { CStr::from_ptr(log_path) });
    let path = path.and_then(|p| p.to_str().ok()).map(Path::new);
    crate::arm(crate::TIMEOUT, path)
}
