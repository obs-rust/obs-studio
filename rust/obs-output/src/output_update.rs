//! Safe core of `libobs/obs-output-update.c.h`: an output's settings update.
use core::ffi::CStr;

/// `obs_output_update()`.
pub fn update(
    valid: bool,
    apply: impl FnOnce(),
    has_update: impl FnOnce() -> bool,
    call_update: impl FnOnce(),
    log: impl FnOnce(&'static CStr),
) {
    let _ = (valid, apply, has_update, call_update, log);
    todo!("obs_output_update")
}
