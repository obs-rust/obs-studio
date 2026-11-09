//! Borrowed source type-data query; ownership remains with the registered type.
use core::ffi::CStr;

pub fn get<T: Copy + Default>(_source: Option<T>, _log: impl FnOnce(&'static CStr)) -> T {
    todo!("port obs_source_get_type_data")
}
