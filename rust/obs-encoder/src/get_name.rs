//! Borrow the current context name without copying or caching it.
use std::ffi::CStr;

/// Reject a missing encoder before projecting its C-owned name.
pub fn get<T>(
    encoder: Option<impl FnOnce() -> T>,
    log: impl FnOnce(i32, &'static CStr),
) -> Option<T> {
    match encoder {
        Some(project) => Some(project()),
        None => {
            log(400, c"obs_encoder_get_name: Null 'encoder' parameter");
            None
        }
    }
}
