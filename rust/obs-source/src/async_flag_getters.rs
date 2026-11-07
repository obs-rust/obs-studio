//! Read-only async flags with the original NULL diagnostics.
use core::ffi::CStr;

pub fn unbuffered(_value: Option<bool>, _log: impl FnOnce(&'static CStr, &'static CStr)) -> bool {
    todo!("#3872")
}

pub fn decoupled(_value: Option<bool>, _log: impl FnOnce(&'static CStr, &'static CStr)) -> bool {
    todo!("#3872")
}
