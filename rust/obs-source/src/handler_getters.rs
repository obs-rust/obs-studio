//! Borrowed source signal and procedure handlers; ownership stays with the source.
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

pub fn signal<T: Copy + Default>(source: Option<T>, log: impl FnOnce(&'static CStr)) -> T {
    match source {
        Some(handler) => handler,
        None => {
            log(c"obs_source_get_signal_handler");
            T::default()
        }
    }
}
