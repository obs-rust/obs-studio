//! Pointer-identity lookup and validation order of the source filter index query.
use core::ffi::CStr;

pub fn valid(source: bool, filter: bool, log: impl FnOnce(&'static CStr)) -> bool {
    let _ = (source, filter, log);
    todo!()
}

pub fn index<T: PartialEq>(entries: &[T], filter: &T) -> i32 {
    let _ = (entries, filter);
    todo!()
}
