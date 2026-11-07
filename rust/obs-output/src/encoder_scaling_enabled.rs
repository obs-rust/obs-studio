//! Public scaling-enabled getter with the original NULL diagnostic.
use crate::encoder_size::{Size, scaling_enabled};
use core::ffi::CStr;

pub fn get(fields: Option<(u32, u32)>, log: impl FnOnce(&'static CStr, &'static CStr)) -> bool {
    match fields {
        Some((width, height)) => scaling_enabled(Size { width, height }),
        None => {
            log(c"obs_encoder_scaling_enabled", c"encoder");
            false
        }
    }
}
