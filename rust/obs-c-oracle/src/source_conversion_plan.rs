//! `oracle_*` build of `libobs/obs-source-conversion-plan.c.h`
//! (oracle/source_conversion_plan.c): the async video GPU conversion plan,
//! renamed.
//!
//! [`OracleConversionPlan`] mirrors `struct obs_source_conversion_plan`,
//! whose layout `oracle_source_conversion_plan_layout` reports. The enums
//! cross as `c_int`s.

use core::ffi::{c_char, c_int};

/// `OBS_SOURCE_CONVERSION_PLAN_PLANES`.
pub const PLANES: usize = 4;

/// `struct obs_source_conversion_plan`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OracleConversionPlan {
    pub convert_type: c_int,
    pub channel_count: c_int,
    pub width: [u32; PLANES],
    pub height: [u32; PLANES],
    pub format: [c_int; PLANES],
    pub technique: *const c_char,
    pub linear: bool,
}

/// The entries of `oracle_source_conversion_plan_layout`, in order.
pub const LAYOUT: [&str; 15] = [
    "sizeof(struct obs_source_conversion_plan)",
    "_Alignof(struct obs_source_conversion_plan)",
    "offsetof(plan, convert_type)",
    "offsetof(plan, channel_count)",
    "offsetof(plan, width)",
    "offsetof(plan, height)",
    "offsetof(plan, format)",
    "offsetof(plan, technique)",
    "offsetof(plan, linear)",
    "OBS_SOURCE_CONVERSION_PLAN_PLANES",
    "sizeof(enum obs_source_convert_type)",
    "sizeof(enum video_format)",
    "sizeof(enum gs_color_format)",
    "sizeof(int)",
    "sizeof(bool)",
];

/// The entries of `oracle_source_conversion_plan_value`, in order: every
/// `enum video_format`, every `enum video_trc`, the `enum gs_color_format`
/// values the plan uses and every `enum obs_source_convert_type`.
pub const VALUES: [&str; 62] = [
    "VIDEO_FORMAT_NONE",
    "VIDEO_FORMAT_I420",
    "VIDEO_FORMAT_NV12",
    "VIDEO_FORMAT_YVYU",
    "VIDEO_FORMAT_YUY2",
    "VIDEO_FORMAT_UYVY",
    "VIDEO_FORMAT_RGBA",
    "VIDEO_FORMAT_BGRA",
    "VIDEO_FORMAT_BGRX",
    "VIDEO_FORMAT_Y800",
    "VIDEO_FORMAT_I444",
    "VIDEO_FORMAT_BGR3",
    "VIDEO_FORMAT_I422",
    "VIDEO_FORMAT_I40A",
    "VIDEO_FORMAT_I42A",
    "VIDEO_FORMAT_YUVA",
    "VIDEO_FORMAT_AYUV",
    "VIDEO_FORMAT_I010",
    "VIDEO_FORMAT_P010",
    "VIDEO_FORMAT_I210",
    "VIDEO_FORMAT_I412",
    "VIDEO_FORMAT_YA2L",
    "VIDEO_FORMAT_P216",
    "VIDEO_FORMAT_P416",
    "VIDEO_FORMAT_V210",
    "VIDEO_FORMAT_R10L",
    "VIDEO_TRC_DEFAULT",
    "VIDEO_TRC_SRGB",
    "VIDEO_TRC_PQ",
    "VIDEO_TRC_HLG",
    "GS_UNKNOWN",
    "GS_R8",
    "GS_RGBA",
    "GS_BGRX",
    "GS_BGRA",
    "GS_R10G10B10A2",
    "GS_R16",
    "GS_RGBA16F",
    "GS_R8G8",
    "GS_BGRA_UNORM",
    "GS_RG16",
    "OBS_SOURCE_CONVERT_NONE",
    "OBS_SOURCE_CONVERT_NV12",
    "OBS_SOURCE_CONVERT_420",
    "OBS_SOURCE_CONVERT_420_PQ",
    "OBS_SOURCE_CONVERT_420_A",
    "OBS_SOURCE_CONVERT_422",
    "OBS_SOURCE_CONVERT_422P10LE",
    "OBS_SOURCE_CONVERT_422_A",
    "OBS_SOURCE_CONVERT_422_PACK",
    "OBS_SOURCE_CONVERT_444",
    "OBS_SOURCE_CONVERT_444P12LE",
    "OBS_SOURCE_CONVERT_444_A",
    "OBS_SOURCE_CONVERT_444P12LE_A",
    "OBS_SOURCE_CONVERT_444_A_PACK",
    "OBS_SOURCE_CONVERT_800",
    "OBS_SOURCE_CONVERT_RGB_LIMITED",
    "OBS_SOURCE_CONVERT_BGR3",
    "OBS_SOURCE_CONVERT_I010",
    "OBS_SOURCE_CONVERT_P010",
    "OBS_SOURCE_CONVERT_V210",
    "OBS_SOURCE_CONVERT_R10L",
];

unsafe extern "C" {
    /// `obs_source_conversion_plan_fill`: writes `*plan`.
    pub fn oracle_obs_source_conversion_plan_fill(
        format: c_int,
        full_range: bool,
        trc: c_int,
        width: u32,
        height: u32,
        plan: *mut OracleConversionPlan,
    );
    /// The entry named `LAYOUT[which]`; `usize::MAX` past the end.
    pub fn oracle_source_conversion_plan_layout(which: usize) -> usize;
    /// The entry named `VALUES[which]`; `i64::MIN` past the end.
    pub fn oracle_source_conversion_plan_value(which: usize) -> i64;
}
