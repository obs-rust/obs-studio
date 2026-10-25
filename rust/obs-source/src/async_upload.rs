//! Safe core of `libobs/obs-source-async-upload.c.h`: the async video upload
//! decisions of `obs-source.c`.
//!
//! - [`texture_changed`] and [`tries_gpu`] (`set_async_texture_size()`):
//!   whether a frame's size, format, range or transfer differs from the
//!   textures', so they are rebuilt, and whether the rebuild tries a GPU
//!   conversion;
//! - [`size_uniforms`], [`hlg_peak`], [`maximum_nits`], [`over_sdr_white`],
//!   [`hdr_lw`] and [`sets_range`] (`update_async_texrender()`): the shader
//!   parameters of the conversion draw;
//! - [`linear_alpha`], [`uses_texrender`] and [`plain`]
//!   (`update_async_textures()`): the linear alpha flag and the route, the
//!   texrender, one plain texture, or neither;
//! - [`bgr_swap`] (`check_to_swap_bgrx_bgra()`): the format the texture is
//!   recreated with;
//! - [`reanchor_reads_unbuffered`] and [`reanchor`]
//!   (`obs_source_update_async_video()`): whether the source's timing
//!   re-anchors on the frame.
//!
//! `obs-source.c.h` keeps every `gs_*` call in its order and under the same
//! conditions, `get_convert_type()`, `init_gpu_conversion()`, `log10f()` and
//! the HLG exponent, the direct read of `obs->video.hdr_nominal_peak_level`,
//! `obs_get_video_sdr_white_level()` and
//! `obs_get_video_hdr_nominal_peak_level()`. It reads each value where it did
//! before (a level only once `gs_texrender_begin()` has succeeded), passes it
//! in and acts on what comes back. Nothing here keeps state, takes a lock or
//! calls libobs.
//!
//! Every float is one `f32` operation, the same IEEE operation the C makes,
//! in its order: `(float)cx * 0.5f`, `0.5f / (float)cx`,
//! `maximum_nits / sdr_white_level`, and the size converts to `f32` rounding
//! to nearest even, as `(float)uint32_t` does. No `f64` intermediate and no
//! fused multiply-add, so every result matches bit for bit, NaNs and
//! infinities included. Nothing divides an integer.
//! Characterized, not endorsed (`test_async_upload_steps`), and kept: a size
//! of 0 gives an infinite half texel, a white level of 0 an infinite ratio,
//! and a NaN peak level stays a NaN.

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

pub use crate::conversion_plan::{gs_color_format, video_format, video_trc};

/// `OBS_SOURCE_FRAME_LINEAR_ALPHA` (`obs.h`): the frame flag of linear
/// alpha.
pub const FRAME_LINEAR_ALPHA: u8 = 1;

/// `OBS_SOURCE_CONVERT_NONE`: the convert type of a frame that needs no
/// conversion (`ConvertType::None` of the conversion plan).
pub const CONVERT_NONE: i32 = 0;

/// The values `set_async_texture_size()` compares: the frame's, or the
/// source's `async_width`, `async_height`, `async_format`,
/// `async_full_range` and `async_trc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Format {
    pub width: u32,
    pub height: u32,
    /// An `enum video_format`, as the C's `int`.
    pub format: i32,
    pub full_range: bool,
    /// An `enum video_trc`, as the frame's `uint8_t`.
    pub trc: u8,
}

/// Whether a frame's values differ from the textures' (`cur`) in any of the
/// five fields, so the textures are rebuilt.
pub fn texture_changed(cur: Format, frame: Format) -> bool {
    !(cur.width == frame.width
        && cur.height == frame.height
        && cur.format == frame.format
        && cur.full_range == frame.full_range
        && cur.trc == frame.trc)
}

/// Whether the rebuild tries `init_gpu_conversion()` for a frame of this
/// convert type: any but [`CONVERT_NONE`].
pub fn tries_gpu(convert_type: i32) -> bool {
    convert_type != CONVERT_NONE
}

/// The size uniforms of the conversion draw.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SizeUniforms {
    pub width: f32,
    pub height: f32,
    pub width_d2: f32,
    pub height_d2: f32,
    pub width_x2_i: f32,
    pub height_x2_i: f32,
}

