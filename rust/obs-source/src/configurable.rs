//! Whether a source instance can show properties: live data and either callback.
use core::ffi::CStr;

/// The instance fields the query reads, taken only after NULL was rejected.
pub struct Source {
    pub data: bool,
    pub get_properties: bool,
    pub get_properties2: bool,
}

/// `None` is a NULL source, which logs once and answers false. No callback is
/// called; a source without data answers false silently.
pub fn configurable(source: Option<Source>, log: impl FnOnce(&'static CStr)) -> bool {
    let Some(source) = source else {
        log(c"obs_source_configurable");
        return false;
    };
    source.data && (source.get_properties || source.get_properties2)
}
