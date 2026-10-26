//! `oracle_*` build of `libobs/obs-encoder-gpu-gates.c.h`
//! (oracle/encoder_gpu_gates.c): the per-frame and per-encoder gates of the
//! GPU encode thread, renamed.
//!
//! Every function takes and returns plain values; nothing here divides.
//! `oracle_obs_encoder_gpu_gates_warm_next` and `_count_next` are the C's
//! `int` subtraction: callers keep the argument above `i32::MIN`, where the
//! C is defined.

use std::ffi::c_int;

/// Index of each entry in `oracle_encoder_gpu_gates_constant`, in the C's
/// order.
pub const CONSTANT_NAMES: [&str; 2] = [
    "OBS_ENCODER_GPU_GATES_WAIT_FRAMES",
    "OBS_ENCODER_GPU_GATES_FORMAT_P010",
];

unsafe extern "C" {
    pub fn oracle_obs_encoder_gpu_gates_warming(wait_frames: c_int) -> bool;

    pub fn oracle_obs_encoder_gpu_gates_warm_next(wait_frames: c_int) -> c_int;

    pub fn oracle_obs_encoder_gpu_gates_divisor_inc(counter: u32) -> u32;

    pub fn oracle_obs_encoder_gpu_gates_divisor_wrap(counter: u32, frame_rate_divisor: u32)
    -> bool;

    pub fn oracle_obs_encoder_gpu_gates_divisor_skip(skip: u32) -> bool;

    pub fn oracle_obs_encoder_gpu_gates_next_key(
        lock_count: usize,
        num_encoders: usize,
        next_key: u64,
    ) -> u64;

    pub fn oracle_obs_encoder_gpu_gates_record_timing(frame_ts: u64) -> bool;

    pub fn oracle_obs_encoder_gpu_gates_count_next(count: c_int) -> c_int;

    pub fn oracle_obs_encoder_gpu_gates_requeue(count: c_int) -> bool;

    pub fn oracle_obs_encoder_gpu_gates_requeue_ts(timestamp: u64, interval: u64) -> u64;

    pub fn oracle_obs_encoder_gpu_gates_p010(format: c_int) -> bool;

    /// The constants as the C sees them, by index into [`CONSTANT_NAMES`];
    /// -1 past the end.
    pub fn oracle_encoder_gpu_gates_constant(which: usize) -> i64;
}
