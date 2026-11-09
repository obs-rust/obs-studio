//! Read-only source filter count, preserving the C NULL diagnostic.
use core::ffi::CStr;

pub fn count(source: Option<usize>, log: impl FnOnce(&'static CStr)) -> usize {
    todo!()
}
