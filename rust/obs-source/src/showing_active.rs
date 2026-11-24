//! `obs_source_active()` and `obs_source_showing()`: whether a source's
//! activate or show reference count is nonzero.
use core::ffi::CStr;

/// The counter a predicate reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Counter {
    /// `activate_refs`, read by `obs_source_active()`.
    Activate,
    /// `show_refs`, read by `obs_source_showing()`.
    Show,
}

impl Counter {
    /// The public function that reads this counter, named in its NULL
    /// diagnostic.
    pub fn function(self) -> &'static CStr {
        todo!()
    }
}

/// `None` is a NULL source: it logs once with the function's name and
/// answers false without reading anything. Otherwise `load` is called once,
/// with this counter only, and the answer is whether the value is nonzero.
pub fn nonzero(
    counter: Counter,
    source: Option<impl FnOnce(Counter) -> i64>,
    log: impl FnOnce(&'static CStr),
) -> bool {
    let _ = (counter, source, log);
    todo!()
}
