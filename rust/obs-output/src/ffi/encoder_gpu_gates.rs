//! C ABI for `libobs/obs-encoder-gpu-gates.c.h`, the per-frame and
//! per-encoder gates of the GPU encode thread (`obs-video-gpu-encode.c`),
//! declared in `libobs/obs-encoder-gpu-gates.h` without `EXPORT`.
//!
//! `obs-video-gpu-encode.c.h` (still C) waits on the semaphore, takes every
//! lock, reads every field of the video, the frame, the encoders, their
//! groups and paired encoders where it always did, walks the weak paired
//! encoders, calls the encoders, sends the packets and runs the profiler,
//! and passes plain values here. So neither `struct obs_encoder` nor
//! `struct obs_tex_frame` is mirrored, and these shims take no lock, call
//! nothing in libobs and dereference no pointer. They stay hidden
//! (`rust-hidden.txt`). The group start, the paired audio wait and the pts
//! step are the `obs_encoder_audio_start_*` shims in
//! [`super::encoder_audio_start`], which the thread calls too.
//!
//! Intentional difference: [`obs_encoder_gpu_gates_warm_next`] and
//! [`obs_encoder_gpu_gates_count_next`] at `INT_MIN`, where the C's
//! subtraction is undefined, wrap. The thread never gets there.

use std::ffi::c_int;

use crate::encoder_gpu_gates as core;

/// `bool obs_encoder_gpu_gates_warming(int wait_frames)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_gpu_gates_warming(wait_frames: c_int) -> bool {
    core::warming(wait_frames)
}

/// `int obs_encoder_gpu_gates_warm_next(int wait_frames)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_gpu_gates_warm_next(wait_frames: c_int) -> c_int {
    core::warm_next(wait_frames)
}

/// `uint32_t obs_encoder_gpu_gates_divisor_inc(uint32_t counter)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_gpu_gates_divisor_inc(counter: u32) -> u32 {
    core::divisor_inc(counter)
}

/// `bool obs_encoder_gpu_gates_divisor_wrap(uint32_t counter, uint32_t
/// frame_rate_divisor)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_gpu_gates_divisor_wrap(
    counter: u32,
    frame_rate_divisor: u32,
) -> bool {
    core::divisor_wrap(counter, frame_rate_divisor)
}

/// `bool obs_encoder_gpu_gates_divisor_skip(uint32_t skip)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_gpu_gates_divisor_skip(skip: u32) -> bool {
    core::divisor_skip(skip)
}

/// `uint64_t obs_encoder_gpu_gates_next_key(size_t lock_count, size_t
/// num_encoders, uint64_t next_key)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_gpu_gates_next_key(
    lock_count: usize,
    num_encoders: usize,
    next_key: u64,
) -> u64 {
    core::next_key(lock_count, num_encoders, next_key)
}

/// `bool obs_encoder_gpu_gates_record_timing(uint64_t frame_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_gpu_gates_record_timing(frame_ts: u64) -> bool {
    core::record_timing(frame_ts)
}

/// `int obs_encoder_gpu_gates_count_next(int count)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_gpu_gates_count_next(count: c_int) -> c_int {
    core::count_next(count)
}

/// `bool obs_encoder_gpu_gates_requeue(int count)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_gpu_gates_requeue(count: c_int) -> bool {
    core::requeue(count)
}

/// `uint64_t obs_encoder_gpu_gates_requeue_ts(uint64_t timestamp, uint64_t
/// interval)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_gpu_gates_requeue_ts(timestamp: u64, interval: u64) -> u64 {
    core::requeue_ts(timestamp, interval)
}

/// `bool obs_encoder_gpu_gates_p010(int format)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_gpu_gates_p010(format: c_int) -> bool {
    core::p010(format)
}
