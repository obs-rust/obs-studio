//! Safe core of `libobs/obs-reset-source-uuids.c.h`: the walk of
//! `obs_reset_source_uuids()` (#5520).
//!
//! Under the sources mutex, held across the whole walk, the core's uuid
//! table is walked from its head in table order with uthash's `HASH_ITER`:
//! each entry's next link is read before the entry is unlinked. Each entry
//! is unlinked from the working table, its uuid freed, a new uuid generated
//! for it and the entry added under that uuid to a new table, which starts
//! empty. After the walk the core's head is replaced once by the new
//! table's (none for an empty table). No validation, reference counting or
//! signal.
//!
//! The tables, their links, the mutex, the allocator and the uuid generator
//! stay in C, behind [`Sources`]; this module decides which reads and calls
//! are made, and in which order.

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

/// The core's uuid table as the original sees it.
pub trait Sources {
    /// A listed context, passed by identity.
    type Entry: Copy;
    /// Locks the sources mutex.
    fn lock(&mut self);
    /// Unlocks it once.
    fn unlock(&mut self);
    /// One read of the core's table head.
    fn head(&mut self) -> Option<Self::Entry>;
    /// One read of `entry`'s next link in its uuid table.
    fn next(&mut self, entry: Self::Entry) -> Option<Self::Entry>;
    /// `HASH_DELETE` of `entry` from the table whose head is `table`.
    fn delete(&mut self, table: &mut Option<Self::Entry>, entry: Self::Entry);
    /// `bfree(entry->uuid)`.
    fn free_uuid(&mut self, entry: Self::Entry);
    /// `entry->uuid = os_generate_uuid()`.
    fn new_uuid(&mut self, entry: Self::Entry);
    /// `HASH_ADD_UUID` of `entry`, under its uuid, to the table whose head
    /// is `table`.
    fn add(&mut self, table: &mut Option<Self::Entry>, entry: Self::Entry);
    /// Stores `table` as the core's head.
    fn store(&mut self, table: Option<Self::Entry>);
}

/// `obs_reset_source_uuids()` over `sources`.
pub fn reset_source_uuids<S: Sources>(sources: &mut S) {
    let _ = sources;
    todo!("obs_reset_source_uuids")
}
