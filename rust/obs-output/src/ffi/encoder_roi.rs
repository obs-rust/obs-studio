//! C ABI for `libobs/obs-encoder-roi.c.h`, declared in
//! `libobs/obs-encoder-roi.h` without `EXPORT`.
//!
//! `obs_encoder_add_roi()` and `obs_encoder_enum_roi()` (still C, in
//! `obs-encoder.c.h`) check the encoder and the NULL ROI, hold
//! `roi_mutex`, keep the list and `roi_increment`, walk it last added
//! first and call the callback; they pass the encoder's caps, the sizes
//! and the ROIs to these. `struct obs_encoder_roi` is mirrored as
//! [`obs_encoder_roi`] (Tier 3 checks the layout against the C). The
//! symbols stay hidden (`rust-hidden.txt`).
//!
//! The pointers are the C's: libobs never passes NULL (`roi` is checked
//! before the admit, and the factors and the scaled ROI are locals), and
//! the C would dereference it.

use crate::encoder_roi as core;

/// `struct obs_encoder_roi` (`obs-encoder.h`).
#[allow(non_camel_case_types)]
pub type obs_encoder_roi = core::Roi;

/// `bool obs_encoder_roi_admit(uint32_t caps, const struct
/// obs_encoder_roi *roi)`.
///
/// # Safety
///
/// As the C: `roi` points to a readable `struct obs_encoder_roi`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_roi_admit(caps: u32, roi: *const obs_encoder_roi) -> bool {
    // SAFETY: a readable ROI (caller contract). Copied, not borrowed.
    let roi = unsafe { roi.read() };
    core::admit(caps, &roi)
}

/// `bool obs_encoder_roi_scale_factors(uint32_t scaled_width, uint32_t
/// scaled_height, uint32_t width, uint32_t height, float *scale_x, float
/// *scale_y)`. The factors are written only when it returns true, as the
/// C.
///
/// # Safety
///
/// As the C: `scale_x` and `scale_y` point to writable floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_roi_scale_factors(
    scaled_width: u32,
    scaled_height: u32,
    width: u32,
    height: u32,
    scale_x: *mut f32,
    scale_y: *mut f32,
) -> bool {
    let Some((x, y)) = core::scale_factors(scaled_width, scaled_height, width, height) else {
        return false;
    };
    // SAFETY: two writable floats (caller contract).
    unsafe {
        scale_x.write(x);
        scale_y.write(y);
    }
    true
}

/// `void obs_encoder_roi_scale(const struct obs_encoder_roi *roi, float
/// scale_x, float scale_y, struct obs_encoder_roi *scaled)`.
///
/// `roi` is read whole before `scaled` is written, as the C's compound
/// literal is, so the two may be the same ROI.
///
/// # Safety
///
/// As the C: `roi` points to a readable and `scaled` to a writable
/// `struct obs_encoder_roi`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_roi_scale(
    roi: *const obs_encoder_roi,
    scale_x: f32,
    scale_y: f32,
    scaled: *mut obs_encoder_roi,
) {
    // SAFETY: a readable ROI (caller contract), copied before the write.
    let roi = unsafe { roi.read() };
    let out = core::scale(&roi, scale_x, scale_y);
    // SAFETY: a writable ROI (caller contract).
    unsafe { scaled.write(out) };
}
