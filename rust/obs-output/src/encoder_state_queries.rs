//! Public encoder active and paused queries.
use core::ffi::CStr;

pub fn active(value: Option<bool>, log: impl FnOnce(&'static CStr)) -> bool {
    match value {
        Some(value) => value,
        None => {
            log(c"obs_encoder_active");
            false
        }
    }
}

pub fn paused(value: Option<bool>, log: impl FnOnce(&'static CStr)) -> bool {
    match value {
        Some(value) => value,
        None => {
            log(c"obs_encoder_paused");
            false
        }
    }
}
