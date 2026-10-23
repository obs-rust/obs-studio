//! Safe core of `libobs/obs-encoder-roi.c.h`: the encoder region of
//! interest (ROI) checks and scaling.
//!
//! The encoder still keeps `roi_mutex`, the list of ROIs, `roi_increment`,
//! the walk over the list (last added first) and the callback, and checks
//! the encoder (`obs_encoder_valid()`) and the NULL ROI. These take and
//! return plain values, keep no state and call nothing in libobs.
//!
//! - [`admit`] (`obs_encoder_add_roi()`): whether an encoder with `caps`
//!   stores the ROI. It needs [`OBS_ENCODER_CAP_ROI`], a rectangle that is
//!   not upside down or empty, at least [`MIN_SIZE`] pixels each way, and a
//!   priority in [-1, 1] that is not NaN. The checks run in the C's order;
//!   the sizes are taken only after the upside-down check, so they never
//!   wrap.
//! - [`scale_factors`] (`obs_encoder_enum_roi()`): `scaled / size` for each
//!   direction, each side converted to the nearest `f32` first; `None` when
//!   the video has no size.
//! - [`scale`] and [`scale_edge`]: each edge is `edge as f32 * factor`, one
//!   float multiply rounded to the nearest, then NaN and below 0 are 0,
//!   2^32 and up `u32::MAX`, and the rest truncated toward 0. Rust's
//!   saturating `as u32` gives the same values on every input; the branches
//!   are written out to match the C. The priority is kept, bit for bit.
//!
//! Every input is untrusted: the ROIs come from encoder plugins, scripts
//! and the frontend. Nothing here can panic.
//!
//! No intentional differences.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `OBS_ENCODER_CAP_ROI` (`obs-encoder.h`): the encoder takes ROIs.
pub const OBS_ENCODER_CAP_ROI: u32 = 1 << 4;

/// The smallest ROI side: one 16 x 16 block.
pub const MIN_SIZE: u32 = 16;

/// 2^32: the first float past `u32::MAX`. `u32::MAX as f32` rounds to it.
const U32_END: f32 = 4_294_967_296.0;

/// `struct obs_encoder_roi` (`obs-encoder.h`), field for field: the edges
/// in pixels from the video's top and left edges, and the priority, from
/// -1 (worse) to 1 (better). `#[repr(C)]`, so the C ABI shims pass it as
/// the C struct.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Roi {
    pub top: u32,
    pub bottom: u32,
    pub left: u32,
    pub right: u32,
    pub priority: f32,
}

/// `obs_encoder_roi_admit`: whether an encoder with `caps` stores `roi`.
///
/// False without [`OBS_ENCODER_CAP_ROI`]; for an upside-down or empty
/// rectangle (bottom at or above top, or right at or left of left); for a
/// NaN priority; for a side shorter than [`MIN_SIZE`]; and for a priority
/// outside [-1, 1]. True otherwise.
#[must_use]
pub fn admit(caps: u32, roi: &Roi) -> bool {
    if caps & OBS_ENCODER_CAP_ROI == 0 {
        return false;
    }
    // Upside-down or empty: the sizes below would wrap.
    if roi.bottom <= roi.top || roi.right <= roi.left {
        return false;
    }
    // NaN is not in [-1, 1], but fails neither comparison below.
    if roi.priority.is_nan() {
        return false;
    }
    // bottom > top and right > left, so neither subtraction wraps. They
    // are written as the C's unsigned subtraction.
    if roi.bottom.wrapping_sub(roi.top) < MIN_SIZE || roi.right.wrapping_sub(roi.left) < MIN_SIZE {
        return false;
    }
    // Other invalid ROIs.
    if roi.priority < -1.0 || roi.priority > 1.0 {
        return false;
    }
    true
}

/// `obs_encoder_roi_scale_factors`: the factors from the video's size
/// (`width` x `height`) to the scaled size, `(scale_x, scale_y)`. Each
/// side is converted to the nearest `f32` before the division, as the C's
/// `(float)` casts. `None` when the video has no size (`width` or `height`
/// 0): then no ROI is passed on.
#[must_use]
pub fn scale_factors(
    scaled_width: u32,
    scaled_height: u32,
    width: u32,
    height: u32,
) -> Option<(f32, f32)> {
    if width == 0 || height == 0 {
        return None;
    }
    // C's (float) casts, then one division each.
    Some((
        scaled_width as f32 / width as f32,
        scaled_height as f32 / height as f32,
    ))
}

/// `obs_encoder_roi_scale_edge`: `edge` scaled by `factor` in `f32`, then
/// the nearest `u32`: 0 for NaN and below 0, `u32::MAX` from 2^32 up, the
/// product truncated toward 0 otherwise.
#[must_use]
pub fn scale_edge(edge: u32, factor: f32) -> u32 {
    // C's (float)edge: the nearest float, ties to even. One multiply, so
    // there is nothing to fuse or reassociate.
    let scaled = edge as f32 * factor;
    if scaled.is_nan() || scaled < 0.0 {
        return 0;
    }
    if scaled >= U32_END {
        return u32::MAX;
    }
    // In [0, 2^32), so the truncation toward 0 fits.
    scaled as u32
}

/// `obs_encoder_roi_scale`: `roi` with its top and bottom scaled by
/// `scale_y` and its left and right by `scale_x` ([`scale_edge`]), its
/// priority kept bit for bit.
#[must_use]
pub fn scale(roi: &Roi, scale_x: f32, scale_y: f32) -> Roi {
    Roi {
        top: scale_edge(roi.top, scale_y),
        bottom: scale_edge(roi.bottom, scale_y),
        left: scale_edge(roi.left, scale_x),
        right: scale_edge(roi.right, scale_x),
        priority: roi.priority,
    }
}
