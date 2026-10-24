//! Safe core of `libobs/obs-source-transition-render.c.h`: the render and
//! audio step decisions of the transitions in
//! `libobs/obs-source-transition.c.h`.
//!
//! A transition renders and mixes its two children while it runs, and stops
//! once its clock reaches 1. These functions make those decisions:
//! - [`video_due`] and [`video_stop`]: the stop step of
//!   `obs_transition_video_render2()` and
//!   `obs_transition_video_render_direct()`. Due at `t >= 1` while the video
//!   transitions (a NaN `t` never is); then the video ends, and the
//!   transition stops unless its audio runs on. The direct render first ends
//!   the audio of a transition that is not active; render2 passes `active`
//!   as `true`;
//! - [`draw`]: what render2 draws: both children through the callback, or
//!   source B, or source A;
//! - [`direct_index`]: the child the direct render draws;
//! - [`audio_stop`]: the stop step of `obs_transition_audio_render()`, with
//!   or without a timestamp;
//! - [`audio_mix`]: which children the audio render mixes, or whether it
//!   copies source A;
//! - [`audio_valid`] and [`audio_place`]: the checks and the start frame of
//!   `transition_process_audio()`;
//! - [`offset`]: the scaled child size of `recalculate_transition_matrix()`
//!   and the offsets it passes to `add_alignment()`.
//!
//! `obs-source-transition.c.h` keeps the locks, the clock reads,
//! `copy_transition_state()` and the refs, `obs_transition_stop()`,
//! `handle_stop()` and the signals, `obs_source_active()`, the removed and
//! pending checks and the timestamp reads, the texture trylock, the
//! renders and the callbacks, `mix_child()` and `get_sample_time()`, the
//! mixer loop and the `memcpy()`, the offsets' `(int)` casts,
//! `add_alignment()` and the matrix calls, each where and when it called
//! them before. Nothing here keeps state, takes a lock or calls libobs.

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

/// `OBS_TRANSITION_SOURCE_B` (`enum obs_transition_target`).
pub const TARGET_B: i32 = 1;

/// `AUDIO_OUTPUT_FRAMES` (`media-io/audio-io.h`): the frames of one audio
/// tick.
pub const AUDIO_FRAMES: usize = 1024;

/// `enum obs_source_transition_render_video_step`: what a due video stop
/// step does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoStep {
    /// `OBS_SOURCE_TRANSITION_RENDER_VIDEO_END`: `transitioning_video`
    /// ends; the audio runs on.
    End,
    /// `OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP`: `transitioning_video`
    /// ends, and the C calls `obs_transition_stop()`.
    Stop,
    /// `OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP_AUDIO`: the transition is
    /// not active (the direct render): `transitioning_video` and
    /// `transitioning_audio` end, and the C calls `obs_transition_stop()`.
    StopAudio,
}

/// `obs_source_transition_render_video_due()`: whether the video stop step
/// is due at video time `t`. `t >= 1.0` in `f32`, so a NaN `t` is never due.
pub fn video_due(t: f32, transitioning_video: bool) -> bool {
    t >= 1.0 && transitioning_video
}

/// `obs_source_transition_render_video_stop()`: the due video stop step,
/// for the transition's `transitioning_audio` and whether it is `active`
/// (`obs_source_active()`, asked by the direct render only once the step is
/// due; render2 passes `true`).
pub fn video_stop(transitioning_audio: bool, active: bool) -> VideoStep {
    if !active {
        VideoStep::StopAudio
    } else if !transitioning_audio {
        VideoStep::Stop
    } else {
        VideoStep::End
    }
}

/// `enum obs_source_transition_render_draw_step`: what
/// `obs_transition_video_render2()` draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Draw {
    /// `OBS_SOURCE_TRANSITION_RENDER_DRAW_CALLBACK`: both children into the
    /// textures, blended by the callback.
    Callback,
    /// `OBS_SOURCE_TRANSITION_RENDER_DRAW_B`: source B with its matrix.
    B,
    /// `OBS_SOURCE_TRANSITION_RENDER_DRAW_A`: source A with its matrix.
    A,
}

/// `obs_source_transition_render_draw()`: the copied state's flags, whether
/// the texture trylock succeeded (`false` when it was not tried) and whether
/// a callback is set.
pub fn draw(
    transitioning_video: bool,
    textures_locked: bool,
    has_callback: bool,
    transitioning_audio: bool,
) -> Draw {
    if transitioning_video && textures_locked && has_callback {
        Draw::Callback
    } else if transitioning_audio {
        Draw::B
    } else {
        Draw::A
    }
}

/// `obs_source_transition_render_direct_index()`: the child (0 or 1) the
/// direct render draws, for whether it is `transitioning` (audio or video)
/// and the `target` asked for (an `enum obs_transition_target`, any `int`).
pub fn direct_index(transitioning: bool, target: i32) -> usize {
    usize::from(transitioning && target == TARGET_B)
}

/// `enum obs_source_transition_render_audio_step`: what the audio stop step
/// of `obs_transition_audio_render()` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioStep {
    /// `OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP`: nothing.
    Keep,
    /// `OBS_SOURCE_TRANSITION_RENDER_AUDIO_END`: `transitioning_audio`
    /// ends; the video runs on.
    End,
    /// `OBS_SOURCE_TRANSITION_RENDER_AUDIO_STOP`: `transitioning_audio`
    /// ends, and the C calls `obs_transition_stop()`.
    Stop,
}

