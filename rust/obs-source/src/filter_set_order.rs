//! `obs_source_filter_set_order()`: the filter move behind the filters
//! dialog's up and down buttons and obs-websocket's filter index helper,
//! with `move_filter_dir()`, its walks `find_next_filter()` and
//! `find_prev_filter()`, and the retarget of every filter after a move.
//!
//! The movement, walk and target rules are [`crate::source_filter_chain`]'s;
//! this is the C around them: the lock, the lookup, the list move, the
//! `filter_target` writes and the `reorder_filters` signal, each through
//! [`FilterChain`].
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
pub const SET_ORDER_FUNCTION: &CStr = c"obs_source_filter_set_order";
/// The parameter a NULL source is logged as.
pub const SOURCE_PARAMETER: &CStr = c"source";
/// The parameter a NULL filter is logged as.
pub const FILTER_PARAMETER: &CStr = c"filter";

/// What the move does with a live source's filter list, in the order it
/// asks.
pub trait FilterChain {
    /// A filter handle, compared by identity.
    type Filter: Copy + PartialEq;
    /// Takes `filter_mutex`.
    fn lock(&self);
    /// `filters.num`, read now.
    fn count(&self) -> usize;
    /// `filters.array[index]`, read now; `index` is below [`Self::count`].
    fn filter(&self, index: usize) -> Self::Filter;
    /// `filter->info.output_flags`, read now.
    fn flags(&self, filter: Self::Filter) -> u32;
    /// `da_move_item(filters, from, to)`; both are below [`Self::count`].
    fn move_filter(&self, from: usize, to: usize);
    /// `filter->filter_target = target`, the source for `None`.
    fn set_target(&self, filter: Self::Filter, target: Option<Self::Filter>);
    /// Releases `filter_mutex`.
    fn unlock(&self);
    /// Signals `reorder_filters` on the source's own handler.
    fn signal(&self);
}

/// `None` is a NULL source, logged as ([`SET_ORDER_FUNCTION`],
/// [`SOURCE_PARAMETER`]); then a `None` filter is logged as
/// ([`SET_ORDER_FUNCTION`], [`FILTER_PARAMETER`]). Otherwise, under the
/// lock, the filter is looked up and moved as `movement` says, every filter
/// retargeted after a move, and `reorder_filters` signalled after the
/// unlock, only when it moved.
pub fn set_order<C: FilterChain>(
    source: Option<&C>,
    filter: Option<C::Filter>,
    movement: i32,
    log: impl FnOnce(&'static CStr, &'static CStr),
) {
    let _ = (source, filter, movement, log);
    todo!("#5775")
}
