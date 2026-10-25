//! Safe core of `libobs/obs-source-async-upload.c.h`: the async video upload
//! decisions of `obs-source.c`, from `set_async_texture_size()`,
//! `update_async_texrender()`, `update_async_textures()`,
//! `check_to_swap_bgrx_bgra()` and `obs_source_update_async_video()`.
//!
//! Not ported yet: every function is `todo!()`, so the Tier 1 tests fail.

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
    let _ = (cur, frame);
    todo!("obs_source_async_upload_texture_changed")
}

/// Whether the rebuild tries `init_gpu_conversion()` for a frame of this
/// convert type: any but [`CONVERT_NONE`].
pub fn tries_gpu(convert_type: i32) -> bool {
    let _ = convert_type;
    todo!("obs_source_async_upload_tries_gpu")
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

/// The size uniforms for an `async_width` x `async_height` texture: the
/// sizes, their halves (`(float)cx * 0.5f`) and the half texel
/// (`0.5f / (float)cx`, +inf for 0).
pub fn size_uniforms(cx: u32, cy: u32) -> SizeUniforms {
    let _ = (cx, cy);
    todo!("obs_source_async_upload_size_uniforms")
}

/// The HLG peak level from `obs->video.hdr_nominal_peak_level`: clamped to
/// 1000 (BT.2408 says higher isn't comfortable). The C's `>`: a NaN is kept,
/// payload and sign included, and so is anything at or below 1000.
pub fn hlg_peak(hdr_nominal_peak_level: f32) -> f32 {
    let _ = hdr_nominal_peak_level;
    todo!("obs_source_async_upload_hlg_peak")
}

/// The maximum nits of a frame with this transfer: the HLG peak level for
/// `VIDEO_TRC_HLG`, 10000 for any other value.
pub fn maximum_nits(trc: u8, hlg_peak_level: f32) -> f32 {
    let _ = (trc, hlg_peak_level);
    todo!("obs_source_async_upload_maximum_nits")
}

/// `maximum_over_sdr_white_nits`: `maximum_nits / sdr_white_level`, one
/// `f32` division.
pub fn over_sdr_white(maximum_nits: f32, sdr_white_level: f32) -> f32 {
    let _ = (maximum_nits, sdr_white_level);
    todo!("obs_source_async_upload_over_sdr_white")
}

/// `hdr_lw`: the frame's `max_luminance`, exactly.
pub fn hdr_lw(max_luminance: u16) -> f32 {
    let _ = max_luminance;
    todo!("obs_source_async_upload_hdr_lw")
}

/// Whether the draw sets `color_range_min` and `color_range_max`: for a
/// limited range frame only.
pub fn sets_range(full_range: bool) -> bool {
    let _ = full_range;
    todo!("obs_source_async_upload_sets_range")
}

/// `async_linear_alpha`: whether the frame's flags carry
/// [`FRAME_LINEAR_ALPHA`].
pub fn linear_alpha(flags: u8) -> bool {
    let _ = flags;
    todo!("obs_source_async_upload_linear_alpha")
}

/// Whether `update_async_textures()` draws through the texrender: a GPU
/// conversion and a texrender to draw into.
pub fn uses_texrender(gpu_conversion: bool, has_texrender: bool) -> bool {
    let _ = (gpu_conversion, has_texrender);
    todo!("obs_source_async_upload_uses_texrender")
}

/// Otherwise, whether it uploads the frame's first plane as one plain
/// texture: a frame of convert type [`CONVERT_NONE`]. Any other type
/// uploads nothing.
pub fn plain(convert_type: i32) -> bool {
    let _ = convert_type;
    todo!("obs_source_async_upload_plain")
}

/// The format `check_to_swap_bgrx_bgra()` recreates the texture with:
/// `GS_BGRA` for a `GS_BGRX` texture and a `VIDEO_FORMAT_BGRA` frame,
/// `GS_BGRX` for a `GS_BGRA` texture and a `VIDEO_FORMAT_BGRX` frame, none
/// (keep the texture) for any other pair.
pub fn bgr_swap(texture_format: i32, frame_format: i32) -> Option<i32> {
    let _ = (texture_format, frame_format);
    todo!("obs_source_async_upload_bgr_swap")
}

/// Whether `obs_source_update_async_video()` reads `async_unbuffered` for
/// the re-anchor: only for a decoupled source.
pub fn reanchor_reads_unbuffered(decoupled: bool) -> bool {
    let _ = decoupled;
    todo!("obs_source_async_upload_reanchor_reads_unbuffered")
}

/// Whether the source's timing re-anchors on the frame: unless it is
/// decoupled and unbuffered.
pub fn reanchor(decoupled: bool, unbuffered: bool) -> bool {
    let _ = (decoupled, unbuffered);
    todo!("obs_source_async_upload_reanchor")
}
