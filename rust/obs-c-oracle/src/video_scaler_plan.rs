//! `oracle_*` build of `libobs/media-io/video-scaler-plan.c.h`
//! (oracle/video_scaler_plan.c).
//!
//! `enum video_scale_type`, `enum video_colorspace` and `enum
//! video_range_type` cross as `int`.

use core::ffi::c_int;

/// Index of each value in `oracle_video_scaler_plan_int`, in the C's order.
pub const INT_NAMES: [&str; 22] = [
    "VIDEO_SCALER_SWS_FAST_BILINEAR",
    "VIDEO_SCALER_SWS_BILINEAR",
    "VIDEO_SCALER_SWS_BICUBIC",
    "VIDEO_SCALER_SWS_POINT",
    "VIDEO_SCALER_SWS_AREA",
    "VIDEO_SCALER_SWS_CS_ITU709",
    "VIDEO_SCALER_SWS_CS_ITU601",
    "VIDEO_SCALER_SWS_CS_BT2020",
    "VIDEO_SCALE_DEFAULT",
    "VIDEO_SCALE_POINT",
    "VIDEO_SCALE_FAST_BILINEAR",
    "VIDEO_SCALE_BILINEAR",
    "VIDEO_SCALE_BICUBIC",
    "VIDEO_CS_DEFAULT",
    "VIDEO_CS_601",
    "VIDEO_CS_709",
    "VIDEO_CS_SRGB",
    "VIDEO_CS_2100_PQ",
    "VIDEO_CS_2100_HLG",
    "VIDEO_RANGE_DEFAULT",
    "VIDEO_RANGE_PARTIAL",
    "VIDEO_RANGE_FULL",
];

unsafe extern "C" {
    pub fn oracle_video_scaler_sws_flags(scale_type: c_int) -> c_int;
    pub fn oracle_video_scaler_sws_colorspace(color_space: c_int) -> c_int;
    pub fn oracle_video_scaler_sws_range(range: c_int) -> c_int;
    pub fn oracle_video_scaler_plane_heights(
        heights: *mut c_int,
        height: u32,
        comp_planes: *const c_int,
        log2_chroma_h: *const u8,
    );
    pub fn oracle_video_scaler_plane_copy(
        scaled_linesize: usize,
        plane_linesize: usize,
        height: usize,
        size: *mut usize,
    ) -> bool;
    /// The value at `which` in [`INT_NAMES`], or -1 past the end.
    pub fn oracle_video_scaler_plan_int(which: usize) -> i64;
}
