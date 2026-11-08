//! Stored source state queries, preserving NULL defaults and diagnostics.
use core::ffi::CStr;

pub fn flags(value: Option<u32>, log: impl FnOnce(&'static CStr, &'static CStr)) -> u32 {
    match value {
        Some(value) => value,
        None => {
            log(c"obs_source_get_flags", c"source");
            0
        }
    }
}

pub fn enabled(value: Option<bool>, log: impl FnOnce(&'static CStr, &'static CStr)) -> bool {
    match value {
        Some(value) => value,
        None => {
            log(c"obs_source_enabled", c"source");
            false
        }
    }
}

pub fn last_obs_version(value: Option<u32>, log: impl FnOnce(&'static CStr, &'static CStr)) -> u32 {
    match value {
        Some(value) => value,
        None => {
            log(c"obs_source_get_last_obs_version", c"source");
            0
        }
    }
}
