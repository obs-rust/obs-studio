//! The draw technique and colour space values shared by the source draw
//! decisions: [`crate::draw_plan`] (`libobs/obs-source-draw-plan.c.h`) and
//! [`crate::deinterlace_draw`] (`libobs/obs-source-deinterlace-draw.c.h`).
//!
//! [`Technique`] is `enum obs_source_draw_technique`
//! (`libobs/obs-source-draw-technique.h`), the technique of the default or
//! deinterlace effect a source draws with; C maps it to the name
//! (`obs_source_draw_technique_name()`). The colour spaces are
//! `enum gs_color_space`, which the decisions take as the C's `int`.

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

/// `GS_CS_SRGB` (`enum gs_color_space`, `graphics/graphics.h`): SDR.
pub const GS_CS_SRGB: i32 = 0;
/// `GS_CS_SRGB_16F`: high-precision SDR.
pub const GS_CS_SRGB_16F: i32 = 1;
/// `GS_CS_709_EXTENDED`: the canvas, Mac EDR (HDR).
pub const GS_CS_709_EXTENDED: i32 = 2;
/// `GS_CS_709_SCRGB`: 1.0 = 80 nits, Windows and Linux HDR.
pub const GS_CS_709_SCRGB: i32 = 3;

/// The nits of 1.0 in scRGB: the multiplier is the white level over it.
pub(crate) const SCRGB_WHITE: f32 = 80.0;

/// Whether a space is SDR, [`GS_CS_SRGB`] or [`GS_CS_SRGB_16F`].
pub(crate) fn sdr(space: i32) -> bool {
    space == GS_CS_SRGB || space == GS_CS_SRGB_16F
}

/// `enum obs_source_draw_technique`: the technique a source draws with; C
/// maps it to the name. The discriminants are the C's. The deinterlace
/// draw decisions return the first four only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Technique {
    /// `"Draw"`.
    Draw = 0,
    /// `"DrawMultiply"`.
    DrawMultiply = 1,
    /// `"DrawTonemap"`.
    DrawTonemap = 2,
    /// `"DrawMultiplyTonemap"`.
    DrawMultiplyTonemap = 3,
    /// `"DrawNonlinearAlpha"`: an SDR async frame with non-linear alpha.
    DrawNonlinearAlpha = 4,
    /// `"DrawNonlinearAlphaMultiply"`: the same into scRGB.
    DrawNonlinearAlphaMultiply = 5,
}

impl Technique {
    /// The C's enum value.
    pub const fn value(self) -> u32 {
        match self {
            Technique::Draw => 0,
            Technique::DrawMultiply => 1,
            Technique::DrawTonemap => 2,
            Technique::DrawMultiplyTonemap => 3,
            Technique::DrawNonlinearAlpha => 4,
            Technique::DrawNonlinearAlphaMultiply => 5,
        }
    }

    /// The technique's name in `default.effect` and the deinterlace
    /// effects, as `obs_source_draw_technique_name()` maps it.
    pub const fn name(self) -> &'static str {
        match self {
            Technique::Draw => "Draw",
            Technique::DrawMultiply => "DrawMultiply",
            Technique::DrawTonemap => "DrawTonemap",
            Technique::DrawMultiplyTonemap => "DrawMultiplyTonemap",
            Technique::DrawNonlinearAlpha => "DrawNonlinearAlpha",
            Technique::DrawNonlinearAlphaMultiply => "DrawNonlinearAlphaMultiply",
        }
    }
}
