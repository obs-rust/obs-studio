//! C ABI for `libobs/obs-source-volume-mul.c.h`, the volume multiply kernels
//! of `obs-source.c`, declared in `libobs/obs-source-volume-mul.h` without
//! `EXPORT`.
//!
//! `multiply_output_audio()` and `multiply_vol_data()` in `obs-source.c.h`
//! (still C) pass a channel buffer of the source (or a mix's first channel
//! and the whole mix's count), and `vol_data`. The shims multiply exactly
//! the `count` samples the C multiplies, take no lock and call nothing in
//! libobs. They stay hidden (`rust-hidden.txt`).
//!
//! No intentional differences.

use core::slice;

use crate::volume_mul as core_mul;

/// `void obs_source_volume_mul_block(float *out, size_t count, float vol)`.
///
/// # Safety
///
/// As the C: `out` points to `count` writable, initialized floats, or
/// `count` is 0. `apply_audio_volume()` passes a mix of the source's audio
/// output buffer and `AUDIO_OUTPUT_FRAMES * channels`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_volume_mul_block(out: *mut f32, count: usize, vol: f32) {
    if count == 0 {
        return;
    }
    // SAFETY: the caller's contract, count > 0.
    let out = unsafe { slice::from_raw_parts_mut(out, count) };
    core_mul::mul_block(out, vol);
}

/// `void obs_source_volume_mul_data(float *out, const float *vol_data,
/// size_t count)`.
///
/// # Safety
///
/// As the C: `out` points to `count` writable, initialized floats and
/// `vol_data` to `count` readable ones, not overlapping `out`, or `count`
/// is 0. `apply_audio_actions()` passes one channel and its local
/// `vol_data` of `AUDIO_OUTPUT_FRAMES`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_volume_mul_data(
    out: *mut f32,
    vol_data: *const f32,
    count: usize,
) {
    if count == 0 {
        return;
    }
    // SAFETY: the caller's contract, count > 0, no overlap.
    let (out, vol) = unsafe {
        (
            slice::from_raw_parts_mut(out, count),
            slice::from_raw_parts(vol_data, count),
        )
    };
    core_mul::mul_data(out, vol);
}
