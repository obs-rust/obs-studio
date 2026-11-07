//! Plain async flag reads, preserving NULL defaults and diagnostics.
use core::ffi::CStr;

pub fn unbuffered(value: Option<bool>, log: impl FnOnce(&'static CStr, &'static CStr)) -> bool {
    match value {
        Some(flag) => flag,
        None => {
            log(c"obs_source_async_unbuffered", c"source");
            false
        }
    }
}

pub fn decoupled(value: Option<bool>, log: impl FnOnce(&'static CStr, &'static CStr)) -> bool {
    match value {
        Some(flag) => flag,
        None => {
            log(c"obs_source_async_decoupled", c"source");
            false
        }
    }
}
