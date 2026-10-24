//! Safe core of `libobs/obs-source-media-admission.c.h`: the media action
//! admission decisions of the `obs_source_media_*()` functions and
//! `process_media_actions()`.
//!
//! A media source (one with `OBS_SOURCE_CONTROLLABLE_MEDIA`) is controlled
//! through eleven public calls. Play/pause, restart, stop, next, previous
//! and set-time queue an action that the video thread runs later in
//! `process_media_actions()`; the duration, time and state getters call
//! the source straight away; started and ended emit a signal. These are
//! the decisions they ask for:
//! - [`admit`], [`admit_flags`] and [`admit_callback`]: whether a call is
//!   accepted, one step at a time in the C's order. An invalid source is
//!   rejected, then one without [`CONTROLLABLE_MEDIA`], then, for every
//!   call but [`Call::Started`] and [`Call::Ended`], one without the call's
//!   callback. A step that needs a value answers [`Admit::ReadFlags`] or
//!   [`Admit::ReadCallback`], and `obs-source.c.h` reads that one value
//!   then, so nothing is read before the step that needs it;
//! - [`time_fallback`] and [`state_fallback`]: what the getters return for
//!   a rejected call, 0 and `OBS_MEDIA_STATE_NONE`;
//! - [`dispatch`]: the callback a queued action type calls,
//!   [`Dispatch::Return`] for `MEDIA_ACTION_NONE` (the queue is empty) and
//!   [`Dispatch::Skip`] for a type outside the enum;
//! - [`signal_reads_pause`] and [`signal`]: the signal emitted after the
//!   callback, `action.pause` read only for a play/pause.
//!
//! `obs-source.c.h` keeps `data_valid()` and `obs_source_valid()` (they
//! log), every field read, the action queue and its mutex, the callbacks,
//! the signals and the source refs. Nothing here keeps state, takes a lock
//! or calls libobs.
//!
//! Characterized, not endorsed (`test_source_media_admission_steps`), and
//! kept: started and ended need no callback, so they signal for any
//! controllable source; a set-time emits no signal; a queued type outside
//! the enum is dropped silently and the loop takes the next action.
//!
//! Not ported yet: every function is `todo!()`, so the Tier 1 tests fail.

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

use core::ffi::CStr;

/// `OBS_SOURCE_CONTROLLABLE_MEDIA`, the output flag of a media source.
pub const CONTROLLABLE_MEDIA: u32 = 1 << 13;

/// `OBS_MEDIA_STATE_NONE`, what `obs_source_media_get_state()` returns
/// for a rejected call.
pub const MEDIA_STATE_NONE: i32 = 0;

/// `enum media_action_type` (`obs-internal.h`), the type of a queued
/// action, as the C passes it.
pub mod action {
    /// `MEDIA_ACTION_NONE`: the queue is empty.
    pub const NONE: i32 = 0;
    /// `MEDIA_ACTION_PLAY_PAUSE`.
    pub const PLAY_PAUSE: i32 = 1;
    /// `MEDIA_ACTION_RESTART`.
    pub const RESTART: i32 = 2;
    /// `MEDIA_ACTION_STOP`.
    pub const STOP: i32 = 3;
    /// `MEDIA_ACTION_NEXT`.
    pub const NEXT: i32 = 4;
    /// `MEDIA_ACTION_PREVIOUS`.
    pub const PREVIOUS: i32 = 5;
    /// `MEDIA_ACTION_SET_TIME`.
    pub const SET_TIME: i32 = 6;
}

/// `enum obs_source_media_call`: the public `obs_source_media_*()`
/// function being admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Call {
    /// `obs_source_media_play_pause()`.
    PlayPause,
    /// `obs_source_media_restart()`.
    Restart,
    /// `obs_source_media_stop()`.
    Stop,
    /// `obs_source_media_next()`.
    Next,
    /// `obs_source_media_previous()`.
    Previous,
    /// `obs_source_media_get_duration()`.
    GetDuration,
    /// `obs_source_media_get_time()`.
    GetTime,
    /// `obs_source_media_set_time()`.
    SetTime,
    /// `obs_source_media_get_state()`.
    GetState,
    /// `obs_source_media_started()`.
    Started,
    /// `obs_source_media_ended()`.
    Ended,
}

impl Call {
    /// Every call, in the C enum's order.
    pub const ALL: [Self; 11] = [
        Self::PlayPause,
        Self::Restart,
        Self::Stop,
        Self::Next,
        Self::Previous,
        Self::GetDuration,
        Self::GetTime,
        Self::SetTime,
        Self::GetState,
        Self::Started,
        Self::Ended,
    ];

