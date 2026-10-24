//! Safe core of `libobs/obs-source-deinterlace-draw.c.h`: the draw
//! decisions of deinterlacing.
//!
//! - [`effect`]: which effect a mode draws with (`get_effect()`; C loads it
//!   into its `obs->video` slot);
//! - [`linear_required`]: the modes that need linear sRGB input;
//! - [`skip`]: the early return of `deinterlace_render()`;
//! - [`reads_linear_srgb`] and [`linear_srgb`]: the linear sRGB flag, and
//!   whether `gs_get_linear_srgb()` is read for it;
//! - [`technique`], [`reads_white_level`] and [`multiplier`]: the technique
//!   and multiplier for each pair of source and current colour spaces;
//! - [`frame2`]: whether the second field is due (#395, #1995, #2048);
//! - [`mode_change`]: what `obs_source_set_deinterlace_mode()` does, a mode
//!   outside the enum being DISABLE (#2120).
//!
//! `obs-source-deinterlace.c.h` keeps every `gs_*` call in its order and
//! under the same conditions, `obs_load_effect()` and the effect slots,
//! `convert_video_space()`, `obs_get_video_sdr_white_level()`,
//! `enable_deinterlacing()`, `disable_deinterlacing()` and
//! `obs_enter_graphics()`. It passes their results in and acts on what
//! comes back. Nothing here keeps state, takes a lock or calls libobs.
//!
//! Modes and colour spaces are the C's `int`: a value outside either enum
//! takes the paths it takes in the C. The multiplier is one `f32`
//! division, the same IEEE operation as the C's, so it matches bit for
//! bit, NaNs and infinities included. The frame2 sum wraps, as `uint64_t`
//! does in the C. Characterized, not endorsed
//! (`test_deinterlace_draw_plan_steps`), and kept: a white level of 0 gives
//! +inf from scRGB, and the frame2 threshold wraps past `u64::MAX` and
//! below 0.

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

/// `OBS_DEINTERLACE_MODE_DISABLE` (`enum obs_deinterlace_mode`, `obs.h`).
pub const MODE_DISABLE: i32 = 0;
/// `OBS_DEINTERLACE_MODE_DISCARD`.
pub const MODE_DISCARD: i32 = 1;
/// `OBS_DEINTERLACE_MODE_RETRO`.
pub const MODE_RETRO: i32 = 2;
/// `OBS_DEINTERLACE_MODE_BLEND`.
pub const MODE_BLEND: i32 = 3;
/// `OBS_DEINTERLACE_MODE_BLEND_2X`.
pub const MODE_BLEND_2X: i32 = 4;
/// `OBS_DEINTERLACE_MODE_LINEAR`.
pub const MODE_LINEAR: i32 = 5;
/// `OBS_DEINTERLACE_MODE_LINEAR_2X`.
pub const MODE_LINEAR_2X: i32 = 6;
/// `OBS_DEINTERLACE_MODE_YADIF`.
pub const MODE_YADIF: i32 = 7;
/// `OBS_DEINTERLACE_MODE_YADIF_2X`.
pub const MODE_YADIF_2X: i32 = 8;

/// `GS_CS_SRGB` (`enum gs_color_space`, `graphics/graphics.h`): SDR.
pub const GS_CS_SRGB: i32 = 0;
/// `GS_CS_SRGB_16F`: high-precision SDR.
pub const GS_CS_SRGB_16F: i32 = 1;
/// `GS_CS_709_EXTENDED`: the canvas, Mac EDR (HDR).
pub const GS_CS_709_EXTENDED: i32 = 2;
/// `GS_CS_709_SCRGB`: 1.0 = 80 nits, Windows and Linux HDR.
pub const GS_CS_709_SCRGB: i32 = 3;

/// `enum obs_source_deinterlace_draw_effect`: the effect a mode draws with,
/// `obs->video.deinterlace_<name>_effect`. The discriminants are the C's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Effect {
    /// `deinterlace_discard`: DISCARD.
    Discard = 0,
    /// `deinterlace_discard_2x`: RETRO.
    Discard2x = 1,
    /// `deinterlace_blend`: BLEND.
    Blend = 2,
    /// `deinterlace_blend_2x`: BLEND_2X.
    Blend2x = 3,
    /// `deinterlace_linear`: LINEAR.
    Linear = 4,
    /// `deinterlace_linear_2x`: LINEAR_2X.
    Linear2x = 5,
    /// `deinterlace_yadif`: YADIF.
    Yadif = 6,
    /// `deinterlace_yadif_2x`: YADIF_2X.
    Yadif2x = 7,
}

/// `enum obs_source_deinterlace_draw_technique`: the technique
/// `deinterlace_render()` draws with; C maps it to the name. The
/// discriminants are the C's.
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
}

/// `enum obs_source_deinterlace_draw_mode_change`: what
/// `obs_source_set_deinterlace_mode()` does. The discriminants are the
/// C's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModeChange {
    /// Nothing: the mode is the current one.
    Same = 0,
    /// `enable_deinterlacing(source, mode)`: from DISABLE.
    Enable = 1,
    /// `disable_deinterlacing(source)`: to DISABLE, or a mode outside the
    /// enum.
    Disable = 2,
    /// Store the mode and its effect: from one mode to another.
    Switch = 3,
}

