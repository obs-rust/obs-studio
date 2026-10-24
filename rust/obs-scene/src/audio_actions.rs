//! Safe core of `libobs/obs-scene-audio-actions.c.h`: the scene item
//! visibility audio ramp.
//!
//! Showing or hiding a scene item whose source has audio does not change the
//! mix at once. `obs_sceneitem_set_visible()` (and `obs_scene_add()`, for the
//! first show) queues an action, a visibility and the time of the call, and
//! the audio thread applies it on the sample of the scene's audio tick that
//! the time falls on, through a 0/1 gain buffer of [`AUDIO_OUTPUT_FRAMES`]
//! gains. Each hide it applies drops one of the item's active references.
//!
//! This is only the logic, on the item's fields. The item side stays C
//! (`obs-scene-item-audio-actions.c.h`): it takes `actions_mutex`, passes
//! the item's queue and visibility, erases the actions [`apply`] consumed,
//! and drops one active reference per hide it returns. [`apply`] and
//! [`action_due`] keep no state, take no lock and call nothing in libobs.
//!
//! Characterized, not endorsed (`test_scene_audio_action_steps`), and kept
//! as the C has them:
//! - actions out of time order: a change timed before the previous one does
//!   not rewrite the buffer, so it never reaches the gain buffer, but a hide
//!   among them is still counted. The loop, and the window of
//!   [`action_due`], stop at the first action that is not due, even when a
//!   later one is;
//! - the window rounds the tick duration down to whole ns: at 44.1 kHz an
//!   action on the last nanosecond of frame 1023 waits for the next tick;
//! - every hide is counted, without bound (the C caller takes `active_refs`
//!   below 0 when there are more hides than references);
//! - with `ts` 0 (no tick: `process_all_audio_actions()` and the
//!   preview-only video tick) every action is applied, whatever its time.
//!
//! Intentional differences from C, both where C has undefined behaviour and
//! libobs never gets there:
//! - with `ts` 0 and a buffer, the C fill is not bounded by the tick and
//!   writes past the [`AUDIO_OUTPUT_FRAMES`] gains. [`apply`] makes the same
//!   writes inside the buffer and none past it;
//! - [`action_due`] with a rate of 0: the C divides by 0. Here every action
//!   is due, as [`apply`] puts every action on frame 0 at rate 0;
//! - on MSVC x64, a frame or duration whose 128-bit quotient does not fit in
//!   64 bits (a rate above 1 GHz): the C faults, here it saturates.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `util_mul_div64` (`util/util_uint64.h`), `num * mul / div` truncated,
/// as the C computes it on each platform. Off MSVC x64 that is
/// `(num / div) * mul + (num % div * mul) / div` in wrapping `uint64_t`,
/// which differs from the exact result once `num % div * mul` overflows. On
/// MSVC x64 it is exact 128-bit math (`_umul128`, `_udiv128`), which faults
/// when the quotient does not fit in 64 bits; here that saturates. `div` is
/// never 0: it is 1e9, or a rate checked first.
///
/// Written out here, not taken from `obs-util`, so `obs-scene` links on its
/// own (`test_scene_bounds_place_rust` links the staticlib without
/// `obs-util`'s system libraries).
pub(crate) fn mul_div64(num: u64, mul: u64, div: u64) -> u64 {
    if crate::platform::MUL_DIV64_IS_UDIV128 {
        let quotient = (u128::from(num).wrapping_mul(u128::from(mul)))
            .checked_div(u128::from(div))
            .unwrap_or(0);
        u64::try_from(quotient).unwrap_or(u64::MAX)
    } else {
        let rem = num.checked_rem(div).unwrap_or(0);
        num.checked_div(div)
            .unwrap_or(0)
            .wrapping_mul(mul)
            .wrapping_add(rem.wrapping_mul(mul).checked_div(div).unwrap_or(0))
    }
}

/// `AUDIO_OUTPUT_FRAMES` (`media-io/audio-io.h`): the frames of one audio
/// tick, and the length of the gain buffer.
pub const AUDIO_OUTPUT_FRAMES: usize = 1024;

/// [`AUDIO_OUTPUT_FRAMES`] as the C compares it with a `uint64_t` frame.
const FRAMES: u64 = AUDIO_OUTPUT_FRAMES as u64;

/// Nanoseconds per second, the unit of the timestamps.
const NS: u64 = 1_000_000_000;

