//! Registered source identity, independent of its mutable context name.
use core::ffi::CStr;

pub fn get_type(source: Option<u32>, log: impl FnOnce(&'static CStr)) -> u32 {
    match source {
        Some(kind) => kind,
        None => {
            log(c"obs_source_get_type");
            0
        }
    }
}

pub fn get_id<T: Copy + Default>(source: Option<T>, log: impl FnOnce(&'static CStr)) -> T {
    match source {
        Some(id) => id,
        None => {
            log(c"obs_source_get_id");
            T::default()
        }
    }
}
