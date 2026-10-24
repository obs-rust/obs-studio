//! C ABI for `libobs/obs-canvas-draw.c.h`, the canvas texture draw and
//! display clear decisions, declared in `libobs/obs-canvas-draw.h` without
//! `EXPORT`.
//!
//! `obs_render_canvas_texture_internal()` in `obs.c.h` and
//! `render_display_begin()` in `obs-display.c.h` (still C) make every `gs_*`
//! call in its order, read the mix's fields, the colour spaces, the white
//! level and the host's answer, pass the values in and act on what comes
//! back. So no canvas, mix or display is mirrored, and these shims take plain
//! values, touch no memory, take no lock and call nothing in libobs. All
//! eight stay hidden (`rust-hidden.txt`).
//!
//! The technique comes back as `enum obs_source_draw_technique`
//! (`libobs/obs-source-draw-technique.h`), whose values run from 0 to 5, so
//! the C's enum type is `unsigned int` or `int` and every value fits both; it
//! crosses as `u32`, as the obs-source shims return it.
//!
//! No intentional differences from C: every input is a value, so there is
//! no pointer to check.

use core::ffi::c_int;

use crate::canvas_draw as draw;

/// `enum obs_source_draw_technique`.
#[allow(non_camel_case_types)]
pub type obs_source_draw_technique = u32;

/// `bool obs_canvas_draw_skip(bool has_mix, bool texture_rendered)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_draw_skip(has_mix: bool, texture_rendered: bool) -> bool {
    draw::skip(has_mix, texture_rendered)
}

/// `enum obs_source_draw_technique obs_canvas_draw_technique(int
/// source_space, int current_space)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_draw_technique(
    source_space: c_int,
    current_space: c_int,
) -> obs_source_draw_technique {
    draw::technique(source_space, current_space).value()
}

/// `bool obs_canvas_draw_reads_white_level(int current_space)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_draw_reads_white_level(current_space: c_int) -> bool {
    draw::reads_white_level(current_space)
}

/// `float obs_canvas_draw_multiplier(float sdr_white_level)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_draw_multiplier(sdr_white_level: f32) -> f32 {
    draw::multiplier(sdr_white_level)
}

/// `bool obs_display_clear_reads_space(bool always_srgb)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_display_clear_reads_space(always_srgb: bool) -> bool {
    draw::clear_reads_space(always_srgb)
}

/// `bool obs_display_clear_linear(int current_space)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_display_clear_linear(current_space: c_int) -> bool {
    draw::clear_linear(current_space)
}

/// `uint32_t obs_display_clear_flags(bool use_clear_workaround)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_display_clear_flags(use_clear_workaround: bool) -> u32 {
    draw::clear_flags(use_clear_workaround)
}

/// `bool obs_display_clear_draws_solid(bool use_clear_workaround)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_display_clear_draws_solid(use_clear_workaround: bool) -> bool {
    draw::clear_draws_solid(use_clear_workaround)
}
