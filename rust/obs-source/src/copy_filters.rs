//! `obs_source_copy_filters()` and `obs_source_copy_single_filter()`: the
//! frontend's Paste Filters and Paste Filter, `obs_scene_duplicate()`'s
//! filter copy, and (through [`duplicate_filters`]) the filter copy of
//! `obs_source_duplicate()`.
//!
//! The duplicate decisions are [`crate::source_duplicate`]'s and the
//! compatibility rule [`crate::source_filter_chain`]'s; this is the C
//! around them: the snapshot of the source's filters under its lock, then
//! for each filter, last to first, a free name, the enabled state, the
//! duplicate, its enabled state, the add and the releases, each through
//! [`FilterCopy`].
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

/// The function a NULL argument of `obs_source_copy_filters()` is logged
/// under.
pub const COPY_FILTERS_FUNCTION: &CStr = c"obs_source_copy_filters";
/// The function a NULL argument of `obs_source_copy_single_filter()` is
/// logged under.
pub const COPY_SINGLE_FILTER_FUNCTION: &CStr = c"obs_source_copy_single_filter";
/// The parameter a NULL destination is logged as.
pub const DST_PARAMETER: &CStr = c"dst";
/// The parameter a NULL source is logged as.
pub const SRC_PARAMETER: &CStr = c"src";
/// The parameter a NULL filter is logged as.
pub const FILTER_PARAMETER: &CStr = c"filter";

/// What the copies ask of libobs, in the order they ask.
pub trait FilterCopy {
    /// A source handle, compared by identity. Passed back as given, so a
    /// handle the host gave (a duplicate) may be its NULL.
    type Source: Copy + PartialEq;
    /// A name from [`Self::new_name`], freed by [`Self::free_name`].
    type Name;
    /// `dst->context.private`.
    fn private(&self, dst: Self::Source) -> bool;
    /// Takes `src->filter_mutex`.
    fn lock(&self, src: Self::Source);
    /// `src->filters.num`, read now.
    fn count(&self, src: Self::Source) -> usize;
    /// `src->filters.array[index]`, read now; `index` is below
    /// [`Self::count`].
    fn filter(&self, src: Self::Source, index: usize) -> Self::Source;
    /// `obs_source_get_ref(filter)`: `None` when the filter is going away.
    fn get_ref(&self, filter: Self::Source) -> Option<Self::Source>;
    /// Releases `src->filter_mutex`.
    fn unlock(&self, src: Self::Source);
    /// `filter_compatible(dst, filter)`.
    fn compatible(&self, dst: Self::Source, filter: Self::Source) -> bool;
    /// `get_new_filter_name(dst, filter->context.name)`.
    fn new_name(&self, dst: Self::Source, filter: Self::Source) -> Self::Name;
    /// `obs_source_enabled(filter)`.
    fn enabled(&self, filter: Self::Source) -> bool;
    /// `obs_source_duplicate(filter, name, private)`.
    fn duplicate(&self, filter: Self::Source, name: &Self::Name, private: bool) -> Self::Source;
    /// `obs_source_set_enabled(copy, enabled)`.
    fn set_enabled(&self, copy: Self::Source, enabled: bool);
    /// `bfree(name)`.
    fn free_name(&self, name: Self::Name);
    /// `obs_source_filter_add(dst, copy)`.
    fn add(&self, dst: Self::Source, copy: Self::Source);
    /// `obs_source_release(source)`.
    fn release(&self, source: Self::Source);
}

/// `obs_source_copy_filters()`: a `None` destination is logged as
/// ([`COPY_FILTERS_FUNCTION`], [`DST_PARAMETER`]), then a `None` source as
/// ([`COPY_FILTERS_FUNCTION`], [`SRC_PARAMETER`]); otherwise
/// [`duplicate_filters`], the copies private exactly when `dst` is.
pub fn copy_filters<H: FilterCopy>(
    host: &H,
    dst: Option<H::Source>,
    src: Option<H::Source>,
    log: impl FnOnce(&'static CStr, &'static CStr),
) {
    let Some(dst) = dst else {
        log(COPY_FILTERS_FUNCTION, DST_PARAMETER);
        return;
    };
    let Some(src) = src else {
        log(COPY_FILTERS_FUNCTION, SRC_PARAMETER);
        return;
    };
    duplicate_filters(host, dst, src, host.private(dst));
}

/// `duplicate_filters()`, the hidden `obs_source_duplicate_filters()`:
/// takes a reference to each of `src`'s filters under its lock (skipping
/// one going away), then after the unlock copies them onto `dst` from the
/// last to the first, each as `private` says.
pub fn duplicate_filters<H: FilterCopy>(host: &H, dst: H::Source, src: H::Source, private: bool) {
    host.lock(src);
    let mut filters = Vec::with_capacity(host.count(src));
    let mut index = 0;
    while index < host.count(src) {
        if let Some(filter) = host.get_ref(host.filter(src, index)) {
            filters.push(filter);
        }
        index = index.wrapping_add(1);
    }
    host.unlock(src);

    for filter in filters.into_iter().rev() {
        copy_one(host, dst, filter, private);
        host.release(filter);
    }
}

/// One filter's copy: a free name, the enabled state, the duplicate, its
/// enabled state, the add, and the copy's release.
fn copy_one<H: FilterCopy>(host: &H, dst: H::Source, filter: H::Source, private: bool) {
    let name = host.new_name(dst, filter);
    let enabled = host.enabled(filter);
    let copy = host.duplicate(filter, &name, private);
    host.set_enabled(copy, enabled);
    host.free_name(name);
    host.add(dst, copy);
    host.release(copy);
}

/// `obs_source_copy_single_filter()`: a `None` destination is logged as
/// ([`COPY_SINGLE_FILTER_FUNCTION`], [`DST_PARAMETER`]), then a `None`
/// filter as ([`COPY_SINGLE_FILTER_FUNCTION`], [`FILTER_PARAMETER`]);
/// otherwise a filter compatible with `dst` is copied onto it once, always
/// private.
pub fn copy_single_filter<H: FilterCopy>(
    host: &H,
    dst: Option<H::Source>,
    filter: Option<H::Source>,
    log: impl FnOnce(&'static CStr, &'static CStr),
) {
    let Some(dst) = dst else {
        log(COPY_SINGLE_FILTER_FUNCTION, DST_PARAMETER);
        return;
    };
    let Some(filter) = filter else {
        log(COPY_SINGLE_FILTER_FUNCTION, FILTER_PARAMETER);
        return;
    };
    if host.compatible(dst, filter) {
        copy_one(host, dst, filter, true);
    }
}
