//! Safe core of `libobs/obs-save-sources.c.h`: the walk and selection of
//! `obs_save_sources_filtered()` and `obs_save_sources()` (#5567), the
//! scene-collection save every frontend uses.
//!
//! A new array first, outside the lock. Then, under the sources mutex, held
//! across the whole walk, the core's uuid table is walked from its head in
//! table order. For each source the C's checks run in the C's order, each
//! one only when the ones before it passed: not a filter, not removed, not
//! temp-removed (hidden), not private, and then the caller's callback. A
//! source the callback keeps is saved, its save pushed into the array and
//! then released. The next link is read after the source is handled. After
//! the walk the mutex is unlocked once and the array returned; the caller
//! owns it. No reference is taken or dropped on any source.
//!
//! The table, its links, the mutex, the source fields and the data calls stay
//! in C, behind [`Sources`]; this module decides which reads and calls are
//! made, and in which order.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// The core's source table and the data calls, as the original sees them.
pub trait Sources {
    /// A listed source, passed by identity.
    type Source: Copy;
    /// A saved source's data, passed by identity.
    type Data: Copy;
    /// The array the save returns, passed by identity.
    type Array: Copy;
    /// `obs_data_array_create()`.
    fn array_create(&mut self) -> Self::Array;
    /// Locks the sources mutex.
    fn lock(&mut self);
    /// Unlocks it once.
    fn unlock(&mut self);
    /// One read of the core's table head.
    fn first(&mut self) -> Option<Self::Source>;
    /// One read of `source`'s next link in the uuid table.
    fn next(&mut self, source: Self::Source) -> Option<Self::Source>;
    /// One read of whether `source`'s type is `OBS_SOURCE_TYPE_FILTER`.
    fn is_filter(&mut self, source: Self::Source) -> bool;
    /// One read of `source->removed`.
    fn removed(&mut self, source: Self::Source) -> bool;
    /// One read of `source->temp_removed`.
    fn temp_removed(&mut self, source: Self::Source) -> bool;
    /// One read of `source->context.private`.
    fn private(&mut self, source: Self::Source) -> bool;
    /// `obs_save_source(source)`.
    fn save(&mut self, source: Self::Source) -> Self::Data;
    /// `obs_data_array_push_back(array, data)`.
    fn push(&mut self, array: Self::Array, data: Self::Data);
    /// `obs_data_release(data)`.
    fn release(&mut self, data: Self::Data);
}

/// `obs_save_sources_filtered(cb, param)` over `sources`, with `keep` the
/// caller's callback bound to its parameter.
pub fn save_sources_filtered<S, F>(sources: &mut S, keep: F) -> S::Array
where
    S: Sources,
    F: FnMut(S::Source) -> bool,
{
    let _ = (sources, keep);
    todo!("obs_save_sources_filtered")
}

/// `obs_save_sources()` over `sources`: every candidate kept.
pub fn save_sources<S: Sources>(sources: &mut S) -> S::Array {
    let _ = sources;
    todo!("obs_save_sources")
}
