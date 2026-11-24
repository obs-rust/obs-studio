//! Safe core of `libobs/obs-load-sources.c.h`: the two-pass walk of
//! `obs_load_sources()` (#5574), the scene-collection load every frontend
//! runs at startup and on every collection switch.
//!
//! The array's count is read once. Pass 1, for each entry in array order:
//! its item taken, the source loaded from it (a NULL result kept in its
//! slot), the item released. Pass 2, for each slot in order: the item taken
//! again; for a non-NULL source, the transition load first when its type is
//! a transition (the type read here, after every earlier source's load), then
//! the second-pass load, then the caller's callback when there is one; the
//! item released, NULL slot or not. Pass 3: every slot released in order,
//! NULL ones included, as `obs_source_release(NULL)` is called in C.
//!
//! The data array, the source loads and the source and data references stay
//! in C, behind [`Loader`]; this module decides which calls are made, and in
//! which order.

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

/// The data array and the source calls, as the original sees them.
pub trait Loader {
    /// The array being loaded, passed by identity.
    type Array: Copy;
    /// One entry's data, passed by identity (NULL included).
    type Data: Copy;
    /// A loaded source, passed by identity.
    type Source: Copy;
    /// `obs_data_array_count(array)`.
    fn count(&mut self, array: Self::Array) -> usize;
    /// `obs_data_array_item(array, index)`: a new reference.
    fn item(&mut self, array: Self::Array, index: usize) -> Self::Data;
    /// `obs_data_release(data)`.
    fn release_data(&mut self, data: Self::Data);
    /// `obs_load_source(data)`: `None` for NULL.
    fn load(&mut self, data: Self::Data) -> Option<Self::Source>;
    /// One read of whether `source`'s type is `OBS_SOURCE_TYPE_TRANSITION`.
    fn is_transition(&mut self, source: Self::Source) -> bool;
    /// `obs_transition_load(source, data)`.
    fn transition_load(&mut self, source: Self::Source, data: Self::Data);
    /// `obs_source_load2(source)`.
    fn load2(&mut self, source: Self::Source);
    /// `obs_source_release(source)`, NULL for `None`.
    fn release_source(&mut self, source: Option<Self::Source>);
}

/// `obs_load_sources(array, cb, private_data)` over `loader`, with `cb` the
/// caller's callback bound to its private data (`None` for a NULL `cb`).
pub fn load_sources<L, F>(loader: &mut L, array: L::Array, cb: Option<F>)
where
    L: Loader,
    F: FnMut(L::Source),
{
    let _ = (loader, array, cb);
    todo!("obs_load_sources")
}
