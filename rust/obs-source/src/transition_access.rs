//! Safe core of `libobs/obs-source-transition-access.c.h`: the getters and
//! manual controls of a transition, moved out of
//! `libobs/obs-source-transition.c.h`.
//!
//! - [`get_source`]: `obs_transition_get_source()`, a new reference on what
//!   one slot holds;
//! - [`get_active_source`]: `obs_transition_get_active_source()`, a new
//!   reference on the active child (slot B while the transition runs, slot A
//!   otherwise: [`crate::transition_start::active_index`]);
//! - [`set_manual_torque`]: `obs_transition_set_manual_torque()`, the torque
//!   and clamp the tick moves the manual value with;
//! - [`set_manual_time`]: `obs_transition_set_manual_time()`, the manual
//!   target, and in manual mode with a time of 0 the end of the transition
//!   on slot A ([`crate::transition_start::manual_reset`]).
//!
//! Each is generic over a [`Transition`]: one method per field read or
//! write, per lock and unlock call and per call into the rest of libobs
//! (`transition_valid()`, `obs_source_get_ref()`, `obs_transition_set()`),
//! so a function makes the C's calls in the C's order and under the C's
//! lock. The C shims live in the `obs-source-seam` crate, which implements
//! [`Transition`] over `libobs/obs-source-transition-seam.h` and the libobs
//! calls.
//!
//! As in the C, the getters read the slot and take the reference under
//! `transition_mutex`; the setters write under it; and
//! [`set_manual_time`] reads slot A for `obs_transition_set()` after the
//! unlock. [`get_source`] hands the caller's slot index to
//! [`Transition::source`] unchecked, as the C indexes with it.
//!
//! One intentional difference: the C's two setters never check their
//! argument (a NULL transition crashes on the lock, and a source that is
//! not a transition has its transition fields written). Here both return
//! first unless `transition_valid()` holds, as every other
//! `obs_transition_*()` setter does, and log a NULL under their own names,
//! [`SET_MANUAL_TORQUE_LOG_NAME`] and [`SET_MANUAL_TIME_LOG_NAME`].
//! [`get_active_source`] still logs under `obs_transition_get_source`'s
//! name ([`GET_ACTIVE_SOURCE_LOG_NAME`]), as the C does: an unchanged C test
//! pins that line.

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

/// The name `obs_transition_get_source()` hands `transition_valid()`,
/// which logs it for a NULL source.
pub const GET_SOURCE_LOG_NAME: &CStr = c"obs_transition_get_source";

/// The name `obs_transition_get_active_source()` hands
/// `transition_valid()`: `obs_transition_get_source`'s, as in the C.
pub const GET_ACTIVE_SOURCE_LOG_NAME: &CStr = GET_SOURCE_LOG_NAME;

/// The name `obs_transition_set_manual_torque()` hands
/// `transition_valid()` (an intentional difference: the C checks nothing).
pub const SET_MANUAL_TORQUE_LOG_NAME: &CStr = c"obs_transition_set_manual_torque";

/// The name `obs_transition_set_manual_time()` hands `transition_valid()`
/// (an intentional difference: the C checks nothing).
pub const SET_MANUAL_TIME_LOG_NAME: &CStr = c"obs_transition_set_manual_time";

/// The slot `obs_transition_set_manual_time()` ends the transition on: A.
pub const RESET_SLOT: usize = 0;

/// The fields, lock and libobs calls of a transition that its getters and
/// manual controls use: in libobs, the transition seam
/// (`obs-source-transition-seam.h`), `obs_source_get_ref()` and
/// `obs_transition_set()`, one call per method.
pub trait Transition {
    /// A child in a slot, as the seam hands it back.
    type Child: Copy;

    /// `transition_valid(source, func)`: false for no source (the C logs
    /// `func` at debug level) and for one that is not a transition.
    fn valid(source: Option<&Self>, func: &'static CStr) -> bool;

    /// `transition_sources[idx]`, `None` for an empty slot. `idx` is the
    /// caller's, unchecked.
    fn source(&self, idx: usize) -> Option<Self::Child>;
    /// `transitioning_video`.
    fn transitioning_video(&self) -> bool;
    /// `transitioning_audio`.
    fn transitioning_audio(&self) -> bool;
    /// `transition_mode`, as its `int`.
    fn mode(&self) -> i32;
    /// Writes `transition_manual_torque`.
    fn set_manual_torque(&self, value: f32);
    /// Writes `transition_manual_clamp`.
    fn set_manual_clamp(&self, value: f32);
    /// Writes `transition_manual_target`.
    fn set_manual_target(&self, value: f32);

    /// Locks `transition_mutex`.
    fn lock(&self);
    /// Unlocks `transition_mutex`.
    fn unlock(&self);

    /// `obs_source_get_ref(child)`: `None` for no child and for one whose
    /// last reference is gone.
    fn get_ref(&self, child: Option<Self::Child>) -> Option<Self::Child>;
    /// `obs_transition_set(self, source)`.
    fn set(&self, source: Option<Self::Child>);
}

/// `obs_transition_get_source()`: for a valid transition, a new reference
/// on what slot `idx` holds, read and referenced under its lock; `None` for
/// an empty slot, a child going away, no transition and a source that is
/// not a transition.
#[expect(clippy::todo, reason = "RED: the port follows")]
pub fn get_source<T: Transition>(transition: Option<&T>, idx: usize) -> Option<T::Child> {
    let _ = (transition, idx);
    todo!()
}

/// `obs_transition_get_active_source()`: for a valid transition, a new
/// reference on its active child, slot B while it runs (audio or video,
/// `transitioning_audio` read first) and slot A otherwise, read and
/// referenced under its lock.
#[expect(clippy::todo, reason = "RED: the port follows")]
pub fn get_active_source<T: Transition>(transition: Option<&T>) -> Option<T::Child> {
    let _ = transition;
    todo!()
}

/// `obs_transition_set_manual_torque()`: for a valid transition, writes the
/// torque, then the clamp, under its lock.
#[expect(clippy::todo, reason = "RED: the port follows")]
pub fn set_manual_torque<T: Transition>(transition: Option<&T>, torque: f32, clamp: f32) {
    let _ = (transition, torque, clamp);
    todo!()
}

/// `obs_transition_set_manual_time()`: for a valid transition, writes the
/// manual target and reads the mode under its lock; after the unlock, in
/// manual mode with `t == 0.0`, reads slot A (without the lock, as the C
/// does) and hands it to `obs_transition_set()`.
#[expect(clippy::todo, reason = "RED: the port follows")]
pub fn set_manual_time<T: Transition>(transition: Option<&T>, t: f32) {
    let _ = (transition, t);
    todo!()
}
