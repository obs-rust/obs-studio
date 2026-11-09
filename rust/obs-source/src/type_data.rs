//! Borrowed source type-data query; ownership remains with the registered type.
use core::ffi::CStr;

pub fn get<T: Copy + Default>(source: Option<T>, log: impl FnOnce(&'static CStr)) -> T {
    match source {
        Some(data) => data,
        None => {
            log(c"obs_source_get_type_data");
            T::default()
        }
    }
}
