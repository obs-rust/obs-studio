//! Public encoder GPU scaling getters with the original diagnostics.
use core::ffi::CStr;

pub fn scale_type(_fields: Option<(i32, i32)>, _log: impl FnOnce(i32, &'static CStr)) -> i32 {
    todo!()
}

pub fn enabled(_fields: Option<(i32, i32)>, _log: impl FnOnce(i32, &'static CStr)) -> bool {
    todo!()
}
