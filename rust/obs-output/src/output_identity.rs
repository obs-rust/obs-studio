//! Borrowed output identity metadata with the existing NULL diagnostics.
use core::ffi::CStr;

pub fn id<T: Default>(_value: Option<T>, _log: impl FnOnce(&'static CStr)) -> T {
    todo!("output identity getter")
}

pub fn type_data<T: Default>(_value: Option<T>, _log: impl FnOnce(&'static CStr)) -> T {
    todo!("output identity getter")
}
