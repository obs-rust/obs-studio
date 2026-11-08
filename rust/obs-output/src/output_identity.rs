//! Borrowed output identity metadata with the existing NULL diagnostics.
use core::ffi::CStr;

fn read<T: Default>(value: Option<T>, name: &'static CStr, log: impl FnOnce(&'static CStr)) -> T {
    match value {
        Some(value) => value,
        None => {
            log(name);
            T::default()
        }
    }
}

pub fn id<T: Default>(value: Option<T>, log: impl FnOnce(&'static CStr)) -> T {
    read(value, c"obs_output_get_id", log)
}

pub fn type_data<T: Default>(value: Option<T>, log: impl FnOnce(&'static CStr)) -> T {
    read(value, c"obs_output_get_type_data", log)
}
