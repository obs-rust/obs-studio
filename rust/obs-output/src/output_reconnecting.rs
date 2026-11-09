//! Output reconnecting query without the private C layout.
use core::ffi::CStr;

/// Preserve the stored flag and NULL diagnostic.
pub fn get(value: Option<bool>, log: impl FnOnce(&'static CStr)) -> bool {
    value.unwrap_or_else(|| {
        log(c"obs_output_reconnecting");
        false
    })
}
