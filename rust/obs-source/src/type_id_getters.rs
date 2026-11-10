//! Registered source identity, independent of its mutable context name.
use core::ffi::CStr;

pub fn get_type(source: Option<u32>, log: impl FnOnce(&'static CStr)) -> u32 {
    let _ = (source, log);
    todo!("source type getter")
}

pub fn get_id<T: Copy + Default>(source: Option<T>, log: impl FnOnce(&'static CStr)) -> T {
    let _ = (source, log);
    todo!("source ID getter")
}
