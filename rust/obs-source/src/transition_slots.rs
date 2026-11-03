//! Safe core of `libobs/obs-source-transition-slots.c.h`: the two child
//! slots of a transition, moved out of `libobs/obs-source-transition.c.h`.
//!
//! - [`clear`]: `obs_transition_clear()`, which empties both slots and stops
//!   the transition under its lock, then drops each child's active-child
//!   link and reference after the unlock;
//! - [`enum_sources`]: `obs_transition_enum_sources()`, which hands each
//!   non-empty slot to the callback with the lock held;
//! - [`swap_begin`] and [`swap_end`]: `obs_transition_swap_begin()` (with
//!   the C's file-local `copy_source_state()`, [`copy_source_state`]) and
//!   `obs_transition_swap_end()`, which hand one transition's children,
//!   running state and render targets to another.
//!
//! Each is generic over a [`Transition`]: one method per field read or
//! write, per lock and unlock call and per call into the rest of libobs
//! (`obs_source_get_ref()`, `obs_source_release()`, the active-child add and
//! remove and the recursion warning), so a function makes the C's calls in
//! the C's order and under the C's locks. The swap asks the rules ported
//! with the transition start and load: [`crate::transition_start::child_active`],
//! [`crate::transition_load::copy_stores`] and
//! [`crate::transition_load::swaps`].
//!
//! The C shims live in the `obs-source-seam` crate, which implements
//! [`Transition`] over `libobs/obs-source-transition-seam.h` and the libobs
//! calls; only libobs-rust links it, so this crate calls nothing in libobs.
//!
//! The locks are the C's, taken and released where it takes them:
//! [`swap_begin`] takes both texture locks (the source's, then the
//! destination's), then both transition locks (the same order), and
//! releases only the transition locks (the destination's first);
//! [`swap_end`] releases the texture locks (the destination's first) after
//! it swaps the render targets. [`enum_sources`] runs the callback under the
//! transition lock.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::todo
)]

use std::ffi::CStr;

/// The name `obs_transition_clear()` hands `transition_valid()`, which logs
/// it for a NULL source.
pub const CLEAR_LOG_NAME: &CStr = c"obs_transition_clear";

/// The warning `obs_transition_swap_begin()` logs (`LOG_WARNING`) when the
/// destination refuses a child as an active child: the format, with the
/// destination's name, then the child's.
pub const RECURSION_WARNING: &CStr =
    c"obs_transition_swap_begin: cannot give transition '%s' source '%s' due to infinite recursion";

/// The two slots, A (0) and B (1).
pub const SLOTS: [usize; 2] = [0, 1];

/// The fields, locks and libobs calls of a transition that its slots use: in
/// libobs, the transition seam (`obs-source-transition-seam.h`) and the
/// `obs_source_*()` functions, one call per method. Every method takes
/// `&self`, as the C writes through its `obs_source_t *`: the swap hands the
/// same `obs_source_t` twice when it is asked to swap a transition with
/// itself, and returns at once.
pub trait Transition {
    /// A child in a slot, as the seam hands it back.
    type Child: Copy;
    /// A `gs_texrender_t *` of the two render targets.
    type Texrender: Copy;

