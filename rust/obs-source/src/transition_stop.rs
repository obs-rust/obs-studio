//! Safe core of `libobs/obs-source-transition-stop.c.h`: the clock read and
//! the stop of a transition, moved out of `libobs/obs-source-transition.c.h`.
//!
//! - [`calc_time`]: `transition_calc_time()`, the transition's position at
//!   a timestamp, from its mode, manual value, start time and duration
//!   ([`crate::transition_clock::time`]);
//! - [`get_video_time`]: `transition_get_video_time()`, [`calc_time`] at
//!   the latest video frame's time;
//! - [`get_time`]: `obs_transition_get_time()`, [`get_video_time`] under
//!   `transition_mutex`;
//! - [`stop`]: `transition_stop()`, the end of a transition: slot A's
//!   child is released (and removed as an active child when its slot was
//!   active), slot B's child becomes slot A, active, and slot B empties;
//! - [`handle_stop`]: `transition_handle_stop()`, the type's
//!   `transition_stop` callback, when it has one, then the `transition_stop`
//!   and `source_transition_stop` signals;
//! - [`force_stop`]: `obs_transition_force_stop()`, [`handle_stop`].
//!
//! Each is generic over a [`Transition`]: one method per field read or
//! write, per lock and unlock call and per call into the rest of libobs
//! (the video frame time, the plugin's `transition_stop`,
//! `obs_source_dosignal()`, `obs_source_remove_active_child()` and
//! `obs_source_release()`), so a function makes the C's calls in the C's
//! order and under the C's locks.
//!
//! The C shims live in the `obs-source-seam` crate, which implements
//! [`Transition`] over `libobs/obs-source-transition-seam.h` and the libobs
//! calls; only libobs-rust links it, so this crate calls nothing in libobs.
//!
//! The locks are the callers': [`calc_time`], [`get_video_time`] and
//! [`stop`] run with `transition_mutex` held by the video and audio renders
//! (still C) and take no lock of their own; [`handle_stop`] runs after the
//! renders let it go. [`get_video_time`] reads the video frame time, which
//! takes the video stats mutex, with `transition_mutex` held, and takes
//! nothing else in between. The force stop never touches the slots.
//!
//! Intentional difference from C: [`get_time`] and [`force_stop`] ask
//! `transition_valid()` first, with their own names, as every other public
//! transition call does. The C asks nothing: a NULL transition crashes on
//! the lock or on the type's callback, and a source that is not a
//! transition returns a time from fields it never uses, or calls back and
//! signals a stop on it. Here [`get_time`] returns 0 and [`force_stop`]
//! does nothing for those.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use std::ffi::CStr;

/// The name [`get_time`] hands `transition_valid()`, which logs it for a
/// NULL source.
pub const GET_TIME_LOG_NAME: &CStr = c"obs_transition_get_time";

/// The name [`force_stop`] hands `transition_valid()`.
pub const FORCE_STOP_LOG_NAME: &CStr = c"obs_transition_force_stop";

/// The signals of a stop: the core handler's, then the transition's.
pub const STOP_SIGNALS: (&CStr, &CStr) = (c"source_transition_stop", c"transition_stop");

/// `OBS_TRANSITION_SOURCE_A`, the slot a stop empties and refills.
pub const SOURCE_A: usize = 0;

/// `OBS_TRANSITION_SOURCE_B`, the slot a stop moves into slot A.
pub const SOURCE_B: usize = 1;

/// The fields, locks and libobs calls of a transition that its clock read
/// and stop use: in libobs, the transition seam
/// (`obs-source-transition-seam.h`) and the libobs functions, one call per
/// method. Every method takes `&self`, as the C writes through its
/// `obs_source_t *`.
pub trait Transition {
    /// A child source, as a slot holds it.
    type Child: Copy + PartialEq;

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
    /// `transition_mode`, as its `int`.
    fn mode(&self) -> i32;
    /// `transition_manual_val`.
    fn manual_val(&self) -> f32;
    /// `transition_start_time`, in ns.
    fn start_time(&self) -> u64;
    /// `transition_duration`, in ns.
    fn duration(&self) -> u64;

    /// Locks `transition_mutex`.
    fn lock(&self);
    /// Unlocks `transition_mutex`.
    fn unlock(&self);

    /// `obs_video_stats_frame_time(&obs->video.stats)`: the latest video
    /// frame's time, read under the video stats mutex.
    fn video_time(&self) -> u64;
    /// The type's `transition_stop` callback on the context data, when it
    /// has one.
    fn call_stop(&self);
    /// `obs_source_dosignal(self, signal_obs, signal_source)`.
    fn dosignal(&self, signal_obs: &'static CStr, signal_source: &'static CStr);
    /// `obs_source_remove_active_child(self, child)`.
    fn remove_active_child(&self, child: Self::Child);
    /// `obs_source_release(child)`, which does nothing for `None`.
    fn release(&self, child: Option<Self::Child>);
}

/// `transition_calc_time()`: the transition's position at `ts`, from its
/// mode, manual value, start time and duration (read in that order), by
/// [`crate::transition_clock::time`]. Called with `transition_mutex` held.
pub fn calc_time<T: Transition>(transition: &T, ts: u64) -> f32 {
    let _ = (transition, ts);
    todo!()
}

/// `transition_get_video_time()`: [`calc_time`] at the latest video
/// frame's time, read first. Called with `transition_mutex` held.
pub fn get_video_time<T: Transition>(transition: &T) -> f32 {
    let _ = transition;
    todo!()
}

/// `obs_transition_get_time()`: [`get_video_time`] under
/// `transition_mutex`, the lock that publishes a start's clock, so a caller
/// that chooses what to draw from it and then renders never pairs a new
/// start with the previous clock.
///
/// 0 for no transition and for a source that is not one (an intentional
/// difference: the C crashes on the first and reads the second's unused
/// fields).
pub fn get_time<T: Transition>(transition: Option<&T>) -> f32 {
    let _ = transition;
    todo!()
}

/// `transition_stop()`: the end of a transition. Slot A's child is removed
/// as an active child (when there is one and its slot is active) and
/// released; then slot A is active, slot B is not, slot B's child moves to
/// slot A and slot B empties. Called with `transition_mutex` held; takes no
/// lock.
pub fn stop<T: Transition>(transition: &T) {
    let _ = transition;
    todo!()
}

/// `transition_handle_stop()`: the type's `transition_stop` callback, when
/// it has one, then the stop signals ([`STOP_SIGNALS`]). Called after
/// `transition_mutex` is let go; touches no field.
pub fn handle_stop<T: Transition>(transition: &T) {
    let _ = transition;
    todo!()
}

/// `obs_transition_force_stop()`: [`handle_stop`], whether or not the
/// transition runs; the slots and running state stay as they are.
///
/// Nothing for no transition and for a source that is not one (an
/// intentional difference: the C crashes on the first and calls back and
/// signals on the second).
pub fn force_stop<T: Transition>(transition: Option<&T>) {
    let _ = transition;
    todo!()
}
