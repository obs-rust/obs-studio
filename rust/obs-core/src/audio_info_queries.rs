//! Safe core of `libobs/obs-audio-info-queries.c.h`: `obs_get_audio_info()`
//! and `obs_get_audio_info2()`, the current audio settings (#5363).
//!
//! A NULL core answers false before anything is read (#2566, #3363).
//! `obs_get_audio_info` checks its output, then the core's audio output, and
//! only then reads the output's info. `obs_get_audio_info2` first runs
//! `obs_get_audio_info` into a local, then checks its own output and the
//! audio output again, and reads the fixed-buffering flag, the buffering
//! ticks and the milliseconds helper in that order; the helper's `int` is
//! cast straight to `uint32_t`. Nothing is written on failure.

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

/// The values `struct obs_audio_info` receives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AudioInfo {
    pub samples_per_sec: u32,
    /// `enum speaker_layout`, unchanged.
    pub speakers: c_int,
}

/// The values `struct obs_audio_info2` receives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioInfo2 {
    pub samples_per_sec: u32,
    /// `enum speaker_layout`, unchanged.
    pub speakers: c_int,
    pub max_buffering_ms: u32,
    pub fixed_buffering: bool,
}

/// A live core's audio state, one read or call per method call.
pub trait AudioCore {
    /// `obs->audio.audio != NULL`.
    fn has_output(&mut self) -> bool;
    /// `audio_output_get_info(obs->audio.audio)`: its rate and speakers.
    /// Asked only right after [`AudioCore::has_output`] held.
    fn output_info(&mut self) -> AudioInfo;
    /// `obs->audio.fixed_buffer`.
    fn fixed_buffer(&mut self) -> bool;
    /// `obs->audio.max_buffering_ticks`.
    fn max_buffering_ticks(&mut self) -> c_int;
    /// `obs_core_setup_rules_buffer_ms(ticks, samples_per_sec)`.
    fn buffer_ms(&mut self, ticks: c_int, samples_per_sec: u32) -> c_int;
}

/// `obs_get_audio_info`: `core` is `None` for a NULL core, `output` false
/// for a NULL `oai`. `Some` is a true return with the values to store.
pub fn get_audio_info<C: AudioCore>(core: Option<&mut C>, output: bool) -> Option<AudioInfo> {
    let core = core?;
    if !output || !core.has_output() {
        return None;
    }
    Some(core.output_info())
}

/// `obs_get_audio_info2`: `core` is `None` for a NULL core, `output` false
/// for a NULL `oai2`. `Some` is a true return with the values to store.
pub fn get_audio_info2<C: AudioCore>(core: Option<&mut C>, output: bool) -> Option<AudioInfo2> {
    let core = core?;
    let info = get_audio_info(Some(&mut *core), true)?;
    if !output || !core.has_output() {
        return None;
    }
    let fixed_buffering = core.fixed_buffer();
    let ticks = core.max_buffering_ticks();
    let max_buffering_ms = core.buffer_ms(ticks, info.samples_per_sec).cast_unsigned();
    Some(AudioInfo2 {
        samples_per_sec: info.samples_per_sec,
        speakers: info.speakers,
        max_buffering_ms,
        fixed_buffering,
    })
}
