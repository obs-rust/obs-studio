//! Encoder counter accessors. Missing encoders return zero; only the frame
//! getter logs, preserving its historical output-getter name.

use core::ffi::CStr;

pub fn encoded_frames(value: Option<u32>, log: impl FnOnce(&'static CStr, &'static CStr)) -> u32 {
    match value {
        Some(frames) => frames,
        None => {
            log(c"obs_output_get_encoded_frames", c"encoder");
            0
        }
    }
}

pub fn pause_offset(value: Option<u64>) -> u64 {
    value.unwrap_or_default()
}
