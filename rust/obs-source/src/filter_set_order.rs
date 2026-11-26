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

use crate::source_filter_chain::{
    Route, Target, edge_move, move_route, same_kind, target_of, walk_step,
};
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
    let Some(source) = source else {
        log(SET_ORDER_FUNCTION, SOURCE_PARAMETER);
        return;
    };
    let Some(filter) = filter else {
        log(SET_ORDER_FUNCTION, FILTER_PARAMETER);
        return;
    };
    source.lock();
    let moved = move_filter_dir(source, filter, movement);
    source.unlock();
    if moved {
        source.signal();
    }
}

/// `move_filter_dir()`: whether the filter moved (and every filter was
/// retargeted).
fn move_filter_dir<C: FilterChain>(source: &C, filter: C::Filter, movement: i32) -> bool {
    let Some(idx) = find(source, filter) else {
        return false;
    };
    let to = match move_route(movement) {
        Route::Up => walk(source, filter, idx, true),
        Route::Down => walk(source, filter, idx, false),
        route @ (Route::Top | Route::Bottom) => edge_move(route == Route::Top, idx, source.count()),
        // The C's answer for a movement outside the enum, which move_route()
        // never gives: no move, but a retarget.
        Route::Retarget => {
            retarget(source);
            return true;
        }
        Route::Stay => None,
    };
    let Some(to) = to else {
        return false;
    };
    source.move_filter(idx, to);
    retarget(source);
    true
}

/// `da_find()`: the first index holding `filter`, the count read before
/// each entry.
fn find<C: FilterChain>(source: &C, filter: C::Filter) -> Option<usize> {
    let mut index = 0;
    while index < source.count() {
        if source.filter(index) == filter {
            return Some(index);
        }
        index = index.wrapping_add(1);
    }
    None
}

/// `find_next_filter()` (`up`) and `find_prev_filter()`: from `idx`, the
/// nearest filter of the same kind as `filter`, which is read again at every
/// step, as the C's recursion did.
fn walk<C: FilterChain>(source: &C, filter: C::Filter, idx: usize, up: bool) -> Option<usize> {
    let mut cur = idx;
    loop {
        let cur_flags = source.flags(filter);
        let next = walk_step(up, cur, source.count())?;
        if same_kind(cur_flags, source.flags(source.filter(next))) {
            return Some(next);
        }
        cur = next;
    }
}

/// The retarget of `reorder_filter_targets()`: each filter, from the first,
/// targets the next one, the last one the source.
fn retarget<C: FilterChain>(source: &C) {
    let mut i = 0;
    while i < source.count() {
        let target = match target_of(i, source.count()) {
            Target::Source => None,
            Target::Filter => Some(source.filter(i.wrapping_add(1))),
        };
        source.set_target(source.filter(i), target);
        i = i.wrapping_add(1);
    }
}
