//! Safe core of `libobs/obs-source-transition-load.c.h`: the load, scale
//! type and swap checks of the transitions in
//! `libobs/obs-source-transition.c.h`.
//!
//! - [`is_transition`]: the type check of `transition_valid()`, after
//!   `obs_ptr_valid()`;
//! - [`scale_type`]: the scale type rule of `obs_transition_set_scale_type()`
//!   and `obs_transition_load()`;
//! - [`looks_up`] and [`source`]: the source steps of
//!   `obs_transition_load()` (the lookup with a name, then keep, not found
//!   or the recursion refusal);
//! - [`swaps`]: the same-transition gate of `obs_transition_swap_begin()`
//!   and `obs_transition_swap_end()`;
//! - [`copy_stores`]: the recursion refusal of `copy_source_state()`;
//! - [`load`]: `obs_transition_load()` itself, from
//!   `libobs/obs-source-transition-restore.c.h`, generic over a
//!   [`Transition`] (one method per `obs_data` read, field write, lock call
//!   and libobs call, in the C's order and under its lock), with its C shim
//!   in the `obs-source-seam` crate.
//!
//! The decisions above take no lock and call nothing in libobs: each is its
//! own function, asked at its own point, so the C callers that remain keep
//! every read and call where it was and
//! under its condition: `obs_ptr_valid()`, the locks and their order, the
//! pointer comparisons, the `obs_data` reads, the lookup, the add only for a
//! found source or an active new child, every `obs_source_get_ref()` and
//! `obs_source_release()`, the active child calls, the `(uint32_t)`
//! truncations of the loaded alignment and size, the mode taken as is, the
//! fields the swap hands over, the texrender swap, the size and matrix
//! recalculation and the logs. The getters' answers for an invalid
//! transition stay C literals. Whether the load takes the old source A out
//! of the active children is [`crate::transition_start::child_active`].
//! Nothing here keeps state.
//!
//! Characterized, not endorsed (`test_transition_load_steps`), and kept as
//! the C has it: the loaded alignment, width and height are truncated to
//! their low 32 bits, and the loaded mode to an `int`.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use std::ffi::CStr;

/// `OBS_SOURCE_TYPE_TRANSITION` (`enum obs_source_type`).
pub const TYPE_TRANSITION: i32 = 2;

/// `OBS_TRANSITION_SCALE_MAX_ONLY`, the same constant
/// [`crate::transition_clock`] uses.
pub const SCALE_MAX_ONLY: i32 = crate::transition_clock::SCALE_MAX_ONLY;
/// `OBS_TRANSITION_SCALE_ASPECT`.
pub const SCALE_ASPECT: i32 = crate::transition_clock::SCALE_ASPECT;
/// `OBS_TRANSITION_SCALE_STRETCH`.
pub const SCALE_STRETCH: i32 = crate::transition_clock::SCALE_STRETCH;

/// `enum obs_source_transition_load_outcome`: what `obs_transition_load()`
/// does with the source it looked up. The discriminants are the C's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    /// Store it in slot A.
    Keep = 0,
    /// Warn that it was not found, and store none.
    NotFound = 1,
    /// Warn of the recursion, release it and store none: the add failed.
    Refuse = 2,
}

/// Whether a source of this type is a transition:
/// `type == OBS_SOURCE_TYPE_TRANSITION`.
#[must_use]
pub const fn is_transition(source_type: i32) -> bool {
    source_type == TYPE_TRANSITION
}

/// A scale type as stored: [`SCALE_MAX_ONLY`], [`SCALE_ASPECT`] and
/// [`SCALE_STRETCH`] as they are, any other value [`SCALE_MAX_ONLY`]. The
/// value is compared in 64 bits, as `obs_data` holds it.
#[must_use]
pub const fn scale_type(value: i64) -> i32 {
    // The whole i64 is matched, so 2^32 + 1 is not Aspect; 0 is Max Only
    // either way.
    match value {
        1 => SCALE_ASPECT,
        2 => SCALE_STRETCH,
        _ => SCALE_MAX_ONLY,
    }
}

/// Whether `obs_transition_load()` looks its saved source up: it has a
/// name.
#[must_use]
pub const fn looks_up(has_name: bool) -> bool {
    has_name
}

/// What `obs_transition_load()` does with the looked-up source, after the
/// add (asked only for a found source): [`Source::NotFound`] when none was
/// found, [`Source::Refuse`] when the add failed, else [`Source::Keep`].
#[must_use]
pub const fn source(found: bool, add_ok: bool) -> Source {
    if !found {
        Source::NotFound
    } else if !add_ok {
        Source::Refuse
    } else {
        Source::Keep
    }
}

/// Whether `obs_transition_swap_begin()` and `obs_transition_swap_end()`
/// do anything: not for a transition swapped with itself.
#[must_use]
pub const fn swaps(same: bool) -> bool {
    !same
}

