//! `oracle_*` build of `libobs/obs-source-async-upload.c.h`
//! (oracle/source_async_upload.c): the async video upload decisions,
//! renamed. The two structs cross by value in the C layout, mirrored here;
//! the `bool` results are declared as `u8`, the byte the C returns, so a
//! parity test compares it without assuming it is 0 or 1.

use std::os::raw::c_int;

/// Index of each entry in `oracle_source_async_upload_value`, in the C's
/// order.
pub const VALUE_NAMES: [&str; 27] = [
    "sizeof(struct obs_source_async_upload_format)",
    "alignof(struct obs_source_async_upload_format)",
    "offsetof(format, width)",
    "offsetof(format, height)",
    "offsetof(format, format)",
    "offsetof(format, full_range)",
    "offsetof(format, trc)",
    "sizeof(struct obs_source_async_upload_size)",
    "alignof(struct obs_source_async_upload_size)",
    "offsetof(size, width)",
    "offsetof(size, height)",
    "offsetof(size, width_d2)",
    "offsetof(size, height_d2)",
    "offsetof(size, width_x2_i)",
    "offsetof(size, height_x2_i)",
    "GS_UNKNOWN",
    "GS_BGRX",
    "GS_BGRA",
    "VIDEO_FORMAT_BGRA",
    "VIDEO_FORMAT_BGRX",
    "VIDEO_TRC_HLG",
    "OBS_SOURCE_FRAME_LINEAR_ALPHA",
    "OBS_SOURCE_CONVERT_NONE",
    "sizeof(enum gs_color_format)",
    "sizeof(enum video_format)",
    "sizeof(bool)",
    "sizeof(float)",
];

/// `struct obs_source_async_upload_format`, as the C lays it out.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleUploadFormat {
    pub width: u32,
    pub height: u32,
    pub format: c_int,
    pub full_range: bool,
    pub trc: u8,
}

/// `struct obs_source_async_upload_size`, as the C lays it out.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OracleUploadSize {
    pub width: f32,
    pub height: f32,
    pub width_d2: f32,
    pub height_d2: f32,
    pub width_x2_i: f32,
    pub height_x2_i: f32,
}

unsafe extern "C" {
    /// `bool obs_source_async_upload_texture_changed(struct
    /// obs_source_async_upload_format cur, ... frame)`.
    pub fn oracle_obs_source_async_upload_texture_changed(
        cur: OracleUploadFormat,
        frame: OracleUploadFormat,
    ) -> u8;
    /// `bool obs_source_async_upload_tries_gpu(int convert_type)`.
    pub fn oracle_obs_source_async_upload_tries_gpu(convert_type: c_int) -> u8;
    /// `struct obs_source_async_upload_size
    /// obs_source_async_upload_size_uniforms(uint32_t cx, uint32_t cy)`.
    pub fn oracle_obs_source_async_upload_size_uniforms(cx: u32, cy: u32) -> OracleUploadSize;
    /// `float obs_source_async_upload_hlg_peak(float)`.
    pub fn oracle_obs_source_async_upload_hlg_peak(hdr_nominal_peak_level: f32) -> f32;
    /// `float obs_source_async_upload_maximum_nits(uint8_t trc, float)`.
    pub fn oracle_obs_source_async_upload_maximum_nits(trc: u8, hlg_peak_level: f32) -> f32;
    /// `float obs_source_async_upload_over_sdr_white(float, float)`.
    pub fn oracle_obs_source_async_upload_over_sdr_white(
        maximum_nits: f32,
        sdr_white_level: f32,
    ) -> f32;
    /// `float obs_source_async_upload_hdr_lw(uint16_t)`.
    pub fn oracle_obs_source_async_upload_hdr_lw(max_luminance: u16) -> f32;
    /// `bool obs_source_async_upload_sets_range(bool)`.
    pub fn oracle_obs_source_async_upload_sets_range(full_range: bool) -> u8;
    /// `bool obs_source_async_upload_linear_alpha(uint8_t)`.
    pub fn oracle_obs_source_async_upload_linear_alpha(flags: u8) -> u8;
    /// `bool obs_source_async_upload_uses_texrender(bool, bool)`.
    pub fn oracle_obs_source_async_upload_uses_texrender(
        gpu_conversion: bool,
        has_texrender: bool,
    ) -> u8;
    /// `bool obs_source_async_upload_plain(int convert_type)`.
    pub fn oracle_obs_source_async_upload_plain(convert_type: c_int) -> u8;
    /// `int obs_source_async_upload_bgr_swap(int, int)`.
    pub fn oracle_obs_source_async_upload_bgr_swap(
        texture_format: c_int,
        frame_format: c_int,
    ) -> c_int;
    /// `bool obs_source_async_upload_reanchor_reads_unbuffered(bool)`.
    pub fn oracle_obs_source_async_upload_reanchor_reads_unbuffered(decoupled: bool) -> u8;
    /// `bool obs_source_async_upload_reanchor(bool, bool)`.
    pub fn oracle_obs_source_async_upload_reanchor(decoupled: bool, unbuffered: bool) -> u8;
    /// The entry named `VALUE_NAMES[which]`; `i64::MIN` out of range.
    pub fn oracle_source_async_upload_value(which: usize) -> i64;
}