    /// The C enum's value.
    pub fn value(self) -> i32 {
        let _ = self;
        todo!("Call::value")
    }

    /// The call with the C enum's value `value`, `None` outside the enum.
    pub fn from_value(value: i32) -> Option<Self> {
        let _ = value;
        todo!("Call::from_value")
    }
}

/// `enum obs_source_media_admit`: an admission answer, or the value to read
/// before it is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Admit {
    /// `OBS_SOURCE_MEDIA_ADMIT_REJECT`: return (a getter returns its
    /// fallback).
    Reject,
    /// `OBS_SOURCE_MEDIA_ADMIT_ACCEPT`: queue the action, call the getter
    /// or emit the signal.
    Accept,
    /// `OBS_SOURCE_MEDIA_ADMIT_READ_FLAGS`: read `source->info.output_flags`.
    ReadFlags,
    /// `OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK`: read the call's callback.
    ReadCallback,
}

impl Admit {
    /// The C enum's value.
    pub fn value(self) -> u32 {
        let _ = self;
        todo!("Admit::value")
    }
}

/// `enum obs_source_media_dispatch`: what `process_media_actions()` does
/// with an action it took off the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dispatch {
    /// `OBS_SOURCE_MEDIA_DISPATCH_RETURN`: `MEDIA_ACTION_NONE`, return.
    Return,
    /// `OBS_SOURCE_MEDIA_DISPATCH_PLAY_PAUSE`: `media_play_pause(data,
    /// action.pause)`.
    PlayPause,
    /// `OBS_SOURCE_MEDIA_DISPATCH_RESTART`: `media_restart(data)`.
    Restart,
    /// `OBS_SOURCE_MEDIA_DISPATCH_STOP`: `media_stop(data)`.
    Stop,
    /// `OBS_SOURCE_MEDIA_DISPATCH_NEXT`: `media_next(data)`.
    Next,
    /// `OBS_SOURCE_MEDIA_DISPATCH_PREVIOUS`: `media_previous(data)`.
    Previous,
    /// `OBS_SOURCE_MEDIA_DISPATCH_SET_TIME`: `media_set_time(data,
    /// action.ms)`.
    SetTime,
    /// `OBS_SOURCE_MEDIA_DISPATCH_SKIP`: a type outside the enum, nothing.
    Skip,
}

impl Dispatch {
    /// Every dispatch, in the C enum's order.
    pub const ALL: [Self; 8] = [
        Self::Return,
        Self::PlayPause,
        Self::Restart,
        Self::Stop,
        Self::Next,
        Self::Previous,
        Self::SetTime,
        Self::Skip,
    ];

    /// The C enum's value.
    pub fn value(self) -> i32 {
        let _ = self;
        todo!("Dispatch::value")
    }

    /// The dispatch with the C enum's value `value`, `None` outside the
    /// enum.
    pub fn from_value(value: i32) -> Option<Self> {
        let _ = value;
        todo!("Dispatch::from_value")
    }
}

/// `obs_source_media_admit()`.
pub fn admit(valid: bool) -> Admit {
    let _ = valid;
    todo!("obs_source_media_admit")
}

/// `obs_source_media_admit_flags()`; `call` is `None` outside the enum.
pub fn admit_flags(call: Option<Call>, output_flags: u32) -> Admit {
    let _ = (call, output_flags);
    todo!("obs_source_media_admit_flags")
}

/// `obs_source_media_admit_callback()`.
pub fn admit_callback(has_callback: bool) -> Admit {
    let _ = has_callback;
    todo!("obs_source_media_admit_callback")
}

/// `obs_source_media_time_fallback()`.
pub fn time_fallback() -> i64 {
    todo!("obs_source_media_time_fallback")
}

/// `obs_source_media_state_fallback()`.
pub fn state_fallback() -> i32 {
    todo!("obs_source_media_state_fallback")
}

/// `obs_source_media_dispatch()`.
pub fn dispatch(action_type: i32) -> Dispatch {
    let _ = action_type;
    todo!("obs_source_media_dispatch")
}

/// `obs_source_media_signal_reads_pause()`; `dispatch` is `None` outside
/// the enum.
pub fn signal_reads_pause(dispatch: Option<Dispatch>) -> bool {
    let _ = dispatch;
    todo!("obs_source_media_signal_reads_pause")
}

/// `obs_source_media_signal()`; `dispatch` is `None` outside the enum.
pub fn signal(dispatch: Option<Dispatch>, pause: bool) -> Option<&'static CStr> {
    let _ = (dispatch, pause);
    todo!("obs_source_media_signal")
}
