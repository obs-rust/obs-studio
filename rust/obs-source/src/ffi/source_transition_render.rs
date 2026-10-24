//! C ABI for `libobs/obs-source-transition-render.c.h`, the render and audio
//! step decisions of the transitions, declared in
//! `libobs/obs-source-transition-render.h` without `EXPORT`.
//!
//! `obs-source-transition.c.h` (still C) reads the transition's and the
//! children's fields under its locks, asks libobs what it asked before
//! (`obs_source_active()`, the timestamps, the texture trylock), and passes
//! the results as plain values: `t` and the flags to
//! [`obs_source_transition_render_video_due`],
//! [`obs_source_transition_render_video_stop`] and
//! [`obs_source_transition_render_audio_stop`]; the copied state's flags,
//! the trylock result and whether a callback is set to
//! [`obs_source_transition_render_draw`]; the flags and the target to
//! [`obs_source_transition_render_direct_index`]; the children it holds to
//! [`obs_source_transition_render_audio_mix`] and
//! [`obs_source_transition_render_audio_valid`]; the timestamps and the
//! sample rate to [`obs_source_transition_render_audio_place`]; and the
//! sizes and the fit's scale to [`obs_source_transition_render_offset`]. So
//! `struct obs_source` is never mirrored, no source pointer crosses, and
//! these shims take no lock and call nothing in libobs. All nine stay
//! hidden (`rust-hidden.txt`).
//!
//! The place writes the start frame through a `size_t *` only where the C
//! writes it: for a late child and a mixed one, not for a timestamp of 0.
//!
//! Intentional difference from C: a null `pos` is not written (the C writes
//! through it). libobs passes the address of its own `size_t`.

use std::os::raw::c_int;

use crate::transition_render::{self as core, AudioStep, Draw, Offset, Place, VideoStep};

/// `enum obs_source_transition_render_video_step`, as the C's enum is
/// returned (an `int` or `unsigned int`; every value fits both).
#[allow(non_camel_case_types)]
pub type obs_source_transition_render_video_step = u32;
/// `OBS_SOURCE_TRANSITION_RENDER_VIDEO_END`.
pub const OBS_SOURCE_TRANSITION_RENDER_VIDEO_END: obs_source_transition_render_video_step = 0;
/// `OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP`.
pub const OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP: obs_source_transition_render_video_step = 1;
/// `OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP_AUDIO`.
pub const OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP_AUDIO: obs_source_transition_render_video_step =
    2;

/// `enum obs_source_transition_render_draw_step`.
#[allow(non_camel_case_types)]
pub type obs_source_transition_render_draw_step = u32;
/// `OBS_SOURCE_TRANSITION_RENDER_DRAW_CALLBACK`.
pub const OBS_SOURCE_TRANSITION_RENDER_DRAW_CALLBACK: obs_source_transition_render_draw_step = 0;
/// `OBS_SOURCE_TRANSITION_RENDER_DRAW_B`.
pub const OBS_SOURCE_TRANSITION_RENDER_DRAW_B: obs_source_transition_render_draw_step = 1;
/// `OBS_SOURCE_TRANSITION_RENDER_DRAW_A`.
pub const OBS_SOURCE_TRANSITION_RENDER_DRAW_A: obs_source_transition_render_draw_step = 2;

/// `enum obs_source_transition_render_audio_step`.
#[allow(non_camel_case_types)]
pub type obs_source_transition_render_audio_step = u32;
/// `OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP`.
pub const OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP: obs_source_transition_render_audio_step = 0;
/// `OBS_SOURCE_TRANSITION_RENDER_AUDIO_END`.
pub const OBS_SOURCE_TRANSITION_RENDER_AUDIO_END: obs_source_transition_render_audio_step = 1;
/// `OBS_SOURCE_TRANSITION_RENDER_AUDIO_STOP`.
pub const OBS_SOURCE_TRANSITION_RENDER_AUDIO_STOP: obs_source_transition_render_audio_step = 2;

/// `OBS_SOURCE_TRANSITION_RENDER_MIX_A`.
pub const OBS_SOURCE_TRANSITION_RENDER_MIX_A: u32 = 1 << 0;
/// `OBS_SOURCE_TRANSITION_RENDER_MIX_B`.
pub const OBS_SOURCE_TRANSITION_RENDER_MIX_B: u32 = 1 << 1;
/// `OBS_SOURCE_TRANSITION_RENDER_COPY_A`.
pub const OBS_SOURCE_TRANSITION_RENDER_COPY_A: u32 = 1 << 2;

/// `enum obs_source_transition_render_audio_place`.
#[allow(non_camel_case_types)]
pub type obs_source_transition_render_audio_place = u32;
/// `OBS_SOURCE_TRANSITION_RENDER_PLACE_NONE`.
pub const OBS_SOURCE_TRANSITION_RENDER_PLACE_NONE: obs_source_transition_render_audio_place = 0;
/// `OBS_SOURCE_TRANSITION_RENDER_PLACE_LATE`.
pub const OBS_SOURCE_TRANSITION_RENDER_PLACE_LATE: obs_source_transition_render_audio_place = 1;
/// `OBS_SOURCE_TRANSITION_RENDER_PLACE_MIX`.
pub const OBS_SOURCE_TRANSITION_RENDER_PLACE_MIX: obs_source_transition_render_audio_place = 2;