/// Whether `copy_source_state()` stores the new child: it was not to be
/// added as an active child, or the add worked.
#[must_use]
pub const fn copy_stores(adds: bool, add_ok: bool) -> bool {
    !adds || add_ok
}

/// The saved key of the source in slot A.
pub const KEY_SOURCE_A: &CStr = c"transition_source_a";
/// The saved key of the alignment.
pub const KEY_ALIGNMENT: &CStr = c"transition_alignment";
/// The saved key of the mode.
pub const KEY_MODE: &CStr = c"transition_mode";
/// The saved key of the scale type.
pub const KEY_SCALE_TYPE: &CStr = c"transition_scale_type";
/// The saved key of the width.
pub const KEY_CX: &CStr = c"transition_cx";
/// The saved key of the height.
pub const KEY_CY: &CStr = c"transition_cy";

/// The warning [`load`] logs (`LOG_WARNING`) when the transition refuses
/// the saved source as an active child: the format, with the transition's
/// name (`context.name`), then the source's.
pub const RECURSION_WARNING: &CStr =
    c"Cannot set transition '%s' to source '%s' due to infinite recursion";
/// The warning [`load`] logs (`LOG_WARNING`) when no source has the saved
/// name: the format, with the name, then the transition's.
pub const NOT_FOUND_WARNING: &CStr = c"Failed to find source '%s' for transition '%s'";

/// The fields, locks and libobs calls of a transition that its load uses,
/// on top of the size and matrix recalculation's
/// ([`crate::transition_size::Transition`]): in libobs, the transition seam
/// (`obs-source-transition-seam.h`), the `obs_data` reads and the source
/// functions the C calls, one call per method.
pub trait Transition: crate::transition_size::Transition {
    /// An `obs_data_t *` of the saved transition.
    type Data;
    /// A `const char *` read from the saved data.
    type Name: Copy;

    /// `obs_data_get_string(data, key)`, `None` for `NULL`.
    fn get_string(&self, data: &Self::Data, key: &'static CStr) -> Option<Self::Name>;
    /// `obs_data_get_int(data, key)`.
    fn get_int(&self, data: &Self::Data, key: &'static CStr) -> i64;
    /// `obs_get_source_by_name(name)`, with the reference it takes; `None`
    /// when no source has the name.
    fn source_by_name(&self, name: Self::Name) -> Option<Self::Child>;
    /// `obs_source_add_active_child(self, child)`: false when `child` is
    /// `self` or contains it.
    fn add_active_child(&self, child: Self::Child) -> bool;
    /// `obs_source_remove_active_child(self, child)`.
    fn remove_active_child(&self, child: Self::Child);
    /// `obs_source_release(child)`, which does nothing for `None`.
    fn release(&self, child: Option<Self::Child>);
    /// Logs [`RECURSION_WARNING`] with `self`'s `context.name` and `name`.
    fn warn_recursion(&self, name: Self::Name);
    /// Logs [`NOT_FOUND_WARNING`] with `name` and `self`'s `context.name`.
    fn warn_not_found(&self, name: Self::Name);

    /// Writes `transition_sources[idx]`.
    fn set_source(&self, idx: usize, value: Option<Self::Child>);
    /// `transition_source_active[idx]`.
    fn source_active(&self, idx: usize) -> bool;
    /// Writes `transition_source_active[idx]`.
    fn set_source_active(&self, idx: usize, value: bool);
    /// Writes `transition_alignment`.
    fn set_alignment(&self, value: u32);
    /// Writes `transition_mode`, as its `int`.
    fn set_mode(&self, value: i32);
    /// Writes `transition_scale_type`, as its `int`.
    fn set_scale_type(&self, value: i32);
    /// Writes `transition_cx`.
    fn set_cx(&self, value: u32);
    /// Writes `transition_cy`.
    fn set_cy(&self, value: u32);
}

/// `obs_transition_load()`: restores a transition from its saved data.
///
/// Reads the saved name of slot A's source and the alignment, mode, scale
/// type, width and height, in that order. With a name ([`looks_up`]), looks
/// the source up and, when found, adds it as an active child; then, by
/// [`source`], keeps it, warns that it was not found, or warns of the
/// recursion and releases it. Under the lock, takes the old child of slot A
/// and its active flag, stores the source (or none) as an active slot A,
/// and writes the alignment and the width and height truncated to their
/// low 32 bits, the mode truncated to an `int`, and the scale type by
/// [`scale_type`]. After the unlock, removes the old child from the active
/// children when it was active ([`crate::transition_start::child_active`])
/// and releases it (`NULL` included, as the C does), then recalculates the
/// size and the matrices ([`crate::transition_size::recalculate_size`],
/// [`crate::transition_size::recalculate_matrices`]).
#[expect(clippy::todo, reason = "RED: the port follows")]
pub fn load<T: Transition>(transition: &T, data: &T::Data) {
    let _ = (transition, data);
    todo!()
}
