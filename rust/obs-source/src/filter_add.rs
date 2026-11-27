//! `obs_source_filter_add()`: the filter add behind the filters dialog, the
//! paste of copied filters, `obs_source_copy_single_filter()`, filter loads
//! (`obs_load_sources()`) and obs-websocket's `CreateSourceFilter`.
//!
//! The gates and the new filter's target are [`crate::source_filter_chain`]'s
//! rules; this is the C around them: the lock, the list check, the
//! compatibility flags, the reference, the parent and target writes, the
//! insert at the front, the two signals, the log line and the type's
//! `filter_add` callback, each through [`FilterList`].
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
pub const ADD_FUNCTION: &CStr = c"obs_source_filter_add";
/// The parameter a NULL source is logged as.
pub const SOURCE_PARAMETER: &CStr = c"source";
/// The parameter a NULL filter, or one that can't be referenced, is logged
/// as.
pub const FILTER_PARAMETER: &CStr = c"filter";

/// What the add does with a live source's filter list and the added filter,
/// in the order it asks.
pub trait FilterList {
    /// A source handle (a filter, or what a filter targets), compared by
    /// identity.
    type Source: Copy + PartialEq;
    /// The source itself, what the first filter of an empty list targets.
    fn this(&self) -> Self::Source;
    /// Takes `filter_mutex`.
    fn lock(&self);
    /// `filters.num`, read now.
    fn count(&self) -> usize;
    /// `filters.array[index]`, read now; `index` is below [`Self::count`].
    fn filter(&self, index: usize) -> Self::Source;
    /// Whether `filter->filter_parent` is set.
    fn has_parent(&self, filter: Self::Source) -> bool;
    /// The warning `Tried to add a filter that was already present on the
    /// source`.
    fn warn_present(&self);
    /// `source->owns_info_id`.
    fn owns_info_id(&self) -> bool;
    /// `source->info.output_flags`.
    fn output_flags(&self) -> u32;
    /// `filter->info.output_flags`.
    fn filter_output_flags(&self, filter: Self::Source) -> u32;
    /// `obs_source_get_ref(filter)`: `None` for a filter whose last
    /// reference is gone.
    fn get_ref(&self, filter: Self::Source) -> Option<Self::Source>;
    /// `filter->filter_parent = source`.
    fn set_parent(&self, filter: Self::Source);
    /// `filter->filter_target = target`.
    fn set_target(&self, filter: Self::Source, target: Self::Source);
    /// `da_insert(filters, 0, &filter)`.
    fn insert_front(&self, filter: Self::Source);
    /// Releases `filter_mutex`.
    fn unlock(&self);
    /// The global `source_filter_add` signal, then the source's own
    /// `filter_add` signal, on one calldata holding the source and `filter`.
    fn signal(&self, filter: Self::Source);
    /// The debug line `- filter '<name>' (<id>) added to source '<name>'`.
    fn log_added(&self, filter: Self::Source);
    /// Whether `filter->info.filter_add` is set.
    fn has_add_callback(&self, filter: Self::Source) -> bool;
    /// `filter->info.filter_add(filter->context.data,
    /// filter->filter_parent)`.
    fn call_add_callback(&self, filter: Self::Source);
}

/// `obs_source_filter_add()`.
pub fn add<L: FilterList>(
    source: Option<&L>,
    filter: Option<L::Source>,
    log: impl FnOnce(&'static CStr, &'static CStr),
) {
    let _ = (source, filter, log);
    todo!("#5799")
}