/// `struct obs_source_transition_render_offset { float x; float y; }`.
#[allow(non_camel_case_types)]
pub type obs_source_transition_render_offset = Offset;

/// `bool obs_source_transition_render_video_due(float t, bool
/// transitioning_video)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_render_video_due(
    t: f32,
    transitioning_video: bool,
) -> bool {
    core::video_due(t, transitioning_video)
}

/// `enum obs_source_transition_render_video_step
/// obs_source_transition_render_video_stop(bool transitioning_audio, bool
/// active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_render_video_stop(
    transitioning_audio: bool,
    active: bool,
) -> obs_source_transition_render_video_step {
    match core::video_stop(transitioning_audio, active) {
        VideoStep::End => OBS_SOURCE_TRANSITION_RENDER_VIDEO_END,
        VideoStep::Stop => OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP,
        VideoStep::StopAudio => OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP_AUDIO,
    }
}

/// `enum obs_source_transition_render_draw_step
/// obs_source_transition_render_draw(bool transitioning_video, bool
/// textures_locked, bool has_callback, bool transitioning_audio)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_render_draw(
    transitioning_video: bool,
    textures_locked: bool,
    has_callback: bool,
    transitioning_audio: bool,
) -> obs_source_transition_render_draw_step {
    match core::draw(
        transitioning_video,
        textures_locked,
        has_callback,
        transitioning_audio,
    ) {
        Draw::Callback => OBS_SOURCE_TRANSITION_RENDER_DRAW_CALLBACK,
        Draw::B => OBS_SOURCE_TRANSITION_RENDER_DRAW_B,
        Draw::A => OBS_SOURCE_TRANSITION_RENDER_DRAW_A,
    }
}

/// `int obs_source_transition_render_direct_index(bool transitioning, int
/// target)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_render_direct_index(
    transitioning: bool,
    target: c_int,
) -> c_int {
    // 0 or 1
    c_int::try_from(core::direct_index(transitioning, target)).unwrap_or(0)
}

/// `enum obs_source_transition_render_audio_step
/// obs_source_transition_render_audio_stop(bool has_min_ts, float t, bool
/// transitioning_video, bool transitioning_audio)`. `t` is read only when
/// `has_min_ts` is true.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_render_audio_stop(
    has_min_ts: bool,
    t: f32,
    transitioning_video: bool,
    transitioning_audio: bool,
) -> obs_source_transition_render_audio_step {
    match core::audio_stop(
        has_min_ts.then_some(t),
        transitioning_video,
        transitioning_audio,
    ) {
        AudioStep::Keep => OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP,
        AudioStep::End => OBS_SOURCE_TRANSITION_RENDER_AUDIO_END,
        AudioStep::Stop => OBS_SOURCE_TRANSITION_RENDER_AUDIO_STOP,
    }
}

/// `uint32_t obs_source_transition_render_audio_mix(bool
/// transitioning_audio, bool has_a, bool has_b)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_render_audio_mix(
    transitioning_audio: bool,
    has_a: bool,
    has_b: bool,
) -> u32 {
    let mix = core::audio_mix(transitioning_audio, has_a, has_b);
    let mut bits = 0;
    if mix.mix_a {
        bits |= OBS_SOURCE_TRANSITION_RENDER_MIX_A;
    }
    if mix.mix_b {
        bits |= OBS_SOURCE_TRANSITION_RENDER_MIX_B;
    }
    if mix.copy_a {
        bits |= OBS_SOURCE_TRANSITION_RENDER_COPY_A;
    }
    bits
}

/// `bool obs_source_transition_render_audio_valid(bool present, bool
/// pending, bool duplicated)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_render_audio_valid(
    present: bool,
    pending: bool,
    duplicated: bool,
) -> bool {
    core::audio_valid(present, pending, duplicated)
}

/// `enum obs_source_transition_render_audio_place
/// obs_source_transition_render_audio_place(uint64_t ts, uint64_t min_ts,
/// size_t sample_rate, size_t *pos)`.
///
/// # Safety
///
/// `pos` is null or valid for a write of one aligned `size_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_transition_render_audio_place(
    ts: u64,
    min_ts: u64,
    sample_rate: usize,
    pos: *mut usize,
) -> obs_source_transition_render_audio_place {
    let (place, frame) = match core::audio_place(ts, min_ts, sample_rate) {
        Place::None => return OBS_SOURCE_TRANSITION_RENDER_PLACE_NONE,
        Place::Late(frame) => (OBS_SOURCE_TRANSITION_RENDER_PLACE_LATE, frame),
        Place::Mix(frame) => (OBS_SOURCE_TRANSITION_RENDER_PLACE_MIX, frame),
    };
    if !pos.is_null() {
        // SAFETY: non-null and writable (caller contract).
        unsafe { pos.write(frame) };
    }
    place
}

/// `struct obs_source_transition_render_offset
/// obs_source_transition_render_offset(float tr_cx, float tr_cy, float
/// source_cx, float source_cy, float scale_x, float scale_y)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_render_offset(
    tr_cx: f32,
    tr_cy: f32,
    source_cx: f32,
    source_cy: f32,
    scale_x: f32,
    scale_y: f32,
) -> obs_source_transition_render_offset {
    core::offset(tr_cx, tr_cy, source_cx, source_cy, scale_x, scale_y)
}
