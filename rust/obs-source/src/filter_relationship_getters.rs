//! Borrowed relationship reads, preserving NULL diagnostics.
use core::ffi::CStr;

/// Return the stored borrowed identity; a missing filter logs and returns its default.
pub fn parent<T: Default>(value: Option<T>, log: impl FnOnce(&'static CStr, &'static CStr)) -> T {
    match value {
        Some(pointer) => pointer,
        None => {
            log(c"obs_filter_get_parent", c"filter");
            T::default()
        }
    }
}

/// Return the stored borrowed identity; a missing filter logs and returns its default.
pub fn target<T: Default>(value: Option<T>, log: impl FnOnce(&'static CStr, &'static CStr)) -> T {
    match value {
        Some(pointer) => pointer,
        None => {
            log(c"obs_filter_get_target", c"filter");
            T::default()
        }
    }
}
