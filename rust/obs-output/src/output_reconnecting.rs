//! Output reconnecting query without the private C layout.
use core::ffi::CStr;

/// Preserve the stored flag and NULL diagnostic.
pub fn get(_value: Option<bool>, _log: impl FnOnce(&'static CStr)) -> bool {
    todo!("port obs_output_reconnecting")
}
