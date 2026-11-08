//! Public output flags with the original NULL diagnostic.
use core::ffi::CStr;

pub fn flags(value: Option<u32>, log: impl FnOnce(&'static CStr)) -> u32 {
    let _ = (value, log);
    todo!("port output flags query")
}
