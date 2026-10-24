//! C ABI for `libobs/obs-source-deinterlace-draw.c.h`, the deinterlace draw
//! decisions, declared in `libobs/obs-source-deinterlace-draw.h` without
//! `EXPORT`.
//!
//! `get_effect()`, `deinterlace_render()` and
//! `obs_source_set_deinterlace_mode()` in `obs-source-deinterlace.c.h`
//! (still C) make every `gs_*` call in its order, load the effects, read
//! the colour spaces, the white level and the source's fields, pass the
//! values in and act on what comes back. So `struct obs_source` is never
//! mirrored, and these shims take plain values, touch no memory, take no
//! lock and call nothing in libobs. All ten stay hidden
//! (`rust-hidden.txt`).
//!
//! The three enums come back as the C's enums do: `enum
//! obs_source_deinterlace_draw_effect` has -1, so it is an `int`; the
//! technique and the mode change are `int` or `unsigned int`, and every
//! value fits both.
//!
//! No intentional differences from C: every input is a value, so there is
//! no pointer to check.

use std::os::raw::c_int;

use crate::deinterlace_draw::{self as core, ModeChange};

/// `enum obs_source_deinterlace_draw_effect`.
#[allow(non_camel_case_types)]
pub type obs_source_deinterlace_draw_effect = c_int;
/// `OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_NONE`.
pub const OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_NONE: obs_source_deinterlace_draw_effect = -1;

/// `enum obs_source_draw_technique` (`obs-source-draw-technique.h`),
/// shared with the source draw plan.
pub use super::source_draw_technique::obs_source_draw_technique;

/// `enum obs_source_deinterlace_draw_mode_change`.
#[allow(non_camel_case_types)]
pub type obs_source_deinterlace_draw_mode_change = u32;

const fn mode_change_value(change: ModeChange) -> obs_source_deinterlace_draw_mode_change {
    match change {
        ModeChange::Same => 0,
        ModeChange::Enable => 1,
        ModeChange::Disable => 2,
        ModeChange::Switch => 3,
    }
}

/// `enum obs_source_deinterlace_draw_effect
/// obs_source_deinterlace_draw_effect(int mode)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_draw_effect(
    mode: c_int,
) -> obs_source_deinterlace_draw_effect {
    core::effect_index(mode).map_or(OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_NONE, c_int::from)
}

/// `bool obs_source_deinterlace_draw_linear_required(int mode)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_draw_linear_required(mode: c_int) -> bool {
    core::linear_required(mode)
}

/// `bool obs_source_deinterlace_draw_skip(bool has_cur_tex, bool
/// has_prev_tex, uint32_t width, uint32_t height)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_draw_skip(
    has_cur_tex: bool,
    has_prev_tex: bool,
    width: u32,
    height: u32,
) -> bool {
    core::skip(has_cur_tex, has_prev_tex, width, height)
}

/// `bool obs_source_deinterlace_draw_reads_linear_srgb(int source_space)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_draw_reads_linear_srgb(source_space: c_int) -> bool {
    core::reads_linear_srgb(source_space)
}

/// `bool obs_source_deinterlace_draw_linear_srgb(int source_space, bool
/// gs_linear_srgb, int mode)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_draw_linear_srgb(
    source_space: c_int,
    gs_linear_srgb: bool,
    mode: c_int,
) -> bool {
    core::linear_srgb(source_space, gs_linear_srgb, mode)
}

/// `enum obs_source_draw_technique
/// obs_source_deinterlace_draw_technique(int source_space, int
/// current_space)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_draw_technique(
    source_space: c_int,
    current_space: c_int,
) -> obs_source_draw_technique {
    core::technique(source_space, current_space).value()
}

/// `bool obs_source_deinterlace_draw_reads_white_level(int source_space,
/// int current_space)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_draw_reads_white_level(
    source_space: c_int,
    current_space: c_int,
) -> bool {
    core::reads_white_level(source_space, current_space)
}

/// `float obs_source_deinterlace_draw_multiplier(int source_space, int
/// current_space, float sdr_white_level)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_draw_multiplier(
    source_space: c_int,
    current_space: c_int,
    sdr_white_level: f32,
) -> f32 {
    core::multiplier(source_space, current_space, sdr_white_level)
}

/// `bool obs_source_deinterlace_draw_frame2(uint64_t video_time, uint64_t
/// frame_ts, uint64_t offset, uint32_t half_duration)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_draw_frame2(
    video_time: u64,
    frame_ts: u64,
    offset: u64,
    half_duration: u32,
) -> bool {
    core::frame2(video_time, frame_ts, offset, half_duration)
}

/// `enum obs_source_deinterlace_draw_mode_change
/// obs_source_deinterlace_draw_mode_change(int current_mode, int mode)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_draw_mode_change(
    current_mode: c_int,
    mode: c_int,
) -> obs_source_deinterlace_draw_mode_change {
    mode_change_value(core::mode_change(current_mode, mode))
}
