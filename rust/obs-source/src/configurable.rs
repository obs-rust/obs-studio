//! Whether a source instance can show properties: live data and either callback.
use core::ffi::CStr;

/// The instance fields the query reads, taken only after NULL was rejected.
pub struct Source {
    pub data: bool,
    pub get_properties: bool,
    pub get_properties2: bool,
}

/// `None` is a NULL source, which logs once and answers false.
pub fn configurable(source: Option<Source>, log: impl FnOnce(&'static CStr)) -> bool {
    let _ = (source, log);
    todo!("obs_source_configurable (Refs #5117)")
}