/// `(float)size`: the nearest `f32`, ties to even, as the C's conversion.
/// No `From<u32>` for `f32` exists because sizes past 2^24 round; the C
/// rounds them the same way.
#[allow(clippy::as_conversions)]
fn size_f32(size: u32) -> f32 {
    size as f32
}

/// The size uniforms for an `async_width` x `async_height` texture: the
/// sizes, their halves (`(float)cx * 0.5f`) and the half texel
/// (`0.5f / (float)cx`, +inf for 0).
#[allow(clippy::arithmetic_side_effects)] // f32 operations, as the C's
pub fn size_uniforms(cx: u32, cy: u32) -> SizeUniforms {
    let width = size_f32(cx);
    let height = size_f32(cy);
    SizeUniforms {
        width,
        height,
        width_d2: width * 0.5,
        height_d2: height * 0.5,
        width_x2_i: 0.5 / width,
        height_x2_i: 0.5 / height,
    }
}

/// The HLG peak level from `obs->video.hdr_nominal_peak_level`: clamped to
/// 1000 (BT.2408 says higher isn't comfortable). The C's `>`: a NaN is kept,
/// payload and sign included, and so is anything at or below 1000.
pub fn hlg_peak(hdr_nominal_peak_level: f32) -> f32 {
    if hdr_nominal_peak_level > 1000.0 {
        1000.0
    } else {
        hdr_nominal_peak_level
    }
}

/// The maximum nits of a frame with this transfer: the HLG peak level for
/// `VIDEO_TRC_HLG`, 10000 for any other value.
pub fn maximum_nits(trc: u8, hlg_peak_level: f32) -> f32 {
    if trc == video_trc::HLG {
        hlg_peak_level
    } else {
        10000.0
    }
}

/// `maximum_over_sdr_white_nits`: `maximum_nits / sdr_white_level`, one
/// `f32` division.
#[allow(clippy::arithmetic_side_effects)] // f32 division, as the C's
pub fn over_sdr_white(maximum_nits: f32, sdr_white_level: f32) -> f32 {
    maximum_nits / sdr_white_level
}

/// `hdr_lw`: the frame's `max_luminance`, exactly.
pub fn hdr_lw(max_luminance: u16) -> f32 {
    f32::from(max_luminance)
}

/// Whether the draw sets `color_range_min` and `color_range_max`: for a
/// limited range frame only.
pub fn sets_range(full_range: bool) -> bool {
    !full_range
}

/// `async_linear_alpha`: whether the frame's flags carry
/// [`FRAME_LINEAR_ALPHA`].
pub fn linear_alpha(flags: u8) -> bool {
    flags & FRAME_LINEAR_ALPHA != 0
}

/// Whether `update_async_textures()` draws through the texrender: a GPU
/// conversion and a texrender to draw into.
pub fn uses_texrender(gpu_conversion: bool, has_texrender: bool) -> bool {
    gpu_conversion && has_texrender
}

/// Otherwise, whether it uploads the frame's first plane as one plain
/// texture: a frame of convert type [`CONVERT_NONE`]. Any other type
/// uploads nothing.
pub fn plain(convert_type: i32) -> bool {
    convert_type == CONVERT_NONE
}

/// The format `check_to_swap_bgrx_bgra()` recreates the texture with:
/// `GS_BGRA` for a `GS_BGRX` texture and a `VIDEO_FORMAT_BGRA` frame,
/// `GS_BGRX` for a `GS_BGRA` texture and a `VIDEO_FORMAT_BGRX` frame, none
/// (keep the texture) for any other pair.
pub fn bgr_swap(texture_format: i32, frame_format: i32) -> Option<i32> {
    if texture_format == gs_color_format::BGRX && frame_format == video_format::BGRA {
        Some(gs_color_format::BGRA)
    } else if texture_format == gs_color_format::BGRA && frame_format == video_format::BGRX {
        Some(gs_color_format::BGRX)
    } else {
        None
    }
}

/// Whether `obs_source_update_async_video()` reads `async_unbuffered` for
/// the re-anchor: only for a decoupled source.
pub fn reanchor_reads_unbuffered(decoupled: bool) -> bool {
    decoupled
}

/// Whether the source's timing re-anchors on the frame: unless it is
/// decoupled and unbuffered.
pub fn reanchor(decoupled: bool, unbuffered: bool) -> bool {
    !decoupled || !unbuffered
}
