//! Borrowed relationship reads, preserving NULL diagnostics.
use core::ffi::CStr;

pub fn parent<T: Default>(_value: Option<T>, _log: impl FnOnce(&'static CStr, &'static CStr)) -> T {
    todo!("#3882")
}

pub fn target<T: Default>(_value: Option<T>, _log: impl FnOnce(&'static CStr, &'static CStr)) -> T {
    todo!("#3882")
}
