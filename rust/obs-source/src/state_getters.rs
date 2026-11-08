//! Stored source state queries, preserving NULL defaults and diagnostics.
use core::ffi::CStr;

pub fn flags(value: Option<u32>, log: impl FnOnce(&'static CStr, &'static CStr)) -> u32 {
    todo!()
}

pub fn enabled(value: Option<bool>, log: impl FnOnce(&'static CStr, &'static CStr)) -> bool {
    todo!()
}

pub fn last_obs_version(value: Option<u32>, log: impl FnOnce(&'static CStr, &'static CStr)) -> u32 {
    todo!()
}

