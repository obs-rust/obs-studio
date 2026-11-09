//! Borrowed source procedure handler; ownership stays with the source.
use core::ffi::CStr;

pub fn proc<T: Copy + Default>(source: Option<T>, log: impl FnOnce(&'static CStr)) -> T {
    match source {
        Some(handler) => handler,
        None => {
            log(c"obs_source_get_proc_handler");
            T::default()
        }
    }
}
