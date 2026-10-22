//! Safe core of `libobs/obs-nix-portal-shortcut-sets.c.h`: the combination
//! set logic of the xdg-desktop-portal GlobalShortcuts client.
//!
//! On every sync of the hotkey bindings the portal client
//! (`obs-nix-portal-shortcuts.c`, Wayland with GIO, Linux and FreeBSD, still
//! C) asks this code:
//! - [`unchanged`]: whether the portal session stays as it is, or is bound
//!   again;
//! - [`request`]: which combinations the new bind asks the desktop for;
//! - [`partition`]: after the desktop's dialog is declined, which
//!   combinations are bound again (they were approved before) and which are
//!   declined;
//! - [`has`]: whether a set holds a combination (the Activated handler).
//!
//! It only reads the caller's arrays and returns positions in them, so the
//! C portal client keeps every allocation. A combination is any
//! [`Combination`] (the safe [`KeyCombination`], or the C struct in the
//! shim), and a description is any `T: PartialEq`, `None` for a NULL one;
//! the shim compares C strings as bytes, as `strcmp` does.
//!
//! Characterized, not endorsed (`test_portal_shortcut_sets`), and kept as
//! the C has them:
//! - the same set compares combinations as sets, so `{A, A}` is the same as
//!   `{A}`, and each copy is compared with the description of the first
//!   copy on the other side;
//! - a NULL description is not the same as another NULL;
//! - the partition counts duplicates in |part| and takes |request| as
//!   given, so two copies of an approved combination can stop the partial
//!   rebind.
//!
//! Every position written goes through a bounds check: the caller gives
//! room for `current.len()` positions, and a shorter buffer only loses the
//! positions past its end (C writes past it). The counts never depend on
//! the room.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

pub use crate::nix_hotkey_state::KeyCombination;

/// An element of a combination array: a key and its `INTERACT_*` bits.
pub trait Combination: Copy {
    /// The combination it holds.
    fn combination(self) -> KeyCombination;
}

impl Combination for KeyCombination {
    fn combination(self) -> KeyCombination {
        self
    }
}

/// `same_combo()` (`obs-nix-portal-shortcuts.h`): the same key and the same
/// modifier bits, every bit, not only the four the portal knows.
pub fn same_combo(_a: KeyCombination, _b: KeyCombination) -> bool {
    todo!()
}

/// Whether `combos` holds `combo` (`obs_nix_portal_sets_has`).
pub fn has<C: Combination>(_combos: &[C], _combo: KeyCombination) -> bool {
    todo!()
}

/// `obs_nix_portal_sets_unchanged`: whether a sync leaves the portal session
/// as it is.
#[allow(clippy::too_many_arguments)]
pub fn unchanged<C: Combination, T: PartialEq>(
    _session: bool,
    _retry: bool,
    _partly_declined: bool,
    _current: &[C],
    _current_descriptions: &[Option<T>],
    _bound: &[C],
    _bound_descriptions: &[Option<T>],
    _declined: &[C],
) -> bool {
    todo!()
}

/// What [`request`] selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    /// The number of positions in the request.
    pub num: usize,
    /// It is only part of `current`.
    pub partly: bool,
}

/// `obs_nix_portal_sets_request`: the positions of the combinations of
/// `current` to ask the desktop for.
pub fn request<C: Combination>(
    _current: &[C],
    _partly_declined: bool,
    _declined: &[C],
    _request: &mut [usize],
) -> Request {
    todo!()
}

/// What [`partition`] selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Partition {
    /// The number of positions in part.
    pub num_part: usize,
    /// The number of positions in declined.
    pub num_declined: usize,
    /// Part is bound again: it is neither empty nor as large as the request.
    pub bind: bool,
}

/// `obs_nix_portal_sets_partition`: what is bound again after the desktop
/// declined `request`, and what is declined.
pub fn partition<C: Combination>(
    _current: &[C],
    _approved: &[C],
    _request: &[C],
    _part: &mut [usize],
    _declined: &mut [usize],
) -> Partition {
    todo!()
}
