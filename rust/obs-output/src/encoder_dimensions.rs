//! Public width and height getters, preserving diagnostics and lazy media reads.
use core::ffi::CStr;

pub fn width(fields: Option<(i32, bool, u32)>, media: impl FnOnce() -> u32, log: impl FnOnce(i32, &'static CStr)) -> u32 {
    let _ = (fields, media, log);
    todo!("public width getter")
}
pub fn height(fields: Option<(i32, bool, u32)>, media: impl FnOnce() -> u32, log: impl FnOnce(i32, &'static CStr)) -> u32 {
    let _ = (fields, media, log);
    todo!("public height getter")
}
