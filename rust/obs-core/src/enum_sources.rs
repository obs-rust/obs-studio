//! Safe core of `libobs/obs-enum-sources.c.h`: the walks of
//! `obs_enum_sources()`, `obs_enum_all_sources()`, `obs_enum_canvases()` and
//! `obs_canvas_enum_scenes()` (#5495).
//!
//! - [`enum_sources`]: under the source table's mutex, walks the table in
//!   uuid order from its head; takes a reference on each entry and passes
//!   over one that has none left; skips a private source; calls back for an
//!   input, then (unless that call answered false) for a source whose id is
//!   the group's; a false answer releases the reference and stops the walk.
//!   Every reference taken is released before the next link is read.
//! - [`enum_uuid`] (`obs_enum_all_sources()`): under the mutex, every entry
//!   of the table, no reference taken; each entry's next link is read before
//!   its callback (uthash's `HASH_ITER`), so the callback may unlink the
//!   current entry. A false answer stops it.
//! - [`enum_canvases`]: as [`enum_uuid`] over the named canvases, except the
//!   head is read before the canvases' mutex is taken, as the original does.
//! - [`canvas_enum_scenes`]: under the canvas's own sources mutex, the
//!   canvas's sources in name-table order, each referenced for the call and
//!   released after; only a listed entry of scene type is called back; a
//!   false answer releases and stops.
//!
//! The tables, their links, the mutexes, the reference counting and the
//! source fields stay in C, behind the traits below; this module decides
//! which reads and calls are made, and in which order.

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

/// One hash table walk as the original sees it: its mutex, its head, each
/// entry's next link and the caller's callback.
pub trait Table {
    /// A listed entry, passed by identity.
    type Entry: Copy;
    /// Locks the table's mutex (re-entrant for this thread).
    fn lock(&mut self);
    /// Unlocks it once.
    fn unlock(&mut self);
    /// One read of the table's head.
    fn first(&mut self) -> Option<Self::Entry>;
    /// One read of `entry`'s next link.
    fn next(&mut self, entry: Self::Entry) -> Option<Self::Entry>;
    /// The caller's callback with its `param` and the entry; `false` stops
    /// the walk.
    fn visit(&mut self, entry: Self::Entry) -> bool;
}

/// A table whose entries are sources, referenced for the callback.
pub trait Referenced: Table {
    /// `obs_source_get_ref(entry)`: the new reference, or `None` for a
    /// source with no reference left.
    fn get_ref(&mut self, entry: Self::Entry) -> Option<Self::Entry>;
    /// `obs_source_release(entry)`.
    fn release(&mut self, entry: Self::Entry);
}

/// The source table as [`enum_sources`] reads it.
pub trait Sources: Referenced {
    /// `source->context.private`.
    fn private(&mut self, source: Self::Entry) -> bool;
    /// `source->info.type == OBS_SOURCE_TYPE_INPUT`.
    fn input(&mut self, source: Self::Entry) -> bool;
    /// `strcmp(source->info.id, group_info.id) == 0`.
    fn group(&mut self, source: Self::Entry) -> bool;
}

/// A canvas's sources as [`canvas_enum_scenes`] reads them.
pub trait Scenes: Referenced {
    /// `source->info.type == OBS_SOURCE_TYPE_SCENE`.
    fn scene(&mut self, source: Self::Entry) -> bool;
}

/// `obs_enum_sources()` over `sources`.
pub fn enum_sources<S: Sources>(sources: &mut S) {
    let _ = sources;
    todo!()
}

/// `obs_enum_all_sources()`: the static `obs_enum_uuid()` over `table`.
pub fn enum_uuid<T: Table>(table: &mut T) {
    let _ = table;
    todo!()
}

/// `obs_enum_canvases()` over the named canvases.
pub fn enum_canvases<T: Table>(canvases: &mut T) {
    let _ = canvases;
    todo!()
}

/// `obs_canvas_enum_scenes()` over one canvas's sources.
pub fn canvas_enum_scenes<S: Scenes>(scenes: &mut S) {
    let _ = scenes;
    todo!()
}
