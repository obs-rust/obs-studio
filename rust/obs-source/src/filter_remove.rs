//! `obs_source_filter_remove()`: the filter removal behind the filters
//! dialog's remove button, obs-websocket's `RemoveSourceFilter` and the
//! filter drain of `obs_source_destroy()`, with its step
//! `obs_source_filter_remove_refless()` (hidden; `obs_source_destroy()` also
//! calls it for a filter leaving its parent).
//!
//! The retarget rule is [`crate::source_filter_chain`]'s; this is the C
//! around it: the lock, the lookup, the target hand-over, the erase, the two
//! signals, the log line, the type's `filter_remove` callback, the cleared
//! links and the release, each through [`FilterList`].
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

/// The public function named in the NULL diagnostics.
pub const REMOVE_FUNCTION: &CStr = c"obs_source_filter_remove";
/// The parameter a NULL source is logged as.
pub const SOURCE_PARAMETER: &CStr = c"source";
/// The parameter a NULL filter is logged as.
pub const FILTER_PARAMETER: &CStr = c"filter";

/// What the removal does with a live source's filter list and the removed
/// filter, in the order it asks.
pub trait FilterList {
    /// A source handle (a filter, or what a filter targets), compared by
    /// identity.
    type Source: Copy + PartialEq;
    /// Takes `filter_mutex`.
    fn lock(&self);
    /// `filters.num`, read now.
    fn count(&self) -> usize;
    /// `filters.array[index]`, read now; `index` is below [`Self::count`].
    fn filter(&self, index: usize) -> Self::Source;
    /// `filter->filter_target`, read now.
    fn target(&self, filter: Self::Source) -> Self::Source;
    /// `filter->filter_target = target`.
    fn set_target(&self, filter: Self::Source, target: Self::Source);
    /// `da_erase(filters, index)`; `index` is below [`Self::count`].
    fn erase(&self, index: usize);
    /// Releases `filter_mutex`.
    fn unlock(&self);
    /// The global `source_filter_remove` signal, then the source's own
    /// `filter_remove` signal, on one calldata holding the source and
    /// `filter`.
    fn signal(&self, filter: Self::Source);
    /// The debug line `- filter '<name>' (<id>) removed from source
    /// '<name>'`.
    fn log_removed(&self, filter: Self::Source);
    /// Whether `filter->info.filter_remove` is set.
    fn has_remove_callback(&self, filter: Self::Source) -> bool;
    /// `filter->info.filter_remove(filter->context.data,
    /// filter->filter_parent)`.
    fn call_remove_callback(&self, filter: Self::Source);
    /// `filter->filter_parent = NULL`, then `filter->filter_target = NULL`.
    fn clear_links(&self, filter: Self::Source);
    /// `obs_source_release(filter)`.
    fn release(&self, filter: Self::Source);
}

/// `obs_source_filter_remove()`: `None` is a NULL source, logged as
/// ([`REMOVE_FUNCTION`], [`SOURCE_PARAMETER`]); then a `None` filter is
/// logged as ([`REMOVE_FUNCTION`], [`FILTER_PARAMETER`]). Otherwise
/// [`remove_refless`], and the list's reference is released when the filter
/// was on it.
pub fn remove<L: FilterList>(
    source: Option<&L>,
    filter: Option<L::Source>,
    log: impl FnOnce(&'static CStr, &'static CStr),
) {
    let _ = (source, filter, log);
    todo!("#5795")
}

/// `obs_source_filter_remove_refless()`: under the lock, looks `filter` up
/// (not listed: unlock, `false`); the entry before it, if any, takes its
/// target; the entry is erased. After the unlock: the signals, the log line,
/// the callback when the type has one, then the filter's parent and target
/// cleared. `true` when it was removed; the reference is the caller's.
pub fn remove_refless<L: FilterList>(source: &L, filter: L::Source) -> bool {
    let _ = (source, filter);
    todo!("#5795")
}
