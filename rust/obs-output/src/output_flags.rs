//! Public output flags with the original NULL diagnostic.
use core::ffi::CStr;

pub fn flags(value: Option<u32>, log: impl FnOnce(&'static CStr)) -> u32 {
    match value {
        Some(flags) => flags,
        None => {
            log(c"obs_output_get_flags");
            0
        }
    }
}
