//! C ABI for `libobs/obs-source-async-upload.c.h`, the async video upload
//! decisions, declared in `libobs/obs-source-async-upload.h` without
//! `EXPORT`.
//!
//! `set_async_texture_size()`, `update_async_texrender()`,
//! `update_async_textures()`, `check_to_swap_bgrx_bgra()` and
//! `obs_source_update_async_video()` in `obs-source.c.h` (still C) make every
//! `gs_*` call in its order, call `get_convert_type()`,
//! `init_gpu_conversion()` and `log10f()`, read the source's and the frame's
//! fields and the video levels where they did before, pass the values in and
//! act on what comes back. So `struct obs_source` is never mirrored, no
//! pointer crosses, and these shims touch no memory, take no lock and call
//! nothing in libobs. All fourteen stay hidden (`rust-hidden.txt`).
//!
//! The two structs cross by value, as `#[repr(C)]` mirrors of the C's; the
//! enums as `int` (`c_int`), the transfer as the frame's `uint8_t`. The
//! floats are the core's `f32` results, unconverted.
//!
//! No intentional differences from C: every input is a value, so there is
//! no pointer to check.

use std::os::raw::c_int;

use crate::async_upload::{self as core, Format, SizeUniforms, gs_color_format};

/// `struct obs_source_async_upload_format` (`obs-source-async-upload.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_source_async_upload_format {
    pub width: u32,
    pub height: u32,
    /// `int format`: an `enum video_format`.
    pub format: c_int,
    pub full_range: bool,
    /// `uint8_t trc`: an `enum video_trc`.
    pub trc: u8,
}

impl From<obs_source_async_upload_format> for Format {
    fn from(f: obs_source_async_upload_format) -> Self {
        Self {
            width: f.width,
            height: f.height,
            format: f.format,
            full_range: f.full_range,
            trc: f.trc,
        }
    }
}

/// `struct obs_source_async_upload_size` (`obs-source-async-upload.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub struct obs_source_async_upload_size {
    pub width: f32,
    pub height: f32,
    pub width_d2: f32,
    pub height_d2: f32,
    pub width_x2_i: f32,
    pub height_x2_i: f32,
}

impl From<SizeUniforms> for obs_source_async_upload_size {
    fn from(s: SizeUniforms) -> Self {
        Self {
            width: s.width,
            height: s.height,
            width_d2: s.width_d2,
            height_d2: s.height_d2,
            width_x2_i: s.width_x2_i,
            height_x2_i: s.height_x2_i,
        }
    }
}

/// `bool obs_source_async_upload_texture_changed(struct
/// obs_source_async_upload_format cur, struct obs_source_async_upload_format
/// frame)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_async_upload_texture_changed(
    cur: obs_source_async_upload_format,
    frame: obs_source_async_upload_format,
) -> bool {
    core::texture_changed(cur.into(), frame.into())
}

/// `bool obs_source_async_upload_tries_gpu(int convert_type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_async_upload_tries_gpu(convert_type: c_int) -> bool {
    core::tries_gpu(convert_type)
}

/// `struct obs_source_async_upload_size
/// obs_source_async_upload_size_uniforms(uint32_t cx, uint32_t cy)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_async_upload_size_uniforms(
    cx: u32,
    cy: u32,
) -> obs_source_async_upload_size {
    core::size_uniforms(cx, cy).into()
}

/// `float obs_source_async_upload_hlg_peak(float hdr_nominal_peak_level)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_async_upload_hlg_peak(hdr_nominal_peak_level: f32) -> f32 {
    core::hlg_peak(hdr_nominal_peak_level)
}

/// `float obs_source_async_upload_maximum_nits(uint8_t trc, float
/// hlg_peak_level)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_async_upload_maximum_nits(trc: u8, hlg_peak_level: f32) -> f32 {
    core::maximum_nits(trc, hlg_peak_level)
}

/// `float obs_source_async_upload_over_sdr_white(float maximum_nits, float
/// sdr_white_level)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_async_upload_over_sdr_white(
    maximum_nits: f32,
    sdr_white_level: f32,
) -> f32 {
    core::over_sdr_white(maximum_nits, sdr_white_level)
}

/// `float obs_source_async_upload_hdr_lw(uint16_t max_luminance)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_async_upload_hdr_lw(max_luminance: u16) -> f32 {
    core::hdr_lw(max_luminance)
}

/// `bool obs_source_async_upload_sets_range(bool full_range)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_async_upload_sets_range(full_range: bool) -> bool {
    core::sets_range(full_range)
}

/// `bool obs_source_async_upload_linear_alpha(uint8_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_async_upload_linear_alpha(flags: u8) -> bool {
    core::linear_alpha(flags)
}

/// `bool obs_source_async_upload_uses_texrender(bool gpu_conversion, bool
/// has_texrender)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_async_upload_uses_texrender(
    gpu_conversion: bool,
    has_texrender: bool,
) -> bool {
    core::uses_texrender(gpu_conversion, has_texrender)
}

/// `bool obs_source_async_upload_plain(int convert_type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_async_upload_plain(convert_type: c_int) -> bool {
    core::plain(convert_type)
}

/// `int obs_source_async_upload_bgr_swap(int texture_format, int
/// frame_format)`: the format, or `GS_UNKNOWN` to keep the texture.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_async_upload_bgr_swap(
    texture_format: c_int,
    frame_format: c_int,
) -> c_int {
    core::bgr_swap(texture_format, frame_format).unwrap_or(gs_color_format::UNKNOWN)
}

/// `bool obs_source_async_upload_reanchor_reads_unbuffered(bool
/// decoupled)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_async_upload_reanchor_reads_unbuffered(decoupled: bool) -> bool {
    core::reanchor_reads_unbuffered(decoupled)
}

/// `bool obs_source_async_upload_reanchor(bool decoupled, bool
/// unbuffered)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_async_upload_reanchor(decoupled: bool, unbuffered: bool) -> bool {
    core::reanchor(decoupled, unbuffered)
}
