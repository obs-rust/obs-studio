//! The settings getter returns the stored value with one added reference.
use core::ffi::CStr;

pub fn get<T: Copy + Default>(
    source: Option<T>,
    addref: impl FnOnce(T),
    log: impl FnOnce(&'static CStr),
) -> T {
    match source {
        Some(settings) => {
            addref(settings);
            settings
        }
        None => {
            log(c"obs_source_get_settings");
            T::default()
        }
    }
}
