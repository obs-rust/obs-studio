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
//! - [`copy_stores`]: the recursion refusal of `copy_source_state()`.
//!
//! Each decision is its own function, asked at its own point, so
//! `obs-source-transition.c` keeps every read and call where it was and
//! under its condition: `obs_ptr_valid()`, the locks and their order, the
//! pointer comparisons, the `obs_data` reads, the lookup, the add only for a
//! found source or an active new child, every `obs_source_get_ref()` and
//! `obs_source_release()`, the active child calls, the `(uint32_t)`
//! truncations of the loaded alignment and size, the mode taken as is, the
//! fields the swap hands over, the texrender swap, the size and matrix
//! recalculation and the logs. The getters' answers for an invalid
//! transition stay C literals. Whether the load takes the old source A out
//! of the active children is [`crate::transition_start::child_active`].
//! Nothing here keeps state, takes a lock or calls libobs.
//!
//! Characterized, not endorsed (`test_transition_load_steps`), and kept in
//! C: the loaded alignment, width and height are truncated to their low 32
//! bits, and the loaded mode is stored as is.

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
pub fn is_transition(source_type: i32) -> bool {
    let _ = source_type;
    todo!("transition_load::is_transition")
}

/// A scale type as stored: [`SCALE_MAX_ONLY`], [`SCALE_ASPECT`] and
/// [`SCALE_STRETCH`] as they are, any other value [`SCALE_MAX_ONLY`]. The
/// value is compared in 64 bits, as `obs_data` holds it.
#[must_use]
pub fn scale_type(value: i64) -> i32 {
    let _ = value;
    todo!("transition_load::scale_type")
}

/// Whether `obs_transition_load()` looks its saved source up: it has a
/// name.
#[must_use]
pub fn looks_up(has_name: bool) -> bool {
    let _ = has_name;
    todo!("transition_load::looks_up")
}

/// What `obs_transition_load()` does with the looked-up source, after the
/// add (asked only for a found source): [`Source::NotFound`] when none was
/// found, [`Source::Refuse`] when the add failed, else [`Source::Keep`].
#[must_use]
pub fn source(found: bool, add_ok: bool) -> Source {
    let _ = (found, add_ok);
    todo!("transition_load::source")
}

/// Whether `obs_transition_swap_begin()` and `obs_transition_swap_end()`
/// do anything: not for a transition swapped with itself.
#[must_use]
pub fn swaps(same: bool) -> bool {
    let _ = same;
    todo!("transition_load::swaps")
}

/// Whether `copy_source_state()` stores the new child: it was not to be
/// added as an active child, or the add worked.
#[must_use]
pub fn copy_stores(adds: bool, add_ok: bool) -> bool {
    let _ = (adds, add_ok);
    todo!("transition_load::copy_stores")
}
