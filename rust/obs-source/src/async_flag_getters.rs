//! Plain async flag reads and the unbuffered store, preserving NULL defaults
//! and diagnostics.
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

/// Store `unbuffered` into the source's flag; a missing source only logs.
pub fn set_unbuffered(
    value: Option<&mut bool>,
    unbuffered: bool,
    log: impl FnOnce(&'static CStr, &'static CStr),
) {
    match value {
        Some(flag) => *flag = unbuffered,
        None => log(c"obs_source_set_async_unbuffered", c"source"),
    }
}
