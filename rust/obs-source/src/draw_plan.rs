//! Safe core of `libobs/obs-source-draw-plan.c.h`: the source draw
//! decisions of `obs-source.c`.
//!
//! - [`rotation_x`] and [`rotation_y`] (`rotate_async_video()`): the
//!   translate that moves a rotated async frame back into its box;
//! - [`async_linear_srgb`] and [`async_nonlinear_alpha`]
//!   (`obs_source_render_async_video()`): the linear sRGB flag and the
//!   non-linear alpha blend of an async frame;
//! - [`technique`]: the technique for each pair of source and current colour
//!   spaces, of an async frame and of `source_render()`'s conversion;
//! - [`reads_white_level`] and [`multiplier`]: when the SDR white level is
//!   read and the multiplier, for both;
//! - [`convert`] (`source_render()`): whether the source is drawn through
//!   `color_space_texrender`.
//!
//! `obs-source.c.h` keeps every `gs_*` call in its order and under the same
//! conditions, `convert_video_space()`, `obs_get_video_sdr_white_level()`,
//! `gs_get_format_from_space()` and the texrenders. It passes their results
//! in and acts on what comes back. Nothing here keeps state, takes a lock or
//! calls libobs.
//!
//! Colour spaces are the C's `int`: a value outside the enum takes the paths
//! it takes in the C. The rotation is the C's `long` ([`c_long`]), so a value
//! outside 0-359 is matched as the C's `switch` matches it on every target.
//! The multiplier is one `f32` division, the same IEEE operation as the C's,
//! so it matches bit for bit, NaNs and infinities included, and the size
//! converts to `f32` rounding to nearest even, as `(float)uint32_t` does.
//! Characterized, not endorsed (`test_source_draw_plan_steps`), and kept: a
//! white level of 0 gives a multiplier of 0 into scRGB and +inf out of it.

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

use std::os::raw::c_long;

pub use crate::draw_technique::{
    GS_CS_709_EXTENDED, GS_CS_709_SCRGB, GS_CS_SRGB, GS_CS_SRGB_16F, Technique,
};
use crate::draw_technique::{SCRGB_WHITE, sdr};

/// `(float)size`: the nearest `f32`, ties to even, as the C's conversion.
/// No `From<u32>` for `f32` exists because sizes past 2^24 round; the C
/// rounds them the same way.
#[allow(clippy::as_conversions)]
fn size_f32(size: u32) -> f32 {
    size as f32
}

/// `obs_source_draw_plan_rotation_x()`: the x of the translate before an
/// async frame of `width` x `height` is rotated by `rotation` degrees:
/// `height` for 270, `width` for 180, 0 for any other rotation
/// (`obs_source_set_async_rotation()` keeps it in 0-359, #2136).
pub fn rotation_x(rotation: c_long, width: u32, height: u32) -> f32 {
    match rotation {
        270 => size_f32(height),
        180 => size_f32(width),
        _ => 0.0,
    }
}

/// `obs_source_draw_plan_rotation_y()`: the y of that translate: `width`
/// for 90, `height` for 180, 0 for any other rotation.
pub fn rotation_y(rotation: c_long, width: u32, height: u32) -> f32 {
    match rotation {
        90 => size_f32(width),
        180 => size_f32(height),
        _ => 0.0,
    }
}

/// Whether a space is [`GS_CS_709_EXTENDED`] or [`GS_CS_709_SCRGB`].
fn hdr(space: i32) -> bool {
    space == GS_CS_709_EXTENDED || space == GS_CS_709_SCRGB
}

/// `obs_source_draw_plan_async_linear_srgb()`: whether
/// `obs_source_render_async_video()` draws with linear sRGB:
/// `gs_linear_srgb` (`gs_get_linear_srgb()`, always read), or a
/// [`GS_CS_SRGB`] source into any current space but [`GS_CS_SRGB`]
/// (792f8473e), or an HDR source into SDR, which is tonemapped (080095786).
pub fn async_linear_srgb(source_space: i32, current_space: i32, gs_linear_srgb: bool) -> bool {
    gs_linear_srgb
        || (source_space == GS_CS_SRGB && current_space != GS_CS_SRGB)
        || (hdr(source_space) && sdr(current_space))
}

/// `obs_source_draw_plan_async_nonlinear_alpha()`: whether the async
/// frame blends with non-linear alpha: a [`GS_CS_SRGB`] source drawn with
/// linear sRGB whose alpha is not linear (`async_linear_alpha`).
pub fn async_nonlinear_alpha(source_space: i32, linear_srgb: bool, linear_alpha: bool) -> bool {
    source_space == GS_CS_SRGB && linear_srgb && !linear_alpha
}

