//! Safe core of `libobs/obs-scene-audio-window.c.h`: the audio window
//! decisions of `scene_audio_render()` in `obs-scene.c.h`.
//!
//! A scene's audio tick starts at the earliest timestamp of its items, and
//! each item's audio is added from the frame its own timestamp falls on to
//! the end of the tick. `obs-scene.c.h` keeps `audio_lock`,
//! `apply_scene_item_volume()` and `process_all_audio_actions()` (it passes
//! the volume result), `obs_source_audio_pending()`,
//! `obs_source_get_audio_timestamp()`, `obs_source_get_audio_mix()` and the
//! two mix loops, reads every item and source field itself, and asks each
//! question where the C decided it:
//! - [`pick`]: the source an item's audio is read from ([`Child`]): the show
//!   transition while it plays for a visible item, the hide transition for a
//!   hidden one, else the source;
//! - [`counts`] and [`earliest`]: the items whose timestamp takes part in
//!   the tick's (not pending, and visible or hiding), and the fold to the
//!   earliest non-zero one ([`min_timestamp`] over a whole scene);
//! - [`silent`]: no such timestamp, so the scene only processes its items'
//!   pending audio actions and reports no audio;
//! - [`frames`]: the frames of the tick an item's audio is added on
//!   ([`Frames`]), or none for a pending source, no timestamp, or a start on
//!   frame [`AUDIO_OUTPUT_FRAMES`] or later;
//! - [`mixes`]: a hidden item with no volume buffer and no hide transition
//!   is not mixed;
//! - [`adds`]: duplicated audio is read but not added.
//!
//! The item's window is three steps ([`frames`], [`mixes`], [`adds`]), not
//! one, because the C reads between them: the item's visibility only once
//! the window is known and only without a volume buffer, and
//! `audio_is_duplicated` only after `obs_source_get_audio_mix()`.
//!
//! Characterized, not endorsed (`test_scene_audio_window_steps`), and kept:
//! - an item left out of the minimum (hidden with no hide transition, or
//!   pending on the first pass) can start before it. `source_ts - timestamp`
//!   wraps in `u64`, to a frame far past the tick (skipped), or, across the
//!   end of the `u64` range, back into it (mixed);
//! - a rate of 0 puts every item on frame 0;
//! - off MSVC x64, `ns * rate / 1e9` is `util_mul_div64`'s wrapping `u64`
//!   formula, so at rates of 1 GHz and up the frame wraps (2 s at 2^63 Hz is
//!   frame 0).
//!
//! Intentional difference, where the C faults and libobs never gets there:
//! on MSVC x64 `util_mul_div64` is exact 128-bit math, which faults when
//! the frame does not fit in 64 bits (a rate above 1 GHz); here the frame
//! saturates, so the item is skipped.
//!
//! Nothing here keeps state, takes a lock or calls libobs.

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

pub use crate::audio_actions::AUDIO_OUTPUT_FRAMES;

use crate::audio_actions::mul_div64;

/// [`AUDIO_OUTPUT_FRAMES`] as the C compares it with the `uint64_t` frame.
const FRAMES: u64 = 1024;

/// Nanoseconds per second, the unit of the timestamps.
const NS: u64 = 1_000_000_000;

const _: () = assert!(AUDIO_OUTPUT_FRAMES == 1024);

/// The source `scene_audio_render()` reads an item's audio from
/// (`enum obs_scene_audio_window_child`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Child {
    /// `item->source`.
    Source,
    /// `item->show_transition`.
    Show,
    /// `item->hide_transition`.
    Hide,
}

/// `obs_scene_audio_window_pick()`: the show transition when the item is
/// visible and it is active, the hide transition when the item is hidden
/// and it is active, else the source. The caller asks for the show
/// transition only when the item is visible, and for the hide transition
/// only when the show transition is not picked and the item is hidden.
pub fn pick(visible: bool, show_active: bool, hide_active: bool) -> Child {
    if visible && show_active {
        Child::Show
    } else if !visible && hide_active {
        Child::Hide
    } else {
        Child::Source
    }
}

/// `obs_scene_audio_window_counts()`: whether an item's timestamp takes part
/// in the tick's earliest timestamp: its source is not pending, and it is
/// visible or its hide transition is active.
pub fn counts(pending: bool, visible: bool, hide_active: bool) -> bool {
    !pending && (visible || hide_active)
}

/// `obs_scene_audio_window_earliest()`: `timestamp`, the earliest so far,
/// with `source_ts` folded in. 0 is no timestamp, on either side.
pub fn earliest(timestamp: u64, source_ts: u64) -> u64 {
    if source_ts != 0 && (timestamp == 0 || source_ts < timestamp) {
        source_ts
    } else {
        timestamp
    }
}

/// The tick's timestamp over the timestamps of the items that [`counts`]:
/// [`earliest`] folded from 0, so 0 when none has one.
pub fn min_timestamp<I>(timestamps: I) -> u64
where
    I: IntoIterator<Item = u64>,
{
    timestamps.into_iter().fold(0, earliest)
}

/// `obs_scene_audio_window_silent()`: the scene has no audio this tick.
pub fn silent(timestamp: u64) -> bool {
    timestamp == 0
}

/// `ns_to_audio_frames()` (`media-io/audio-io.h`): the frame `ns` after the
/// start of a tick at `sample_rate`, `util_mul_div64(ns, sample_rate, 1e9)`
/// as the C computes it on this target.
pub fn ns_to_audio_frames(sample_rate: u64, ns: u64) -> u64 {
    mul_div64(ns, sample_rate, NS)
}

/// Where an item's audio lands in the tick
/// (`struct obs_scene_audio_window` with `mix` true).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Frames {
    /// The first frame of the tick the item's audio is added at, below
    /// [`AUDIO_OUTPUT_FRAMES`].
    pub pos: usize,
    /// The frames added, [`AUDIO_OUTPUT_FRAMES`] - `pos`.
    pub count: usize,
}

/// `obs_scene_audio_window_frames()`: `None` for a pending source, no
/// timestamp (`source_ts` 0), or a start on frame [`AUDIO_OUTPUT_FRAMES`]
/// or later; else the frames from `pos`, the [`ns_to_audio_frames`] of
/// `source_ts - timestamp` (wrapping, as the C's `uint64_t`), to the end of
/// the tick. The caller asks for `source_ts` only when the source is not
/// pending.
pub fn frames(pending: bool, source_ts: u64, timestamp: u64, sample_rate: u64) -> Option<Frames> {
    if pending || source_ts == 0 {
        return None;
    }
    let pos = ns_to_audio_frames(sample_rate, source_ts.wrapping_sub(timestamp));
    if pos >= FRAMES {
        return None;
    }
    // Below FRAMES, so it fits, and FRAMES - pos does not underflow.
    let pos = usize::try_from(pos).ok()?;
    let count = AUDIO_OUTPUT_FRAMES.checked_sub(pos)?;
    Some(Frames { pos, count })
}

/// `obs_scene_audio_window_mixes()`: whether an item in the window is
/// mixed: it has a volume buffer, or it is visible, or its hide transition
/// is active. The caller asks for the visibility only without a volume
/// buffer, and for the hide transition only when the item is hidden.
pub fn mixes(apply_buf: bool, visible: bool, hide_active: bool) -> bool {
    apply_buf || visible || hide_active
}

/// `obs_scene_audio_window_adds()`: whether the item's mix, once read, is
/// added to the scene's: not when the source's audio is duplicated.
pub fn adds(audio_is_duplicated: bool) -> bool {
    !audio_is_duplicated
}
