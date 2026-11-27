//! `obs_source_remove()`: the removal behind the frontend's source and scene
//! deletion, obs-websocket's `RemoveInput`/`RemoveScene` and most tests'
//! teardown.
//!
//! The C around it stays behind [`Source`]: the atomic `removed` flag, the
//! reference, `obs_source_dosignal()`, the canvas and private reads,
//! `obs_canvas_remove_source()` and the release. The `removed` check is a
//! plain load and the set a later separate store, as the C does them (no
//! compare-and-swap).
#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use core::ffi::CStr;

/// The public function named in the NULL diagnostic.
pub const REMOVE_FUNCTION: &CStr = c"obs_source_remove";
/// The parameter a NULL source is logged as.
pub const SOURCE_PARAMETER: &CStr = c"source";

/// What the removal reads, writes and calls, in the order it asks. `self` is
/// the source passed in; [`Self::Ref`] the reference `obs_source_get_ref()`
/// returned for it.
pub trait Source {
    /// A source reference, the handle the later steps act on.
    type Ref: Copy;
    /// `os_atomic_load_bool(&source->removed)`.
    fn removed(&self) -> bool;
    /// `obs_source_get_ref(source)`; `None` for NULL (the last reference is
    /// gone).
    fn get_ref(&self) -> Option<Self::Ref>;
    /// `os_atomic_set_bool(&s->removed, true)`.
    fn set_removed(&self, s: Self::Ref);
    /// `obs_source_dosignal(s, "source_remove", "remove")`.
    fn signal(&self, s: Self::Ref);
    /// `source->canvas != NULL`, read from the source passed in.
    fn has_canvas(&self) -> bool;
    /// `source->context.private`, read from the source passed in.
    fn is_private(&self) -> bool;
    /// `obs_canvas_remove_source(s)`.
    fn remove_from_canvas(&self, s: Self::Ref);
    /// `obs_source_release(s)`.
    fn release(&self, s: Self::Ref);
}

/// `obs_source_remove()`: `None` is a NULL source, logged as
/// ([`REMOVE_FUNCTION`], [`SOURCE_PARAMETER`]). A source already removed is
/// left alone. Otherwise one reference; without one nothing else happens.
/// With it: marked removed, signalled, taken off its canvas when it has one
/// and is not private (the private flag read only when there is a canvas),
/// and the reference released.
pub fn remove<S: Source>(source: Option<&S>, log: impl FnOnce(&'static CStr, &'static CStr)) {
    let _ = (source, log);
    todo!("#5803")
}
