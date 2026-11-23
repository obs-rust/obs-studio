//! Safe core of `libobs/obs-reset-audio2.c.h`: `obs_reset_audio2()`, which
//! closes the core's audio and opens it again with new settings (#5486).
//!
//! In order: with no core it refuses before anything else
//! ([`audio_refused`] at [`AudioStep::Core`]); when an audio output exists
//! and is active it refuses, nothing changed ([`AudioStep::Active`], asked
//! only when there is an output). Otherwise it frees the current audio, and
//! a NULL info stops there with true. With an info it stores the maximum
//! buffering in ticks ([`buffer_ticks`] of a set `max_buffering_ms`, else
//! [`buffer_default_ticks`]) and the fixed-buffering flag, logs the separator
//! and the "audio settings reset" message (the maximum read back in
//! milliseconds through [`buffer_ms`]), and returns what opening the audio
//! output with the info's rate and layout returns.
//!
//! The steps on the core's private state (the core pointer, its audio
//! output, `obs_free_audio()`, the two stored fields and `obs_init_audio()`
//! on the "Audio" float planar output with `audio_callback`) stay in C,
//! behind [`Steps`].

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

use core::ffi::c_int;

use crate::audio_info_queries::AudioInfo2;
#[allow(unused_imports)]
use crate::core_setup_rules::{
    AudioStep, audio_refused, buffer_default_ticks, buffer_ms, buffer_set, buffer_ticks,
};

/// The separator logged before the reset message.
pub const SEPARATOR: &str = "---------------------------------";

/// The buffering type the message names for fixed buffering.
pub const FIXED: &str = "fixed";

/// The buffering type the message names for dynamic buffering.
pub const DYNAMIC: &str = "dynamically increasing";

/// One `blog(LOG_INFO, ...)` line of a reset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    /// [`SEPARATOR`].
    Separator,
    /// "audio settings reset", with the `int` arguments the C passes.
    Reset {
        samples_per_sec: c_int,
        speakers: c_int,
        max_buffering_ms: c_int,
        fixed: bool,
    },
}

impl Line {
    /// The buffering type of a [`Line::Reset`], [`FIXED`] or [`DYNAMIC`].
    pub fn buffering_type(fixed: bool) -> &'static str {
        if fixed { FIXED } else { DYNAMIC }
    }

    /// The text as logged.
    pub fn text(&self) -> String {
        match *self {
            Line::Separator => SEPARATOR.to_owned(),
            Line::Reset {
                samples_per_sec,
                speakers,
                max_buffering_ms,
                fixed,
            } => format!(
                "audio settings reset:\n\tsamples per sec: {samples_per_sec}\n\tspeakers:        {speakers}\n\tmax buffering:   {max_buffering_ms} milliseconds\n\tbuffering type:  {}",
                Line::buffering_type(fixed)
            ),
        }
    }
}

/// The core's private audio state and the caller's info, one read or call
/// per method call.
pub trait Steps {
    /// `obs != NULL`.
    fn core(&mut self) -> bool;
    /// `obs->audio.audio != NULL`.
    fn has_output(&mut self) -> bool;
    /// `audio_output_active(obs->audio.audio)`, asked only right after
    /// [`Steps::has_output`] held.
    fn output_active(&mut self) -> bool;
    /// `obs_free_audio()`.
    fn free_audio(&mut self);
    /// `*oai`, or `None` for a NULL `oai`; asked once, right after
    /// [`Steps::free_audio`].
    fn info(&mut self) -> Option<AudioInfo2>;
    /// `obs->audio.max_buffering_ticks = max_buffering_ticks;` then
    /// `obs->audio.fixed_buffer = fixed_buffer;`.
    fn set_buffering(&mut self, max_buffering_ticks: c_int, fixed_buffer: bool);
    /// `blog(LOG_INFO, ...)` of `line`.
    fn log(&mut self, line: Line);
    /// `obs_init_audio()` on the "Audio" float planar output at
    /// `samples_per_sec` and `speakers`, with `audio_callback`.
    fn init_audio(&mut self, samples_per_sec: u32, speakers: c_int) -> bool;
}

/// `obs_reset_audio2(oai)` over `steps`.
pub fn reset_audio2(steps: &mut impl Steps) -> bool {
    let _ = steps;
    todo!()
}
