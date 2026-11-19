//! The paired subscriptions used to pause source audio.
use core::ffi::CStr;

/// Validate before acquiring the borrowed handler, then update both signals.
pub fn update<H>(
    present: bool,
    adding: bool,
    handler: impl FnOnce() -> H,
    change: impl FnMut(&H, &'static CStr),
    log: impl FnOnce(&'static CStr),
) {
    let _ = (present, adding, handler, change, log);
    todo!()
}
