//! C ABI for `libobs/obs-source-draw-plan.c.h`, the source draw decisions,
//! declared in `libobs/obs-source-draw-plan.h` without `EXPORT`.
//!
//! `rotate_async_video()`, `obs_source_render_async_video()` and
//! `source_render()` in `obs-source.c.h` (still C) make every `gs_*` call in
//! its order, read the colour spaces, the graphics linear sRGB flag, the
//! white level and the source's fields, pass the values in and act on what
//! comes back. So `struct obs_source` is never mirrored, and these shims take
//! plain values, touch no memory, take no lock and call nothing in libobs.
//! All eight stay hidden (`rust-hidden.txt`).
//!
//! The rotation is the C's `long` ([`c_long`]: 64 bits on Linux and macOS,
//! 32 on Windows), passed through unconverted. The technique comes back as
//! `enum obs_source_draw_technique`, shared with the deinterlace draw shims.
//!
//! No intentional differences from C: every input is a value, so there is
//! no pointer to check.

use std::os::raw::{c_int, c_long};

use super::source_draw_technique::obs_source_draw_technique;
use crate::draw_plan as core;

/// `float obs_source_draw_plan_rotation_x(long rotation, uint32_t width,
/// uint32_t height)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_draw_plan_rotation_x(
    rotation: c_long,
    width: u32,
    height: u32,
) -> f32 {
    core::rotation_x(rotation, width, height)
}

/// `float obs_source_draw_plan_rotation_y(long rotation, uint32_t width,
/// uint32_t height)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_draw_plan_rotation_y(
    rotation: c_long,
    width: u32,
    height: u32,
) -> f32 {
    core::rotation_y(rotation, width, height)
}

/// `bool obs_source_draw_plan_async_linear_srgb(int source_space, int
/// current_space, bool gs_linear_srgb)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_draw_plan_async_linear_srgb(
    source_space: c_int,
    current_space: c_int,
    gs_linear_srgb: bool,
) -> bool {
    core::async_linear_srgb(source_space, current_space, gs_linear_srgb)
}

/// `bool obs_source_draw_plan_async_nonlinear_alpha(int source_space, bool
/// linear_srgb, bool linear_alpha)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_draw_plan_async_nonlinear_alpha(
    source_space: c_int,
    linear_srgb: bool,
    linear_alpha: bool,
) -> bool {
    core::async_nonlinear_alpha(source_space, linear_srgb, linear_alpha)
}

/// `enum obs_source_draw_technique obs_source_draw_plan_technique(int
/// source_space, int current_space, bool nonlinear_alpha)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_draw_plan_technique(
    source_space: c_int,
    current_space: c_int,
    nonlinear_alpha: bool,
) -> obs_source_draw_technique {
    core::technique(source_space, current_space, nonlinear_alpha).value()
}

/// `bool obs_source_draw_plan_reads_white_level(int source_space, int
/// current_space)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_draw_plan_reads_white_level(
    source_space: c_int,
    current_space: c_int,
) -> bool {
    core::reads_white_level(source_space, current_space)
}

/// `float obs_source_draw_plan_multiplier(int source_space, int
/// current_space, float sdr_white_level)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_draw_plan_multiplier(
    source_space: c_int,
    current_space: c_int,
    sdr_white_level: f32,
) -> f32 {
    core::multiplier(source_space, current_space, sdr_white_level)
}

/// `bool obs_source_draw_plan_convert(int source_space, int
/// current_space)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_draw_plan_convert(source_space: c_int, current_space: c_int) -> bool {
    core::convert(source_space, current_space)
}
