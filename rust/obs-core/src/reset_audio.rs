//! Safe core of `libobs/obs-reset-audio.c.h`: `obs_reset_audio()`, the
//! legacy audio reset (#5482).
//!
//! It takes only the sample rate and the speaker layout of a
//! `struct obs_audio_info`, hands them to `obs_reset_audio2()` with
//! `max_buffering_ms` and `fixed_buffering` zeroed (so the reset takes the
//! default maximum and dynamic buffering), and returns what that call
//! returns. Nothing else is read, written or logged here: the gates, the
//! defaults and the "audio settings reset" message are `obs_reset_audio2`'s,
//! which stays C.

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

use crate::audio_info_queries::{AudioInfo, AudioInfo2};

/// `obs_reset_audio(oai)`: `info` is `*oai`, and `reset2` is one call of
/// `obs_reset_audio2()` with the info it receives.
#[allow(clippy::todo)]
pub fn reset_audio(info: AudioInfo, reset2: impl FnOnce(AudioInfo2) -> bool) -> bool {
    let _ = (info, reset2);
    todo!("obs_reset_audio")
}
