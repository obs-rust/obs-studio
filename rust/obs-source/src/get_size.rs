//! `obs_source_get_width()` and `obs_source_get_height()`, the public size
//! of a source. A NULL source logs and answers 0, as does a source with no
//! `context.data`. A filter answers its own base size without a lock; any
//! other source takes its `filter_mutex`, starts at its first filter when it
//! has one ([`crate::size_route::entry`]), reads that base size and unlocks.
//! The base size itself (`obs_source_base_width()` / `_height()`), the
//! private fields and the mutex stay C; the core reaches them through
//! [`Libobs`].

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

/// `obs_source_get_width`, named in its NULL diagnostic.
pub const WIDTH_FUNCTION: &CStr = c"obs_source_get_width";
/// `obs_source_get_height`, named in its NULL diagnostic.
pub const HEIGHT_FUNCTION: &CStr = c"obs_source_get_height";
/// The parameter a NULL source's diagnostic names.
pub const SOURCE: &CStr = c"source";

/// Which of the two calls: the axis it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// `obs_source_get_width()`.
    Width,
    /// `obs_source_get_height()`.
    Height,
}

impl Axis {
    /// The call's name, as its NULL diagnostic gives it.
    pub const fn function(self) -> &'static CStr {
        match self {
            Self::Width => WIDTH_FUNCTION,
            Self::Height => HEIGHT_FUNCTION,
        }
    }
}

/// What the two calls reach in libobs. `Source` is a non-NULL source.
pub trait Libobs {
    /// A source handle.
    type Source: Copy;
    /// `LOG_DEBUG` `"<function>: Null 'source' parameter"`.
    fn log_null(&mut self, function: &'static CStr);
    /// `source->context.data != NULL`.
    fn has_data(&mut self, source: Self::Source) -> bool;
    /// `(int)source->info.type`.
    fn source_type(&mut self, source: Self::Source) -> i32;
    /// `pthread_mutex_lock(&source->filter_mutex)`.
    fn lock_filters(&mut self, source: Self::Source);
    /// `pthread_mutex_unlock(&source->filter_mutex)`.
    fn unlock_filters(&mut self, source: Self::Source);
    /// `source->filters.num != 0`, read under the lock.
    fn has_filters(&mut self, source: Self::Source) -> bool;
    /// `source->filters.array[0]`, read under the lock when it has filters.
    fn first_filter(&mut self, source: Self::Source) -> Self::Source;
    /// `obs_source_base_width(source)` or `obs_source_base_height(source)`.
    fn base_size(&mut self, source: Self::Source, axis: Axis) -> u32;
}

/// `obs_source_get_width()` (`Axis::Width`) or `obs_source_get_height()`
/// (`Axis::Height`) of `source` (`None` for NULL).
pub fn get<L: Libobs>(libobs: &mut L, source: Option<L::Source>, axis: Axis) -> u32 {
    let _ = (libobs, source, axis);
    todo!("#5726")
}