/// `obs_source_draw_plan_technique()`: the technique of an async frame, and
/// of `source_render()`'s conversion (`nonlinear_alpha` false). SDR into
/// [`GS_CS_709_SCRGB`] multiplies; a [`GS_CS_SRGB`] source with non-linear
/// alpha draws with DrawNonlinearAlpha into SDR or [`GS_CS_709_EXTENDED`]
/// and DrawNonlinearAlphaMultiply into [`GS_CS_709_SCRGB`];
/// [`GS_CS_709_EXTENDED`] tonemaps into SDR and multiplies into
/// [`GS_CS_709_SCRGB`]; [`GS_CS_709_SCRGB`] multiplies and tonemaps into SDR
/// and multiplies into [`GS_CS_709_EXTENDED`]; Draw for any other pair, a
/// space outside the enum included.
pub fn technique(source_space: i32, current_space: i32, nonlinear_alpha: bool) -> Technique {
    let nonlinear_alpha = nonlinear_alpha && source_space == GS_CS_SRGB;
    if sdr(source_space) {
        if current_space == GS_CS_709_SCRGB {
            return if nonlinear_alpha {
                Technique::DrawNonlinearAlphaMultiply
            } else {
                Technique::DrawMultiply
            };
        }
        if nonlinear_alpha && (sdr(current_space) || current_space == GS_CS_709_EXTENDED) {
            return Technique::DrawNonlinearAlpha;
        }
    } else if source_space == GS_CS_709_EXTENDED {
        if sdr(current_space) {
            return Technique::DrawTonemap;
        }
        if current_space == GS_CS_709_SCRGB {
            return Technique::DrawMultiply;
        }
    } else if source_space == GS_CS_709_SCRGB {
        if sdr(current_space) {
            return Technique::DrawMultiplyTonemap;
        }
        if current_space == GS_CS_709_EXTENDED {
            return Technique::DrawMultiply;
        }
    }
    Technique::Draw
}

/// Into scRGB from SDR or [`GS_CS_709_EXTENDED`].
fn into_scrgb(source_space: i32, current_space: i32) -> bool {
    current_space == GS_CS_709_SCRGB && (sdr(source_space) || source_space == GS_CS_709_EXTENDED)
}

/// From scRGB into SDR or [`GS_CS_709_EXTENDED`].
fn from_scrgb(source_space: i32, current_space: i32) -> bool {
    source_space == GS_CS_709_SCRGB && (sdr(current_space) || current_space == GS_CS_709_EXTENDED)
}

/// `obs_source_draw_plan_reads_white_level()`: whether the async draw or
/// `source_render()` reads `obs_get_video_sdr_white_level()` for the pair:
/// into or out of scRGB.
pub fn reads_white_level(source_space: i32, current_space: i32) -> bool {
    into_scrgb(source_space, current_space) || from_scrgb(source_space, current_space)
}

/// `obs_source_draw_plan_multiplier()`: `sdr_white_level / 80.0` into
/// [`GS_CS_709_SCRGB`] from SDR or [`GS_CS_709_EXTENDED`],
/// `80.0 / sdr_white_level` from [`GS_CS_709_SCRGB`] into SDR or
/// [`GS_CS_709_EXTENDED`], 1.0 for any other pair. One `f32` division, as
/// in the C: 0 gives +inf from scRGB, -0 gives -inf, a NaN stays a NaN.
pub fn multiplier(source_space: i32, current_space: i32, sdr_white_level: f32) -> f32 {
    if into_scrgb(source_space, current_space) {
        sdr_white_level / SCRGB_WHITE
    } else if from_scrgb(source_space, current_space) {
        SCRGB_WHITE / sdr_white_level
    } else {
        1.0
    }
}

/// The class of a space in the enum: SDR, [`GS_CS_709_EXTENDED`] or
/// [`GS_CS_709_SCRGB`]; `None` outside the enum.
fn class(space: i32) -> Option<u8> {
    if sdr(space) {
        Some(0)
    } else if space == GS_CS_709_EXTENDED {
        Some(1)
    } else if space == GS_CS_709_SCRGB {
        Some(2)
    } else {
        None
    }
}

/// `obs_source_draw_plan_convert()`: whether `source_render()` draws the
/// source into `color_space_texrender` and converts it: the two spaces are
/// in the enum and in different classes (SDR, [`GS_CS_709_EXTENDED`],
/// [`GS_CS_709_SCRGB`]).
pub fn convert(source_space: i32, current_space: i32) -> bool {
    match (class(source_space), class(current_space)) {
        (Some(source), Some(current)) => source != current,
        _ => false,
    }
}