/// The gain buffer of one tick: 1.0 where the item is heard, 0.0 where not.
pub type Gains = [f32; AUDIO_OUTPUT_FRAMES];

/// One queued action (`struct item_action`): the visibility to change to,
/// and the time of the change, in ns (`os_gettime_ns()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemAction {
    pub visible: bool,
    pub timestamp: u64,
}

/// What [`apply`] did: the actions it consumed (always the first ones of the
/// queue), the visibility after them, and the hides among them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    pub consumed: usize,
    pub visible: bool,
    pub hides: usize,
}

/// The frame of the tick at `ts` that an action at `timestamp` falls on:
/// `floor((timestamp - ts) * sample_rate / 1e9)` through `util_mul_div64`,
/// with a timestamp before the tick on frame 0. Not bounded by the tick.
pub fn frame(timestamp: u64, ts: u64, sample_rate: u64) -> u64 {
    let since = timestamp.max(ts).wrapping_sub(ts);
    mul_div64(since, sample_rate, NS)
}

/// `obs_scene_audio_actions_apply()`: applies the actions of `actions`, in
/// order, that fall in the tick at `ts`, starting from `visible`.
///
/// It stops at the first action on frame [`AUDIO_OUTPUT_FRAMES`] or later
/// (see [`frame`]); with `ts` 0 it consumes them all, whatever their time.
/// With a buffer, it fills each frame with the visibility before the change
/// on or after it, and the rest of the tick with the visibility after the
/// last one. With `ts` 0, frames past the buffer are not written (the C
/// writes past it).
pub fn apply<I>(
    actions: I,
    visible: bool,
    ts: u64,
    sample_rate: u64,
    buf: Option<&mut Gains>,
) -> Applied
where
    I: IntoIterator<Item = ItemAction>,
{
    let mut buf = buf;
    let mut cur_visible = visible;
    let mut out = Applied {
        consumed: 0,
        visible,
        hides: 0,
    };
    let mut frame_num: u64 = 0;

    for action in actions {
        let new_frame_num = frame(action.timestamp, ts, sample_rate);

        if ts != 0 && new_frame_num >= FRAMES {
            break;
        }

        out.consumed = out.consumed.wrapping_add(1);
        out.visible = action.visible;
        if !out.visible {
            out.hides = out.hides.wrapping_add(1);
        }

        if let Some(gains) = buf.as_deref_mut()
            && new_frame_num > frame_num
        {
            fill(gains, frame_num, new_frame_num, cur_visible);
            frame_num = new_frame_num;
        }

        cur_visible = out.visible;
    }

    if let Some(gains) = buf {
        fill(gains, frame_num, FRAMES, cur_visible);
    }

    out
}

/// Writes the gain of `visible` to the frames `from..to` that are in the
/// buffer.
fn fill(gains: &mut Gains, from: u64, to: u64, visible: bool) {
    let end = to.min(FRAMES);
    if from >= end {
        return;
    }
    // Both are at most FRAMES, so they fit.
    let (Ok(from), Ok(end)) = (usize::try_from(from), usize::try_from(end)) else {
        return;
    };
    if let Some(span) = gains.get_mut(from..end) {
        span.fill(if visible { 1.0 } else { 0.0 });
    }
}

/// The duration of one tick, [`AUDIO_OUTPUT_FRAMES`] frames in ns rounded
/// down (`util_mul_div64(AUDIO_OUTPUT_FRAMES, 1e9, sample_rate)`). `None`
/// for a rate of 0, where the C divides by 0.
pub fn tick_duration(sample_rate: u64) -> Option<u64> {
    if sample_rate == 0 {
        return None;
    }
    Some(mul_div64(FRAMES, NS, sample_rate))
}

/// `obs_scene_audio_action_due()`: whether an action at `timestamp` is due
/// in the tick at `ts`, that is `ts` is 0 or `timestamp` is before `ts` plus
/// [`tick_duration`] (the sum wraps, as the C's `uint64_t` does).
///
/// Intentional difference: with a rate of 0, every action is due (the C
/// divides by 0).
pub fn action_due(timestamp: u64, ts: u64, sample_rate: u64) -> bool {
    match tick_duration(sample_rate) {
        None => true,
        Some(duration) => ts == 0 || timestamp < ts.wrapping_add(duration),
    }
}
