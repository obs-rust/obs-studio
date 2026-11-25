//! Missing-file replacement dispatch to a source's private data.
use core::ffi::CStr;

/// What `obs_source_replace_missing_file()` sees of its source: absent
/// (`None`), present without private data (`Some(None)`), or present with it.
pub type Source<D> = Option<Option<D>>;

/// Calls `call` with the source's private data, once. A NULL source logs
/// the function name instead; a source without private data does nothing.
pub fn replace<D>(source: Source<D>, call: impl FnOnce(D), log: impl FnOnce(&'static CStr)) {
    let _ = (source, call, log);
    todo!("#5635")
}
