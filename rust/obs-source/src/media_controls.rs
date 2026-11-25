//! The eight public `obs_source_media_*()` control calls the media controls
//! dock, obs-websocket and the media sources use. Play/pause, restart, stop,
//! next, previous and set-time queue an action for the video thread
//! (`process_media_actions()`, still C, runs it in the tick); started and
//! ended emit a signal on the source. Whether a call is accepted is
//! [`crate::media_admission`]'s, one step at a time in the C's order, each
//! value read only when the step before asked for it.

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

use crate::media_admission::{Call, action};
use core::ffi::CStr;

/// `obs_source_media_play_pause`, named in its NULL diagnostic.
pub const PLAY_PAUSE_FUNCTION: &CStr = c"obs_source_media_play_pause";
/// `obs_source_media_restart`, named in its NULL diagnostic.
pub const RESTART_FUNCTION: &CStr = c"obs_source_media_restart";
/// `obs_source_media_stop`, named in its NULL diagnostic.
pub const STOP_FUNCTION: &CStr = c"obs_source_media_stop";
/// `obs_source_media_next`, named in its NULL diagnostic.
pub const NEXT_FUNCTION: &CStr = c"obs_source_media_next";
/// `obs_source_media_previous`, named in its NULL diagnostic.
pub const PREVIOUS_FUNCTION: &CStr = c"obs_source_media_previous";
/// `obs_source_media_set_time`, named in its NULL diagnostic.
pub const SET_TIME_FUNCTION: &CStr = c"obs_source_media_set_time";
/// `obs_source_media_started`, named in its NULL diagnostic.
pub const STARTED_FUNCTION: &CStr = c"obs_source_media_started";
/// `obs_source_media_ended`, named in its NULL diagnostic.
pub const ENDED_FUNCTION: &CStr = c"obs_source_media_ended";

/// The signal `obs_source_media_started()` emits on the source.
pub const STARTED_SIGNAL: &CStr = c"media_started";
/// The signal `obs_source_media_ended()` emits on the source.
pub const ENDED_SIGNAL: &CStr = c"media_ended";

/// A `struct media_action` to queue: its type, and `pause` for a
/// play/pause or `ms` for a set-time; the rest zero, as the C's designated
/// initializers left it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `MEDIA_ACTION_PLAY_PAUSE` with `pause`.
    PlayPause(bool),
    /// `MEDIA_ACTION_RESTART`.
    Restart,
    /// `MEDIA_ACTION_STOP`.
    Stop,
    /// `MEDIA_ACTION_NEXT`.
    Next,
    /// `MEDIA_ACTION_PREVIOUS`.
    Previous,
    /// `MEDIA_ACTION_SET_TIME` with `ms`.
    SetTime(i64),
}

impl Action {
    /// The call that queues it, for its admission and its callback.
    #[must_use]
    pub const fn call(self) -> Call {
        match self {
            Self::PlayPause(_) => Call::PlayPause,
            Self::Restart => Call::Restart,
            Self::Stop => Call::Stop,
            Self::Next => Call::Next,
            Self::Previous => Call::Previous,
            Self::SetTime(_) => Call::SetTime,
        }
    }

    /// The public function that queues it, named in its NULL diagnostic.
    #[must_use]
    pub const fn function(self) -> &'static CStr {
        match self {
            Self::PlayPause(_) => PLAY_PAUSE_FUNCTION,
            Self::Restart => RESTART_FUNCTION,
            Self::Stop => STOP_FUNCTION,
            Self::Next => NEXT_FUNCTION,
            Self::Previous => PREVIOUS_FUNCTION,
            Self::SetTime(_) => SET_TIME_FUNCTION,
        }
    }

    /// Its `enum media_action_type` value.
    #[must_use]
    pub const fn action_type(self) -> i32 {
        match self {
            Self::PlayPause(_) => action::PLAY_PAUSE,
            Self::Restart => action::RESTART,
            Self::Stop => action::STOP,
            Self::Next => action::NEXT,
            Self::Previous => action::PREVIOUS,
            Self::SetTime(_) => action::SET_TIME,
        }
    }
}

/// What the calls read, queue and emit on a live source, each one access,
/// in the order they ask.
pub trait Source {
    /// Whether `context.data` is set, read now (`data_valid()`'s second
    /// half).
    fn has_data(&self) -> bool;
    /// `info.output_flags`, read now.
    fn output_flags(&self) -> u32;
    /// Whether the `info.media_*` callback of `call` is set, read now.
    fn has_callback(&self, call: Call) -> bool;
    /// Appends `action` to `media_actions` under `media_actions_mutex`.
    fn queue(&self, action: Action);
    /// Emits `signal` on the source's own handler with only the source
    /// (`obs_source_dosignal(source, NULL, signal)`).
    fn signal(&self, signal: &'static CStr);
}

/// `obs_source_media_play_pause()` and the other queueing calls.
#[expect(clippy::todo, reason = "RED: the port lands in the next commit")]
pub fn queue(
    _source: Option<&impl Source>,
    _action: Action,
    _log_null: impl FnOnce(&'static CStr),
) {
    todo!("#5616")
}

/// `obs_source_media_started()`.
#[expect(clippy::todo, reason = "RED: the port lands in the next commit")]
pub fn started(_source: Option<&impl Source>, _log_null: impl FnOnce(&'static CStr)) {
    todo!("#5616")
}

/// `obs_source_media_ended()`.
#[expect(clippy::todo, reason = "RED: the port lands in the next commit")]
pub fn ended(_source: Option<&impl Source>, _log_null: impl FnOnce(&'static CStr)) {
    todo!("#5616")
}
