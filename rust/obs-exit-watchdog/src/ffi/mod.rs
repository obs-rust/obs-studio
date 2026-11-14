//! C ABI for the frontend (#4065), declared in
//! `frontend/utility/obs-exit-watchdog.h`. It converts types and calls the
//! safe core; there is no logic here.

use std::ffi::{CStr, c_char};
use std::path::Path;

/// Starts the helper process with [`crate::TIMEOUT`] (#4589); `log_path`
/// (UTF-8, may be null) is the log file it appends the line to.
///
/// # Safety
///
/// `log_path` is null or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_exit_watchdog_start(log_path: *const c_char) -> bool {
    // SAFETY: the caller passes null or a valid C string.
    let path = (!log_path.is_null()).then(|| unsafe { CStr::from_ptr(log_path) });
    let path = path.and_then(|p| p.to_str().ok()).map(Path::new);
    crate::start(crate::TIMEOUT, path)
}

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

/// The frontend notes the binary path of a loaded OBS module (UTF-8, may
/// be null), before [`obs_exit_watchdog_note_loaded`].
///
/// # Safety
///
/// `bin_path` is null or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_exit_watchdog_note_module(bin_path: *const c_char) {
    if bin_path.is_null() {
        return;
    }
    // SAFETY: the caller passes a valid C string.
    if let Ok(path) = unsafe { CStr::from_ptr(bin_path) }.to_str() {
        crate::note_module(path);
    }
}

/// The frontend: the modules are loaded, before their post-load.
#[unsafe(no_mangle)]
pub extern "C" fn obs_exit_watchdog_note_loaded() {
    crate::note_loaded();
}

/// The frontend: the modules' post-load is done.
#[unsafe(no_mangle)]
pub extern "C" fn obs_exit_watchdog_note_post_load() {
    crate::note_post_load();
}
