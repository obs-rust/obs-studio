//! `obs_source_removed()`: whether a source has been removed.
use core::ffi::CStr;

/// The public function named in the NULL diagnostic.
pub const FUNCTION: &CStr = c"obs_source_removed";

/// `None` is a NULL source: it logs once with [`FUNCTION`] and answers true
/// (a missing source counts as removed) without reading anything. Otherwise
/// `load` is called once and its `removed` flag is the answer.
pub fn removed(source: Option<impl FnOnce() -> bool>, log: impl FnOnce(&'static CStr)) -> bool {
    let _ = (source.is_some(), log);
    todo!("obs_source_removed core (#5587)")
}
