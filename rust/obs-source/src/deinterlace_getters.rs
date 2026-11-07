//! Read-only deinterlace accessors, preserving historical NULL diagnostics.
use core::ffi::CStr;

pub fn mode(_value: Option<u32>, _log: impl FnOnce(&'static CStr, &'static CStr)) -> u32 {
    todo!("#3861")
}

pub fn field_order(
    _top_first: Option<bool>,
    _log: impl FnOnce(&'static CStr, &'static CStr),
) -> u32 {
    todo!("#3861")
}
