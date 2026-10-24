//! Safe core of `libobs/obs-canvas-draw.c.h`: the canvas texture draw
//! decisions of `obs_render_canvas_texture_internal()` (`obs.c`) and the
//! display clear decisions of `render_display_begin()` (`obs-display.c`).
//!
//! `obs.c` and `obs-display.c` keep every `gs_*` call in its order and under
//! its conditions, the effect and param lookups, the blend state, the
//! swapchain, the resize, the viewport and the solid effect, and they make
//! the two queries: `obs_get_video_sdr_white_level()` and
//! `obs_host_display_clear_color_always_srgb()`. They pass the values here
//! and do what the answer says. These keep no state, take no lock and touch
//! no texture.
//!
//! - [`skip`]: whether a canvas draws at all (it needs a mix that rendered
//!   its texture).
//! - [`technique`], [`reads_white_level`] and [`multiplier`]: the technique
//!   of `default.effect` the canvas texture draws with, whether the SDR white
//!   level is read, and the multiplier it gives.
//! - [`clear_reads_space`] and [`clear_linear`]: whether the background
//!   colour converts to a linear or an sRGB clear colour.
//! - [`clear_flags`] and [`clear_draws_solid`]: what `gs_clear()` clears, and
//!   whether the solid colour workaround draws the colour instead.
//!
//! The C asks the white level only into scRGB, and the colour space for the
//! clear colour only when the host does not always clear in sRGB, so each is
//! a "reads" question and an answer from the value, called where the C
//! decided.
//!
//! [`Technique`] mirrors the three values of `enum
//! obs_source_draw_technique` (`libobs/obs-source-draw-technique.h`) these
//! decisions return, with the C's numbers and names: the same values as
//! `obs_source::draw_technique::Technique` (#2206), which obs-core cannot
//! depend on. The colour spaces are `enum gs_color_space`, taken as the C's
//! `int`; a value outside the enum takes the path it took in the C.
//!
//! No input comes from outside OBS (the spaces come from the graphics
//! subsystem and the mix, the white level from `obs_set_video_levels()`, the
//! flags from libobs and the host), but nothing here can panic on any input.
//!
//! No intentional differences from the C.

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

use core::ffi::c_int;

/// `GS_CS_SRGB` (`enum gs_color_space`, `graphics/graphics.h`): SDR.
pub const GS_CS_SRGB: c_int = 0;
/// `GS_CS_SRGB_16F`: high-precision SDR.
pub const GS_CS_SRGB_16F: c_int = 1;
/// `GS_CS_709_EXTENDED`: the canvas, Mac EDR (HDR).
pub const GS_CS_709_EXTENDED: c_int = 2;
/// `GS_CS_709_SCRGB`: 1.0 = 80 nits, Windows and Linux HDR.
pub const GS_CS_709_SCRGB: c_int = 3;

/// `GS_CLEAR_COLOR` (`OBS_DISPLAY_CLEAR_COLOR`).
pub const CLEAR_COLOR: u32 = 1 << 0;
/// `GS_CLEAR_DEPTH` (`OBS_DISPLAY_CLEAR_DEPTH`).
pub const CLEAR_DEPTH: u32 = 1 << 1;
/// `GS_CLEAR_STENCIL` (`OBS_DISPLAY_CLEAR_STENCIL`).
pub const CLEAR_STENCIL: u32 = 1 << 2;

/// The nits of 1.0 in scRGB: the multiplier is the white level over it.
pub const SCRGB_WHITE: f32 = 80.0;

/// The technique of `default.effect` the canvas texture draws with: the
/// values of `enum obs_source_draw_technique` that
/// `obs_canvas_draw_technique()` returns, with the C's discriminants. C maps
/// them to the name (`obs_source_draw_technique_name()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Technique {
    /// `"Draw"`.
    Draw = 0,
    /// `"DrawMultiply"`: into scRGB.
    DrawMultiply = 1,
    /// `"DrawTonemap"`: a 709 extended canvas into SDR.
    DrawTonemap = 2,
}

impl Technique {
    /// The C's enum value.
    #[must_use]
    pub const fn value(self) -> u32 {
        match self {
            Technique::Draw => 0,
            Technique::DrawMultiply => 1,
            Technique::DrawTonemap => 2,
        }
    }

    /// The technique's name in `default.effect`, as
    /// `obs_source_draw_technique_name()` maps it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Technique::Draw => "Draw",
            Technique::DrawMultiply => "DrawMultiply",
            Technique::DrawTonemap => "DrawTonemap",
        }
    }
}

/// `obs_canvas_draw_skip()`: whether `obs_render_canvas_texture_internal()`
/// returns before drawing, for a canvas with no mix or a mix whose texture
/// is not rendered.
#[must_use]
pub fn skip(has_mix: bool, texture_rendered: bool) -> bool {
    !has_mix || !texture_rendered
}

/// `obs_canvas_draw_technique()`: `DrawTonemap` for a
/// [`GS_CS_709_EXTENDED`] canvas into SDR, `DrawMultiply` into
/// [`GS_CS_709_SCRGB`], `Draw` otherwise.
#[must_use]
pub fn technique(source_space: c_int, current_space: c_int) -> Technique {
    match current_space {
        GS_CS_SRGB | GS_CS_SRGB_16F if source_space == GS_CS_709_EXTENDED => Technique::DrawTonemap,
        GS_CS_709_SCRGB => Technique::DrawMultiply,
        _ => Technique::Draw,
    }
}

/// `obs_canvas_draw_reads_white_level()`: whether the white level is read,
/// into [`GS_CS_709_SCRGB`] only.
#[must_use]
pub fn reads_white_level(current_space: c_int) -> bool {
    current_space == GS_CS_709_SCRGB
}

/// `obs_canvas_draw_multiplier()`: the white level over [`SCRGB_WHITE`], one
/// `f32` division.
#[must_use]
pub fn multiplier(sdr_white_level: f32) -> f32 {
    sdr_white_level / SCRGB_WHITE
}

/// `obs_display_clear_reads_space()`: whether the colour space is read for
/// the clear colour, only when the host does not always clear in sRGB.
#[must_use]
pub fn clear_reads_space(always_srgb: bool) -> bool {
    !always_srgb
}

/// `obs_display_clear_linear()`: whether the clear colour stays linear
/// (`vec4_from_rgba()`), into [`GS_CS_SRGB`] only.
#[must_use]
pub fn clear_linear(current_space: c_int) -> bool {
    current_space == GS_CS_SRGB
}

/// `obs_display_clear_flags()`: depth and stencil, and colour without the
/// clear workaround.
#[must_use]
pub fn clear_flags(use_clear_workaround: bool) -> u32 {
    let flags = CLEAR_DEPTH | CLEAR_STENCIL;
    if use_clear_workaround {
        flags
    } else {
        flags | CLEAR_COLOR
    }
}

/// `obs_display_clear_draws_solid()`: whether the solid effect draws the
/// clear colour, with the clear workaround only.
#[must_use]
pub fn clear_draws_solid(use_clear_workaround: bool) -> bool {
    use_clear_workaround
}
