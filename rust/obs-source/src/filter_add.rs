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

use crate::source_filter_chain::{
    RefStep, Target, add_allowed, add_target, checks_compatible, compatible, ref_step,
};
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

/// `obs_source_filter_add()`: `None` is a NULL source, logged as
/// ([`ADD_FUNCTION`], [`SOURCE_PARAMETER`]); then a `None` filter is logged
/// as ([`ADD_FUNCTION`], [`FILTER_PARAMETER`]). Under the lock: a filter on
/// the list or with a parent warns and stops; one whose output flags don't
/// fit the source's stops without a line (unless the source owns its info
/// id); one that can't be referenced is logged like a NULL filter and stops
/// after the unlock (the C kept the lock, #2981). Otherwise the referenced
/// filter gets the source as parent and the old first filter (the source on
/// an empty list) as target and goes to the front of the list. After the
/// unlock: the signals, the log line, then the callback when the type has
/// one.
pub fn add<L: FilterList>(
    source: Option<&L>,
    filter: Option<L::Source>,
    log: impl FnOnce(&'static CStr, &'static CStr),
) {
    let Some(source) = source else {
        log(ADD_FUNCTION, SOURCE_PARAMETER);
        return;
    };
    let Some(filter) = filter else {
        log(ADD_FUNCTION, FILTER_PARAMETER);
        return;
    };
    source.lock();

    let listed = find(source, filter);
    if !add_allowed(listed, source.has_parent(filter)) {
        source.warn_present();
        source.unlock();
        return;
    }

    if checks_compatible(source.owns_info_id())
        && !compatible(source.output_flags(), source.filter_output_flags(filter))
    {
        source.unlock();
        return;
    }

    let referenced = source.get_ref(filter);
    if referenced.is_none() {
        log(ADD_FUNCTION, FILTER_PARAMETER);
    }
    let filter = match (ref_step(referenced.is_some()), referenced) {
        (RefStep::Add, Some(filter)) => filter,
        (RefStep::Return, _) => return,
        (RefStep::UnlockReturn | RefStep::Add, _) => {
            source.unlock();
            return;
        }
    };

    source.set_parent(filter);
    let target = match add_target(source.count()) {
        Target::Source => source.this(),
        Target::Filter => source.filter(0),
    };
    source.set_target(filter, target);
    source.insert_front(filter);
    source.unlock();

    source.signal(filter);
    source.log_added(filter);
    if source.has_add_callback(filter) {
        source.call_add_callback(filter);
    }
}

/// `da_find() != DARRAY_INVALID`: whether `filter` is on the list, the
/// count read before each entry.
fn find<L: FilterList>(source: &L, filter: L::Source) -> bool {
    let mut index = 0;
    while index < source.count() {
        if source.filter(index) == filter {
            return true;
        }
        index = index.wrapping_add(1);
    }
    false
}
