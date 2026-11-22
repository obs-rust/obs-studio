//! Safe core of `libobs/obs-enum-contexts.c.h`: the snapshot walk shared by
//! `obs_enum_outputs()`, `obs_enum_encoders()` and `obs_enum_services()`.
//!
//! Under the kind's list mutex the list is walked from its head along each
//! entry's `next`, a reference is taken on every entry with the kind's
//! `obs_*_get_ref` and only the entries that returned one are kept, in list
//! order; the mutex is released. Then, for each kept entry in order, the
//! mutex is taken again around one call of the caller's callback, and the
//! walk stops after the first callback that answers false. Last, every kept
//! reference is released with the kind's `obs_*_release`, in snapshot order,
//! with the walk's own hold of the mutex released. The list mutex is
//! re-entrant: the callback may enumerate the same kind again, create an
//! entry (which the walk in progress does not see) or release its own
//! reference to the current entry (which the snapshot keeps alive until the
//! end of the walk). Nothing else is written.

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

/// One kind's context list as the walk sees it: its mutex, its head, each
/// entry's `next`, the kind's reference functions and the caller's callback.
pub trait Contexts {
    /// A `struct obs_context_data *` of the kind, passed by identity.
    type Entry: Copy;
    /// Locks the kind's list mutex (re-entrant for this thread).
    fn lock(&mut self);
    /// Unlocks it once.
    fn unlock(&mut self);
    /// One read of the list head (`obs->data.first_*`), under the lock.
    fn first(&mut self) -> Option<Self::Entry>;
    /// One read of `entry->next`, under the lock.
    fn next(&mut self, entry: Self::Entry) -> Option<Self::Entry>;
    /// The kind's `obs_*_get_ref(entry)`: the new reference, or `None` for an
    /// entry already being destroyed.
    fn get_ref(&mut self, entry: Self::Entry) -> Option<Self::Entry>;
    /// The caller's callback with its `param` and the entry, under the lock;
    /// `false` stops the walk.
    fn visit(&mut self, entry: Self::Entry) -> bool;
    /// The kind's `obs_*_release(entry)`.
    fn release(&mut self, entry: Self::Entry);
}

/// Walks a referenced snapshot of the kind's list, as `obs_enum()` did.
pub fn enum_contexts<C: Contexts>(contexts: &mut C) {
    let _ = contexts;
    todo!("obs_enum snapshot walk")
}
