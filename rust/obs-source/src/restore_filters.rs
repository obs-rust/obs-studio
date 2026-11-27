//! `obs_source_restore_filters()`: the filter restore behind the filters
//! dialog's undo and redo and the paste of copied filters
//! (`frontend/utility/filter-clipboard.c.h`), the partner of
//! `obs_source_backup_filters()` ([`crate::filter_backup`]).
//!
//! The name match and how a backed-up filter with no current one is loaded
//! are [`crate::source_filter_chain`]'s rules ([`restore_matches`],
//! [`restore_step`]); this is the C around them: the lock, taking the
//! current filters off the list, the array reads, the references and
//! loads, the target and parent writes, the move of the new list into the
//! source and the releases, each through [`RestoreList`].
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

use crate::source_filter_chain::{Restore, restore_matches, restore_step};
use core::ffi::CStr;

/// The public function named in the NULL diagnostics.
pub const RESTORE_FUNCTION: &CStr = c"obs_source_restore_filters";
/// The parameter a NULL source is logged as.
pub const SOURCE_PARAMETER: &CStr = c"source";
/// The parameter a NULL array is logged as.
pub const ARRAY_PARAMETER: &CStr = c"array";

/// What the restore does with a live source's filter list and the
/// backed-up array, in the order it asks.
pub trait RestoreList {
    /// A source handle (a filter, or what a filter targets), compared by
    /// identity.
    type Source: Copy + PartialEq;
    /// One item of the array (an `obs_data_t`), held until
    /// [`Self::release_item`].
    type Item: Copy;
    /// The source itself, what the last restored filter targets and every
    /// restored filter's parent.
    fn this(&self) -> Self::Source;
    /// Takes `filter_mutex`.
    fn lock(&self);
    /// `filters.num`, read now.
    fn count(&self) -> usize;
    /// `filters.array[index]`, read now; `index` is below [`Self::count`].
    fn filter(&self, index: usize) -> Self::Source;
    /// `filter->filter_parent = NULL`, then `filter->filter_target = NULL`.
    fn unlink(&self, filter: Self::Source);
    /// `da_free(source->filters)`.
    fn free_list(&self);
    /// Releases `filter_mutex`.
    fn unlock(&self);
    /// `obs_data_array_count(array)`.
    fn item_count(&self) -> usize;
    /// `obs_data_array_item(array, index)`: a new reference to the item.
    fn item(&self, index: usize) -> Self::Item;
    /// `obs_data_get_string(item, "name")`, without its NUL; `None` for a
    /// NULL string (which `obs_data_get_string()` never returns).
    fn item_name(&self, item: Self::Item) -> Option<&[u8]>;
    /// `filter->context.name`, without its NUL; `None` when unnamed.
    fn name(&self, filter: Self::Source) -> Option<&[u8]>;
    /// `obs_source_get_ref(filter)`: `None` for a filter whose last
    /// reference is gone.
    fn get_ref(&self, filter: Self::Source) -> Option<Self::Source>;
    /// `obs_load_source(item)`: the new filter, holding its reference.
    fn load(&self, item: Self::Item) -> Self::Source;
    /// `obs_load_private_source(item)`: the new filter, holding its
    /// reference.
    fn load_private(&self, item: Self::Item) -> Self::Source;
    /// `filter->filter_target = target`.
    fn set_target(&self, filter: Self::Source, target: Self::Source);
    /// `filter->filter_parent = source`.
    fn set_parent(&self, filter: Self::Source);
    /// `da_move(source->filters, new_filters)`: the source's list becomes
    /// `filters`, which takes over the references the restore holds.
    fn store(&self, filters: Vec<Self::Source>);
    /// `obs_data_release(item)`.
    fn release_item(&self, item: Self::Item);
    /// `obs_source_release(filter)`.
    fn release(&self, filter: Self::Source);
}

/// `obs_source_restore_filters()`: `None` is a NULL source, logged as
/// ([`RESTORE_FUNCTION`], [`SOURCE_PARAMETER`]); then a NULL array
/// (`array` false) is logged as ([`RESTORE_FUNCTION`], [`ARRAY_PARAMETER`]).
/// Under the lock every current filter is kept aside in order, unlinked,
/// and the list freed. With the lock free, each item reuses the first
/// current filter whose name matches (through a new reference) or, when
/// there is none or it can't be referenced, is loaded as the chain rule
/// says; each restored filter becomes the previous one's target and the
/// item is released. The last restored filter targets the source, then
/// every restored filter gets the source as its parent, and the new list
/// goes into the source under the lock. Then each old filter is released
/// once, in old order.
pub fn restore<L: RestoreList>(
    source: Option<&L>,
    array: bool,
    log: impl FnOnce(&'static CStr, &'static CStr),
) {
    let Some(source) = source else {
        log(RESTORE_FUNCTION, SOURCE_PARAMETER);
        return;
    };
    if !array {
        log(RESTORE_FUNCTION, ARRAY_PARAMETER);
        return;
    }

    source.lock();
    let reserve = source.count();
    let mut current = Vec::with_capacity(reserve);
    let mut restored = Vec::with_capacity(reserve);
    let mut index = 0;
    while index < source.count() {
        let filter = source.filter(index);
        current.push(filter);
        source.unlink(filter);
        index = index.wrapping_add(1);
    }
    source.free_list();
    source.unlock();

    let mut prev = None;
    for index in 0..source.item_count() {
        let item = source.item(index);
        let found = reuse(source, &current, source.item_name(item));
        let filter = match (restore_step(found.is_some()), found) {
            (Restore::Reuse, Some(filter)) => filter,
            (Restore::Load, _) => source.load(item),
            (Restore::LoadPrivate | Restore::Reuse, _) => source.load_private(item),
        };
        if let Some(prev) = prev {
            source.set_target(prev, filter);
        }
        prev = Some(filter);
        restored.push(filter);
        source.release_item(item);
    }

    if let Some(prev) = prev {
        source.set_target(prev, source.this());
    }
    for &filter in &restored {
        source.set_parent(filter);
    }

    source.lock();
    source.store(restored);
    source.unlock();

    for filter in current {
        source.release(filter);
    }
}

/// The first current filter whose name matches `name` (a NULL current
/// name never does, nor a NULL item name), referenced: `None` when there
/// is none or it can't be referenced. The search stops at the first match
/// either way.
fn reuse<L: RestoreList>(
    source: &L,
    current: &[L::Source],
    name: Option<&[u8]>,
) -> Option<L::Source> {
    for &filter in current {
        let cur_name = source.name(filter);
        if name.is_some_and(|name| restore_matches(cur_name, name)) {
            return source.get_ref(filter);
        }
    }
    None
}
