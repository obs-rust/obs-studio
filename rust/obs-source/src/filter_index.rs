//! Pointer-identity lookup and validation order of the source filter index query.
use core::ffi::CStr;

pub fn valid(source: bool, filter: bool, log: impl FnOnce(&'static CStr)) -> bool {
    if !source {
        log(c"source");
        false
    } else if !filter {
        log(c"filter");
        false
    } else {
        true
    }
}

pub fn index<T: PartialEq>(entries: &[T], filter: &T) -> i32 {
    entries
        .iter()
        .position(|entry| entry == filter)
        .map_or(-1, |index| index as i32)
}
