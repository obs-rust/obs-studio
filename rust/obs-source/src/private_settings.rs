//! `obs_source_get_private_settings` (libobs/obs-source-private-settings.c.h):
//! the stored private settings with one added reference.
use core::ffi::CStr;

/// The name the NULL-source diagnostic reports.
pub const NAME: &CStr = c"obs_source_get_private_settings";

/// `source` is the stored private-settings value of a live source, or `None`
/// for a NULL source. A live source's value, NULL included, gets exactly one
/// `addref` and is returned unchanged; a NULL source logs once and returns
/// the default.
pub fn get<T: Copy + Default>(
    source: Option<T>,
    addref: impl FnOnce(T),
    log: impl FnOnce(&'static CStr),
) -> T {
    match source {
        Some(private_settings) => {
            addref(private_settings);
            private_settings
        }
        None => {
            log(NAME);
            T::default()
        }
    }
}
