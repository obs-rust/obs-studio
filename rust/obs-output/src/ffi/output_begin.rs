//! C ABI for `libobs/obs-output-begin.c.h`, the begin-capture plan of
//! `obs-output.c`, declared in `libobs/obs-output-begin.h` without `EXPORT`.
//!
//! These only decide. `obs-output.c.h` (still C) reads the video info and
//! the encoders' divisors, sample rates and frame sizes, walks the encoder
//! slots and keeps the array of intervals, takes the `init_mutex` locks,
//! makes the weak references and the `paired_encoders` pushes, and stores
//! the batch size. Plain values cross, and the intervals as the caller's
//! `uint64_t` array, read only. All five stay hidden (`rust-hidden.txt`).
//!
//! Intentional differences, unreachable: the core's (where C is undefined),
//! and [`obs_output_begin_batch_size`] answers a NULL `intervals` with a
//! non-zero `num` as 0, where the C dereferences it (libobs passes its
//! darray, NULL only when empty).

use crate::output_begin as core;

/// `uint64_t obs_output_begin_video_interval(uint32_t fps_num, uint32_t
/// fps_den, uint32_t frame_rate_divisor)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_begin_video_interval(
    fps_num: u32,
    fps_den: u32,
    frame_rate_divisor: u32,
) -> u64 {
    core::video_interval(fps_num, fps_den, frame_rate_divisor)
}

/// `uint64_t obs_output_begin_audio_interval(uint32_t sample_rate, size_t
/// frame_size)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_begin_audio_interval(sample_rate: u32, frame_size: usize) -> u64 {
    core::audio_interval(sample_rate, frame_size)
}

/// `size_t obs_output_begin_batch_size(const uint64_t *intervals, size_t
/// num)`.
///
/// # Safety
///
/// `intervals` points to `num` initialized `uint64_t`s, or `num` is 0 (then
/// `intervals` may be NULL). Nothing writes them during the call. A NULL
/// `intervals` with a non-zero `num` is answered as 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_begin_batch_size(intervals: *const u64, num: usize) -> usize {
    let intervals: &[u64] = if num == 0 || intervals.is_null() {
        &[]
    } else {
        // SAFETY: the caller's contract: `num` initialized values at
        // `intervals`, not written during the call.
        unsafe { std::slice::from_raw_parts(intervals, num) }
    };
    core::batch_size(intervals)
}

/// `bool obs_output_begin_pair_video(bool video_active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_begin_pair_video(video_active: bool) -> bool {
    core::pair_video(video_active)
}

/// `bool obs_output_begin_pair_audio(bool audio_active, size_t paired_num)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_begin_pair_audio(audio_active: bool, paired_num: usize) -> bool {
    core::pair_audio(audio_active, paired_num)
}
