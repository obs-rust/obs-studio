//! C ABI for `libobs/media-io/audio-resampler-drift.c.h`.
//!
//! `audio-resampler-ffmpeg.c` calls these inside libobs. The C file has no
//! `EXPORT`, so the symbols stay hidden (`rust-hidden.txt`). The nine
//! functions keep the C signatures: `enum audio_format` crosses as `int` (as
//! everywhere in this crate), `enum audio_resampler_comp_action` (0 to 2) as
//! `unsigned int`, and `struct audio_resampler_comp` as
//! [`Compensation`], whose layout is the C's. A null pointer, which the C
//! dereferences, is not read or written: the state reads as zeroed and the
//! answer is what that gives; `audio-resampler-ffmpeg.c` always passes the
//! resampler's own state and locals.

use core::ffi::{c_int, c_uint};

use crate::audio_io::MAX_AUDIO_CHANNELS;
use crate::audio_resampler_drift::{
    Compensation, count, delta, input_rate, output_frames, reports_drift, sample_format, set_error,
    step, upmix_row,
};

/// Same contract as C `audio_resampler_sample_format`.
#[unsafe(no_mangle)]
pub extern "C" fn audio_resampler_sample_format(format: c_int) -> c_int {
    sample_format(format)
}

/// Same contract as C `audio_resampler_upmix_row`.
///
/// # Safety
///
/// `row` is null or points to [`MAX_AUDIO_CHANNELS`] doubles the caller may
/// write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_resampler_upmix_row(
    output_ch: u32,
    row: *mut [f64; MAX_AUDIO_CHANNELS],
) -> bool {
    let Some(coefficients) = upmix_row(output_ch) else {
        return false;
    };
    // SAFETY: null or MAX_AUDIO_CHANNELS writable doubles, as the caller
    // promises.
    if let Some(out) = unsafe { row.as_mut() } {
        *out = coefficients;
    }
    true
}

/// Same contract as C `audio_resampler_comp_set_error`.
///
/// # Safety
///
/// `c` is null or points to a `struct audio_resampler_comp` the caller may
/// write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_resampler_comp_set_error(c: *mut Compensation, error_ns: i64) {
    // SAFETY: null or a writable struct audio_resampler_comp, as the caller
    // promises.
    if let Some(c) = unsafe { c.as_mut() } {
        set_error(c, error_ns);
    }
}

/// Same contract as C `audio_resampler_comp_step`.
///
/// # Safety
///
/// `c` is null or points to a `struct audio_resampler_comp` the caller may
/// write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_resampler_comp_step(
    c: *mut Compensation,
    input_freq: u32,
    in_frames: u32,
) -> c_uint {
    // SAFETY: null or a writable struct audio_resampler_comp, as the caller
    // promises.
    let action = match unsafe { c.as_mut() } {
        Some(c) => step(c, input_freq, in_frames),
        None => step(&mut Compensation::default(), input_freq, in_frames),
    };
    action.to_c()
}

/// Same contract as C `audio_resampler_comp_delta`.
///
/// # Safety
///
/// `c` is null or points to a readable `struct audio_resampler_comp`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_resampler_comp_delta(
    c: *const Compensation,
    input_freq: u32,
) -> c_int {
    // SAFETY: null or a readable struct audio_resampler_comp, as the caller
    // promises.
    match unsafe { c.as_ref() } {
        Some(c) => delta(c, input_freq),
        None => delta(&Compensation::default(), input_freq),
    }
}

/// Same contract as C `audio_resampler_output_frames`.
#[unsafe(no_mangle)]
pub extern "C" fn audio_resampler_output_frames(estimated: c_int, engaged: bool) -> c_int {
    output_frames(estimated, engaged)
}

/// Same contract as C `audio_resampler_comp_count`.
///
/// # Safety
///
/// `c` is null or points to a `struct audio_resampler_comp` the caller may
/// write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_resampler_comp_count(
    c: *mut Compensation,
    in_frames: u32,
    converted: c_int,
) {
    // SAFETY: null or a writable struct audio_resampler_comp, as the caller
    // promises.
    if let Some(c) = unsafe { c.as_mut() } {
        count(c, in_frames, converted);
    }
}

/// Same contract as C `audio_resampler_comp_reports_drift`.
///
/// # Safety
///
/// `c` is null or points to a readable `struct audio_resampler_comp`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_resampler_comp_reports_drift(c: *const Compensation) -> bool {
    // SAFETY: null or a readable struct audio_resampler_comp, as the caller
    // promises.
    unsafe { c.as_ref() }.is_some_and(reports_drift)
}

/// Same contract as C `audio_resampler_comp_input_rate`.
///
/// # Safety
///
/// `c` is null or points to a readable `struct audio_resampler_comp`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_resampler_comp_input_rate(
    c: *const Compensation,
    output_freq: u32,
) -> f64 {
    // SAFETY: null or a readable struct audio_resampler_comp, as the caller
    // promises.
    match unsafe { c.as_ref() } {
        Some(c) => input_rate(c, output_freq),
        None => input_rate(&Compensation::default(), output_freq),
    }
}
