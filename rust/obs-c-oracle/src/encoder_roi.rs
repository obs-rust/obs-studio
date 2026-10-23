//! `oracle_*` build of `libobs/obs-encoder-roi.c.h` (oracle/encoder_roi.c).
//!
//! The ROIs are untyped pointers to a `struct obs_encoder_roi`: callers
//! lay it out themselves, and [`oracle_encoder_roi_layout`] says how the C
//! sees it.

use core::ffi::c_void;

/// Index of each entry in `oracle_encoder_roi_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 7] = [
    "sizeof(struct obs_encoder_roi)",
    "alignof(struct obs_encoder_roi)",
    "offsetof(struct obs_encoder_roi, top)",
    "offsetof(struct obs_encoder_roi, bottom)",
    "offsetof(struct obs_encoder_roi, left)",
    "offsetof(struct obs_encoder_roi, right)",
    "offsetof(struct obs_encoder_roi, priority)",
];

unsafe extern "C" {
    pub fn oracle_obs_encoder_roi_admit(caps: u32, roi: *const c_void) -> bool;

    pub fn oracle_obs_encoder_roi_scale_factors(
        scaled_width: u32,
        scaled_height: u32,
        width: u32,
        height: u32,
        scale_x: *mut f32,
        scale_y: *mut f32,
    ) -> bool;

    pub fn oracle_obs_encoder_roi_scale(
        roi: *const c_void,
        scale_x: f32,
        scale_y: f32,
        scaled: *mut c_void,
    );

    /// `OBS_ENCODER_CAP_ROI`.
    pub fn oracle_encoder_roi_cap() -> u32;

    /// Entry `which` of [`LAYOUT_NAMES`], or `usize::MAX` past the end.
    pub fn oracle_encoder_roi_layout(which: usize) -> usize;
}