/// `obs_source_transition_render_audio_stop()`: `t` is `calc_time()` at the
/// children's earliest timestamp, or `None` when they gave none (the C
/// then does not ask the clock).
pub fn audio_stop(
    t: Option<f32>,
    transitioning_video: bool,
    transitioning_audio: bool,
) -> AudioStep {
    let ends = match t {
        Some(t) => t >= 1.0 && transitioning_audio,
        None => !transitioning_video && transitioning_audio,
    };
    if !ends {
        AudioStep::Keep
    } else if !transitioning_video {
        // transition_stop_audio()
        AudioStep::Stop
    } else {
        AudioStep::End
    }
}

/// The result of [`audio_mix`]: the bits of
/// `obs_source_transition_render_audio_mix()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AudioMix {
    /// `OBS_SOURCE_TRANSITION_RENDER_MIX_A`: mix source A with `mix_a`.
    pub mix_a: bool,
    /// `OBS_SOURCE_TRANSITION_RENDER_MIX_B`: mix source B with `mix_b`.
    pub mix_b: bool,
    /// `OBS_SOURCE_TRANSITION_RENDER_COPY_A`: copy source A's audio.
    pub copy_a: bool,
}

/// `obs_source_transition_render_audio_mix()`: from the copied state's
/// `transitioning_audio` and the children it holds.
pub fn audio_mix(transitioning_audio: bool, has_a: bool, has_b: bool) -> AudioMix {
    AudioMix {
        mix_a: transitioning_audio && has_a,
        mix_b: transitioning_audio && has_b,
        copy_a: !transitioning_audio && has_a,
    }
}

/// `obs_source_transition_render_audio_valid()`: whether
/// `transition_process_audio()` reads a child's audio.
pub fn audio_valid(present: bool, pending: bool, duplicated: bool) -> bool {
    present && !pending && !duplicated
}

/// `enum obs_source_transition_render_audio_place`, with the start frame:
/// where a child's audio goes in `transition_process_audio()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Place {
    /// `OBS_SOURCE_TRANSITION_RENDER_PLACE_NONE`: a timestamp of 0; nothing
    /// more is read.
    None,
    /// `OBS_SOURCE_TRANSITION_RENDER_PLACE_LATE`: the child's mix is read,
    /// but it starts at this frame, past the tick; nothing is mixed.
    Late(usize),
    /// `OBS_SOURCE_TRANSITION_RENDER_PLACE_MIX`: mixed from this frame on
    /// (up to [`AUDIO_FRAMES`], so a start at `AUDIO_FRAMES` mixes 0
    /// frames).
    Mix(usize),
}

/// `obs_source_transition_render_audio_place()`: the child's audio
/// timestamp `ts`, the transition's `min_ts` and the output's
/// `sample_rate`. The frame is `ns_to_audio_frames(sample_rate, ts -
/// min_ts)` with the difference wrapping in `u64` and the C's
/// `util_mul_div64()`, cast to `usize` as the C casts to `size_t`.
pub fn audio_place(ts: u64, min_ts: u64, sample_rate: usize) -> Place {
    if ts == 0 {
        return Place::None;
    }

    let rate = u64::try_from(sample_rate).unwrap_or(u64::MAX);
    let pos = to_size(ns_to_audio_frames(rate, ts.wrapping_sub(min_ts)));
    if pos > AUDIO_FRAMES {
        Place::Late(pos)
    } else {
        Place::Mix(pos)
    }
}

/// `struct obs_source_transition_render_offset`: the offsets of
/// `recalculate_transition_matrix()`, before the C's `(int)` casts.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(C)]
pub struct Offset {
    /// `tr_cx - source_cx * scale_x`.
    pub x: f32,
    /// `tr_cy - source_cy * scale_y`.
    pub y: f32,
}

/// `obs_source_transition_render_offset()`: a child of `source_cx` by
/// `source_cy` with the fit's scale in a transition of `tr_cx` by `tr_cy`.
/// Each product is rounded to `f32`, then subtracted (never fused).
pub fn offset(
    tr_cx: f32,
    tr_cy: f32,
    source_cx: f32,
    source_cy: f32,
    scale_x: f32,
    scale_y: f32,
) -> Offset {
    let scaled_cx = source_cx * scale_x;
    let scaled_cy = source_cy * scale_y;
    Offset {
        x: tr_cx - scaled_cx,
        y: tr_cy - scaled_cy,
    }
}

/// `ns_to_audio_frames()` (`media-io/audio-io.h`): `util_mul_div64(ns,
/// sample_rate, 1000000000)`.
fn ns_to_audio_frames(sample_rate: u64, ns: u64) -> u64 {
    mul_div64(ns, sample_rate, 1_000_000_000)
}

/// `util_mul_div64()` (`util/util_uint64.h`). Outside MSVC x64 it is the
/// `uint64_t` formula `(num / div) * mul + (num % div * mul) / div`, which
/// wraps where it overflows. On MSVC x64 it is exact 128-bit math
/// (`_umul128`, `_udiv128`), which faults when the quotient does not fit in
/// 64 bits; here that saturates. `div` is never 0 here (1e9). For a sample
/// rate up to 1e9 neither overflows and both are the exact quotient.
///
/// Written out here, not taken from `obs-util`, so `obs-source` still
/// depends on no other crate (as `obs-scene`'s `audio_actions` does).
fn mul_div64(num: u64, mul: u64, div: u64) -> u64 {
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

/// The C's `(size_t)` cast of a `uint64_t`: the value on a 64-bit host, its
/// low bits on a 32-bit one.
#[allow(clippy::as_conversions)] // u64 to usize truncates as the C's cast does; there is no From
#[allow(clippy::cast_possible_truncation)]
const fn to_size(value: u64) -> usize {
    value as usize
}
