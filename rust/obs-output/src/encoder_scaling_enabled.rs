//! Public scaling-enabled getter with the original NULL diagnostic.
use core::ffi::CStr;

pub fn get(fields: Option<(u32, u32)>, log: impl FnOnce(&'static CStr, &'static CStr)) -> bool {
    let _ = (fields, log);
    todo!("#3926")
}
