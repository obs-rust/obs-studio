//! Read-only deinterlace accessors, preserving historical NULL diagnostics.
use core::ffi::CStr;

pub fn mode(value: Option<u32>, log: impl FnOnce(&'static CStr, &'static CStr)) -> u32 {
    match value {
        Some(mode) => mode,
        None => {
            log(c"obs_source_set_deinterlace_mode", c"source");
            0
        }
    }
}

pub fn field_order(top_first: Option<bool>, log: impl FnOnce(&'static CStr, &'static CStr)) -> u32 {
    match top_first {
        Some(top) => u32::from(!top),
        None => {
            log(c"obs_source_set_deinterlace_field_order", c"source");
            0
        }
    }
}