/// `obs_source_deinterlace_draw_effect()` (`get_effect()`): the effect of
/// each mode from DISCARD to YADIF_2X (RETRO draws with discard 2x); `None`
/// for DISABLE and for a mode outside the enum.
pub fn effect(mode: i32) -> Option<Effect> {
    let _ = (mode,);
    todo!()
}

/// The index of [`effect`]: 0 for DISCARD to 7 for YADIF_2X, `None` for no
/// effect. The C's enum value, -1 for none.
pub fn effect_index(mode: i32) -> Option<u8> {
    let _ = (mode,);
    todo!()
}

/// `deinterlace_draw_mode_known()`: DISABLE and the modes [`effect`] has an
/// effect for.
pub fn mode_known(mode: i32) -> bool {
    let _ = (mode,);
    todo!()
}

/// `obs_source_deinterlace_draw_linear_required()`: whether a mode needs
/// linear sRGB input: BLEND to YADIF_2X, which read neighbouring lines;
/// not DISABLE, DISCARD, RETRO or a mode outside the enum.
pub fn linear_required(mode: i32) -> bool {
    let _ = (mode,);
    todo!()
}

/// `obs_source_deinterlace_draw_skip()`: the early return of
/// `deinterlace_render()`, without the current or the previous texture or
/// with a width or height of 0.
pub fn skip(has_cur_tex: bool, has_prev_tex: bool, width: u32, height: u32) -> bool {
    let _ = (has_cur_tex, has_prev_tex, width, height);
    todo!()
}

/// `obs_source_deinterlace_draw_reads_linear_srgb()`: whether
/// `deinterlace_render()` reads `gs_get_linear_srgb()` for a source in
/// `source_space`, only for [`GS_CS_SRGB`] (the short circuit of the C's
/// expression).
pub fn reads_linear_srgb(source_space: i32) -> bool {
    let _ = (source_space,);
    todo!()
}

/// `obs_source_deinterlace_draw_linear_srgb()`: whether
/// `deinterlace_render()` draws with linear sRGB: a source not in
/// [`GS_CS_SRGB`], `gs_linear_srgb` (`gs_get_linear_srgb()`, false when it
/// was not read), or a mode that needs it ([`linear_required`]).
pub fn linear_srgb(source_space: i32, gs_linear_srgb: bool, mode: i32) -> bool {
    let _ = (source_space, gs_linear_srgb, mode);
    todo!()
}

/// `obs_source_deinterlace_draw_technique()`: the technique for a source in
/// `source_space` drawn into `current_space`. DrawMultiply for SDR or
/// [`GS_CS_709_EXTENDED`] into [`GS_CS_709_SCRGB`], DrawTonemap for
/// [`GS_CS_709_EXTENDED`] into SDR, DrawMultiplyTonemap for
/// [`GS_CS_709_SCRGB`] into SDR, DrawMultiply for [`GS_CS_709_SCRGB`] into
/// [`GS_CS_709_EXTENDED`], Draw for any other pair, a space outside the
/// enum included.
pub fn technique(source_space: i32, current_space: i32) -> Technique {
    let _ = (source_space, current_space);
    todo!()
}

/// `obs_source_deinterlace_draw_reads_white_level()`: whether
/// `deinterlace_render()` reads `obs_get_video_sdr_white_level()` for the
/// pair: when the [`technique`] multiplies.
pub fn reads_white_level(source_space: i32, current_space: i32) -> bool {
    let _ = (source_space, current_space);
    todo!()
}

/// `obs_source_deinterlace_draw_multiplier()`: `sdr_white_level / 80.0`
/// into [`GS_CS_709_SCRGB`] from SDR or [`GS_CS_709_EXTENDED`],
/// `80.0 / sdr_white_level` from [`GS_CS_709_SCRGB`] into SDR or
/// [`GS_CS_709_EXTENDED`], 1.0 for any other pair. One `f32` division, as
/// in the C: 0 gives +inf from scRGB, -0 gives -inf, a NaN stays a NaN.
pub fn multiplier(source_space: i32, current_space: i32, sdr_white_level: f32) -> f32 {
    let _ = (source_space, current_space, sdr_white_level);
    todo!()
}

/// `obs_source_deinterlace_draw_frame2()`: whether the second field is
/// due: `video_time` at or after `frame_ts + offset + half_duration -`
/// [`TWOX_TOLERANCE`], each step wrapping as the C's `uint64_t` does
/// (#395, #1995, #2048).
pub fn frame2(video_time: u64, frame_ts: u64, offset: u64, half_duration: u32) -> bool {
    let _ = (video_time, frame_ts, offset, half_duration);
    todo!()
}

/// `obs_source_deinterlace_draw_mode_change()`: what
/// `obs_source_set_deinterlace_mode()` does from the source's
/// `current_mode`. A mode outside the enum is DISABLE (#2120); then nothing
/// for the current mode, enable from DISABLE, disable to DISABLE, else
/// switch.
pub fn mode_change(current_mode: i32, mode: i32) -> ModeChange {
    let _ = (current_mode, mode);
    todo!()
}
