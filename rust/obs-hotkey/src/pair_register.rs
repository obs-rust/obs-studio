//! Admission and owner selection for the four object-bound hotkey-pair
//! registration APIs. The unchanged C pair engine keeps locking, allocation,
//! weak-owner acquisition, saved bindings, partner IDs and teardown.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

pub use crate::register::Kind;

/// `OBS_INVALID_HOTKEY_PAIR_ID` from the unchanged public header.
pub const INVALID_PAIR_ID: usize = usize::MAX;

/// The existing pair engine and owner adapters.
pub trait Registry<R> {
    type Owner: Copy;
    /// The owner kind's weak-reference helper and its context, read from the
    /// real owner layout.
    type Fields;
    fn fields(&mut self, kind: Kind, owner: Self::Owner) -> Self::Fields;
    /// `register_hotkey_pair_internal` with the owner itself as registerer.
    fn register(
        &mut self,
        kind: Kind,
        owner: Self::Owner,
        fields: Self::Fields,
        request: R,
    ) -> usize;
}

/// Register a pair for a non-null owner. A NULL owner returns the invalid
/// pair ID before any owner field is read and without reaching the engine.
pub fn register<B: Registry<R>, R>(
    registry: &mut B,
    kind: Kind,
    owner: Option<B::Owner>,
    request: R,
) -> usize {
    let _ = (registry, kind, owner, request);
    todo!("port object-bound hotkey-pair registration")
}
