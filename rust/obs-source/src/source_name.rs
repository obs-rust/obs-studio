//! Borrow the stored source name without copying or extending its lifetime.
use core::ffi::CStr;

pub fn name<T: Copy + Default>(
    valid: bool,
    read: impl FnOnce() -> T,
    log: impl FnOnce(&'static CStr),
) -> T {
    if valid {
        read()
    } else {
        log(c"obs_source_get_name");
        T::default()
    }
}
