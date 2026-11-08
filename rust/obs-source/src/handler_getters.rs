//! Borrowed source procedure handler; ownership stays with the source.
use core::ffi::CStr;

pub fn proc<T: Copy + Default>(source: Option<T>, log: impl FnOnce(&'static CStr)) -> T {
    let _ = (source, log);
    todo!("source procedure handler getter")
}
