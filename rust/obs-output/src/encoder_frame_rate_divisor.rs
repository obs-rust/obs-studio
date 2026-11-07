//! Public frame-rate-divisor accessor, preserving historical diagnostics.
use core::ffi::CStr;

pub fn get(_fields: Option<(i32, u32)>, _log: impl FnOnce(i32, &'static CStr)) -> u32 {
    todo!("#3921")
}
