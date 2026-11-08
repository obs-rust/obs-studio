//! Public output pause-state queries.
use core::ffi::CStr;

pub fn paused(value: Option<bool>, log: impl FnOnce(&'static CStr)) -> bool {
    value.unwrap_or_else(|| {
        log(c"obs_output_paused");
        false
    })
}

pub fn offset(value: Option<u64>, log: impl FnOnce(&'static CStr)) -> u64 {
    value.unwrap_or_else(|| {
        log(c"obs_output_get_pause_offset");
        0
    })
}
