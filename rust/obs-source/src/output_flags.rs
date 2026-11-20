//! A source type's declared output flags, read from the instance's own info.
use core::ffi::CStr;

/// `None` is a NULL source: it logs once and returns 0 without reading
/// anything. Otherwise `read` is called once and its value is returned
/// unchanged, unknown bits included (characterized, not endorsed).
pub fn output_flags(source: Option<impl FnOnce() -> u32>, log: impl FnOnce(&'static CStr)) -> u32 {
    let _ = (source, log);
    todo!("RED: obs_source_get_output_flags core")
}
