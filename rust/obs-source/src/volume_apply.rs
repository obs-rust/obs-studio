//! Safe core of `libobs/obs-source-volume.c.h`: the volume decisions of
//! `get_source_volume()` and `apply_audio_volume()`.
//!
//! Each audio tick, `apply_audio_volume()` in `obs-source.c.h` scales a
//! source's mixes by its volume, and these are the decisions it asks for:
//! - [`action_due`]: whether the first queued audio action (a volume, mute,
//!   push-to-talk or push-to-mute change) falls inside this tick,
//!   `action_ts < audio_ts + duration`, the sum wrapping as the C's
//!   `uint64_t`. A due action hands the tick to `apply_audio_actions()`,
//!   which stays C;
//! - [`snap`]: the volume `get_source_volume()` returns, `0.0` when its gate
//!   mutes the source (through [`crate::ptm_gate`]) or the volume is within
//!   [`SNAP_EPSILON`] of 0, `1.0` within it of 1, else the volume itself,
//!   bit for bit;
//! - [`step`]: skip at exactly `1.0`, clear every mix at exactly `0.0` (so
//!   `-0.0` too) or with no mixers, else multiply; NaN multiplies;
//! - [`mixes`]: the multiply touches a mix only when it is in the source's
//!   mixers and the tick's.
//!
//! `obs-source.c.h` keeps every read and write of the source's fields, in
//! the same order, and passes the values it read: the pressed flags, the
//! mute and the volume are written by the queued actions on the mix thread,
//! `enabled` and the mixers by other threads, without atomics. It keeps the
//! action queue and its mutex, `apply_audio_actions()`,
//! `conv_frames_to_time()` (it passes the duration), the memset and the
//! multiply loops. Nothing here keeps state, takes a lock or calls libobs.
//!
//! The compares are the C's, bit for bit: `close_float()` is
//! `fabsf(f1 - f2) <= precision` in `f32`, so NaN is never close and `-0.0`
//! is close to 0; the step compares with `==`, so `-0.0 == 0.0` and NaN is
//! neither.
//!
//! Characterized, not endorsed (`test_source_volume_apply_steps`), and
//! kept: `0.9999f`, `1.0001f` and `1.0f -+ 0.0001f` round to the floats
//! just outside the snap, so none of them snaps to 1; a volume of 1 skips
//! with no mixers too; tick mixers with no bit below [`MAX_AUDIO_MIXES`]
//! neither clear nor multiply; in the last tick before the `u64` clock ends
//! `audio_ts + duration` wraps, so nothing at or after `audio_ts` is due.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// The snap distance of `close_float(volume, 0.0f, 0.0001f)` and
/// `close_float(volume, 1.0f, 0.0001f)`: the C literal `0.0001f`, the
/// nearest `f32` to 1e-4.
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

/// `close_float(f1, f2, SNAP_EPSILON)` of `graphics/math-defs.h`:
/// `fabsf(f1 - f2) <= precision`, the difference rounded to `f32` as the C
/// does.
fn close(f1: f32, f2: f32) -> bool {
    (f1 - f2).abs() <= SNAP_EPSILON
}

/// `obs_source_volume_snap()`: `0.0` when `muted` or `volume` is within
/// [`SNAP_EPSILON`] of 0 (`-0.0` and the subnormals included, and the
/// result is `+0.0`), `1.0` when it is within it of 1, else `volume` with
/// its bits (NaN and the infinities included).
pub fn snap(muted: bool, volume: f32) -> f32 {
    if muted || close(volume, 0.0) {
        return 0.0;
    }
    if close(volume, 1.0) {
        return 1.0;
    }

    volume
}

/// `obs_source_volume_action_due()`: whether an action at `action_ts` is
/// due in the tick from `audio_ts` lasting `duration`, before the tick's
/// end, `audio_ts + duration` wrapping as the C's `uint64_t`.
pub const fn action_due(action_ts: u64, audio_ts: u64, duration: u64) -> bool {
    action_ts < audio_ts.wrapping_add(duration)
}

/// `obs_source_volume_step()`: [`Step::Skip`] when `vol == 1.0`,
/// [`Step::Clear`] when `vol == 0.0` (so `-0.0` too) or `mixers == 0`, else
/// [`Step::Multiply`]. IEEE compares, so NaN multiplies.
#[allow(clippy::float_cmp, reason = "the C compares the floats exactly")]
pub fn step(vol: f32, mixers: u32) -> Step {
    if vol == 1.0 {
        return Step::Skip;
    }

    if vol == 0.0 || mixers == 0 {
        return Step::Clear;
    }

    Step::Multiply
}

/// `obs_source_volume_mixes()`: whether mix `mix` is in both `audio_mixers`
/// and `mixers`.
///
/// libobs passes `mix < MAX_AUDIO_MIXES`. Past the `u32`'s width no mix is
/// chosen; the C shifts an `int` by `mix`, which is undefined from 31 on
/// (an intentional difference there).
pub fn mixes(audio_mixers: u32, mixers: u32, mix: usize) -> bool {
    let Some(bit) = u32::try_from(mix)
        .ok()
        .and_then(|shift| 1_u32.checked_shl(shift))
    else {
        return false;
    };
    audio_mixers & bit != 0 && mixers & bit != 0
}
