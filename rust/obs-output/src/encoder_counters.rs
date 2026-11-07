//! Encoder counter accessors. Missing encoders return zero; only the frame
//! getter logs, preserving its historical output-getter name.

use core::ffi::CStr;

pub fn encoded_frames(_value: Option<u32>, _log: impl FnOnce(&'static CStr, &'static CStr)) -> u32 {
    todo!("#3856")
}

pub fn pause_offset(_value: Option<u64>) -> u64 {
    todo!("#3856")
}
