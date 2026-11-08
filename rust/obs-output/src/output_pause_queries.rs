//! Public output pause-state queries.
use core::ffi::CStr;

pub fn paused(_value: Option<bool>, _log: impl FnOnce(&'static CStr)) -> bool {
    todo!("port obs_output_paused")
}

pub fn offset(_value: Option<u64>, _log: impl FnOnce(&'static CStr)) -> u64 {
    todo!("port obs_output_get_pause_offset")
}
