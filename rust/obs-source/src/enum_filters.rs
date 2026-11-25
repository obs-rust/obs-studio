//! `obs_source_enum_filters()`: the public walk over a source's filter list,
//! which the filters dialog, the scene collection log, the missing-files
//! list, the scripting wrappers and filter duplication use.
use core::ffi::CStr;

/// The public function named in the NULL diagnostics.
pub const ENUM_FILTERS_FUNCTION: &CStr = c"obs_source_enum_filters";
/// The parameter a NULL source is logged as.
pub const SOURCE_PARAMETER: &CStr = c"source";
/// The parameter a NULL callback is logged as: upstream's `obs_ptr_valid()`
/// stringizes the union member it was handed (kept).
pub const CALLBACK_PARAMETER: &CStr = c"callback_obj.obj";

/// What the walk does with a live source's filter list, in the order it
/// asks.
pub trait FilterList {
    /// A filter handle.
    type Filter: Copy;
    /// Takes `filter_mutex`.
    fn lock(&self);
    /// `filters.num`, read now.
    fn count(&self) -> usize;
    /// `filters.array[index]`, read now; `index` is below a count read
    /// under the lock.
    fn filter(&self, index: usize) -> Self::Filter;
    /// Releases `filter_mutex`.
    fn unlock(&self);
}

/// `None` is a NULL source, logged as ([`ENUM_FILTERS_FUNCTION`],
/// [`SOURCE_PARAMETER`]); then a `None` callback is logged as
/// ([`ENUM_FILTERS_FUNCTION`], [`CALLBACK_PARAMETER`]). Otherwise, under the
/// lock, the count is read once and the callback gets every filter from the
/// last index to the first, each entry read when it is visited.
pub fn enum_filters<L: FilterList>(
    source: Option<&L>,
    callback: Option<impl FnMut(L::Filter)>,
    log: impl FnOnce(&'static CStr, &'static CStr),
) {
    let Some(source) = source else {
        log(ENUM_FILTERS_FUNCTION, SOURCE_PARAMETER);
        return;
    };
    let Some(mut callback) = callback else {
        log(ENUM_FILTERS_FUNCTION, CALLBACK_PARAMETER);
        return;
    };
    source.lock();
    for index in (0..source.count()).rev() {
        callback(source.filter(index));
    }
    source.unlock();
}