    /// Whether `self` and `other` are the same `obs_source_t`.
    fn same(&self, other: &Self) -> bool;
    /// `transition_valid(source, func)`: false for no source (the C logs
    /// `func` at debug level) and for one that is not a transition.
    fn valid(source: Option<&Self>, func: &'static CStr) -> bool;

    /// `transition_sources[idx]`, `None` for an empty slot.
    fn source(&self, idx: usize) -> Option<Self::Child>;
    /// Writes `transition_sources[idx]`.
    fn set_source(&self, idx: usize, value: Option<Self::Child>);
    /// `transition_source_active[idx]`.
    fn source_active(&self, idx: usize) -> bool;
    /// Writes `transition_source_active[idx]`.
    fn set_source_active(&self, idx: usize, value: bool);
    /// `transitioning_video`.
    fn transitioning_video(&self) -> bool;
    /// Writes `transitioning_video`.
    fn set_transitioning_video(&self, value: bool);
    /// `transitioning_audio`.
    fn transitioning_audio(&self) -> bool;
    /// Writes `transitioning_audio`.
    fn set_transitioning_audio(&self, value: bool);
    /// `transition_start_time`.
    fn start_time(&self) -> u64;
    /// Writes `transition_start_time`.
    fn set_start_time(&self, value: u64);
    /// `transition_duration`.
    fn duration(&self) -> u64;
    /// Writes `transition_duration`.
    fn set_duration(&self, value: u64);
    /// `transition_mode`, as its `int`.
    fn mode(&self) -> i32;
    /// Writes `transition_mode`.
    fn set_mode(&self, value: i32);
    /// `transition_manual_val`.
    fn manual_val(&self) -> f32;
    /// Writes `transition_manual_val`.
    fn set_manual_val(&self, value: f32);
    /// `transition_manual_target`.
    fn manual_target(&self) -> f32;
    /// Writes `transition_manual_target`.
    fn set_manual_target(&self, value: f32);
    /// `transition_texrender[idx]`.
    fn texrender(&self, idx: usize) -> Self::Texrender;
    /// Writes `transition_texrender[idx]`.
    fn set_texrender(&self, idx: usize, value: Self::Texrender);

    /// Locks `transition_mutex`.
    fn lock(&self);
    /// Unlocks `transition_mutex`.
    fn unlock(&self);
    /// Locks `transition_tex_mutex`.
    fn lock_textures(&self);
    /// Unlocks `transition_tex_mutex`.
    fn unlock_textures(&self);

    /// `obs_source_get_ref(child)`: `None` for no child and for one whose
    /// last reference is gone.
    fn get_ref(&self, child: Option<Self::Child>) -> Option<Self::Child>;
    /// `obs_source_release(child)`, which does nothing for `None`.
    fn release(&self, child: Option<Self::Child>);
    /// `obs_source_add_active_child(self, child)`: false when `child` is
    /// `self` or contains it.
    fn add_active_child(&self, child: Self::Child) -> bool;
    /// `obs_source_remove_active_child(self, child)`.
    fn remove_active_child(&self, child: Self::Child);
    /// Logs [`RECURSION_WARNING`] with `self`'s name and `child`'s
    /// (`obs_source_get_name()` of each).
    fn warn_recursion(&self, child: Self::Child);
}

/// `obs_transition_clear()`: for a valid transition, under its lock, takes
/// both children and their active flags out of the slots and stops the
/// video and audio transition; after the unlock, removes each child that
/// was active from the active children and releases each slot's reference
/// (an empty slot's `NULL` included, as the C does), slot A first.
#[expect(clippy::todo, reason = "RED: the port follows")]
pub fn clear<T: Transition>(transition: Option<&T>) {
    let _ = transition;
    todo!()
}

/// `obs_transition_enum_sources()`: no check of its source, as the C has
/// none. Under the transition's lock, calls `callback` with each non-empty
/// slot's child, slot A first.
#[expect(clippy::todo, reason = "RED: the port follows")]
pub fn enum_sources<T: Transition>(transition: &T, callback: impl FnMut(T::Child)) {
    let _ = (transition, callback);
    todo!()
}

/// `copy_source_state()`: hands `source`'s slot `idx` to `dest` under both
/// transition locks and returns the child `dest` held there, for the caller
/// to release after the unlock.
///
/// `dest` takes its own reference on the new child and adds it as an active
/// child when its slot is active
/// ([`crate::transition_start::child_active`]). When that add fails (the
/// child is `dest` or contains it, so `obs_transition_set()` would refuse
/// it), `dest` logs [`RECURSION_WARNING`], releases its new reference and
/// stores an empty, inactive slot
/// ([`crate::transition_load::copy_stores`]). The child `dest` held is
/// removed from its active children first, when its slot was active.
#[expect(clippy::todo, reason = "RED: the port follows")]
pub fn copy_source_state<T: Transition>(dest: &T, source: &T, idx: usize) -> Option<T::Child> {
    let _ = (dest, source, idx);
    todo!()
}

/// `obs_transition_swap_begin()`: nothing when `dest` and `source` are the
/// same transition ([`crate::transition_load::swaps`]). Otherwise takes the
/// texture locks (`source`'s, then `dest`'s) and the transition locks (the
/// same order), copies both slots ([`copy_source_state`]) and the running
/// state (`transitioning_video`, `transitioning_audio`, the start time, the
/// duration, the mode and the manual value and target) from `source` to
/// `dest`, releases the transition locks (`dest`'s first) and the children
/// `dest` held. The texture locks stay held for [`swap_end`].
#[expect(clippy::todo, reason = "RED: the port follows")]
pub fn swap_begin<T: Transition>(dest: &T, source: &T) {
    let _ = (dest, source);
    todo!()
}

/// `obs_transition_swap_end()`: nothing when `dest` and `source` are the
/// same transition. Otherwise clears `source` ([`clear`]), swaps the two
/// render targets of each slot between `dest` and `source`, and releases
/// the texture locks [`swap_begin`] took (`dest`'s first).
#[expect(clippy::todo, reason = "RED: the port follows")]
pub fn swap_end<T: Transition>(dest: &T, source: &T) {
    let _ = (dest, source);
    todo!()
}
