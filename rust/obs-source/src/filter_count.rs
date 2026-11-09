//! Read-only source filter count, preserving the C NULL diagnostic.
use core::ffi::CStr;

pub fn count(source: Option<usize>, log: impl FnOnce(&'static CStr)) -> usize {
    match source {
        Some(count) => count,
        None => {
            log(c"obs_source_filter_count");
            0
        }
    }
}
