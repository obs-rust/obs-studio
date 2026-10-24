//! Safe core of `libobs/obs-source-volume.c.h`: the volume decisions of
//! `get_source_volume()` and `apply_audio_volume()`.
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

/// The snap distance of `close_float(volume, 0.0f, 0.0001f)` and
/// `close_float(volume, 1.0f, 0.0001f)`.
pub const SNAP_EPSILON: f32 = 0.0001;

/// `MAX_AUDIO_MIXES`: the mixes `apply_audio_volume()` walks.
pub const MAX_AUDIO_MIXES: usize = 6;

/// `enum obs_source_volume_op`: what `apply_audio_volume()` does with the
/// volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Step {
    /// `OBS_SOURCE_VOLUME_SKIP`: exactly 1, leave the buffers.
    Skip = 0,
    /// `OBS_SOURCE_VOLUME_CLEAR`: exactly 0 or -0, or no mixers.
    Clear = 1,
    /// `OBS_SOURCE_VOLUME_MULTIPLY`: anything else, NaN included.
    Multiply = 2,
}

impl From<Step> for u32 {
    /// The C enum's value.
    fn from(step: Step) -> Self {
        match step {
            Step::Skip => 0,
            Step::Clear => 1,
            Step::Multiply => 2,
        }
    }
}

/// `obs_source_volume_snap()`.
pub fn snap(muted: bool, volume: f32) -> f32 {
    let _ = (muted, volume);
    todo!("obs_source_volume_snap")
}

/// `obs_source_volume_action_due()`.
pub fn action_due(action_ts: u64, audio_ts: u64, duration: u64) -> bool {
    let _ = (action_ts, audio_ts, duration);
    todo!("obs_source_volume_action_due")
}

/// `obs_source_volume_step()`.
pub fn step(vol: f32, mixers: u32) -> Step {
    let _ = (vol, mixers);
    todo!("obs_source_volume_step")
}

/// `obs_source_volume_mixes()`.
pub fn mixes(audio_mixers: u32, mixers: u32, mix: usize) -> bool {
    let _ = (audio_mixers, mixers, mix);
    todo!("obs_source_volume_mixes")
}
