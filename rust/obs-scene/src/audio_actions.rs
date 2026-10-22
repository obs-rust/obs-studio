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
//!   is due, as [`apply`] puts every action on frame 0 at rate 0.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

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
    let _ = (timestamp, ts, sample_rate, NS);
    todo!()
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
    let _ = (actions, visible, ts, sample_rate, buf, FRAMES);
    todo!()
}

/// The duration of one tick, [`AUDIO_OUTPUT_FRAMES`] frames in ns rounded
/// down (`util_mul_div64(AUDIO_OUTPUT_FRAMES, 1e9, sample_rate)`). `None`
/// for a rate of 0, where the C divides by 0.
pub fn tick_duration(sample_rate: u64) -> Option<u64> {
    let _ = sample_rate;
    todo!()
}

/// `obs_scene_audio_action_due()`: whether an action at `timestamp` is due
/// in the tick at `ts`, that is `ts` is 0 or `timestamp` is before `ts` plus
/// [`tick_duration`] (the sum wraps, as the C's `uint64_t` does).
///
/// Intentional difference: with a rate of 0, every action is due (the C
/// divides by 0).
pub fn action_due(timestamp: u64, ts: u64, sample_rate: u64) -> bool {
    let _ = (timestamp, ts, sample_rate);
    todo!()
}
