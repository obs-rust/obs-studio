//! Public encoder active and paused queries.
use core::ffi::CStr;

pub fn active(value: Option<bool>, log: impl FnOnce(&'static CStr)) -> bool {
    let _ = (value, log);
    todo!()
}

pub fn paused(value: Option<bool>, log: impl FnOnce(&'static CStr)) -> bool {
    let _ = (value, log);
    todo!()
}
