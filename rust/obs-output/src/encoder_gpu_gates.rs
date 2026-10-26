//! Safe core of `libobs/obs-encoder-gpu-gates.c.h`: the per-frame and
//! per-encoder gates of the GPU encode thread (`obs-video-gpu-encode.c`).
//!
//! One function per decision, each asked where the C makes it.
//! `obs-video-gpu-encode.c` keeps `os_sem_wait()`, the stop flag, every
//! lock (`gpu_encoder_mutex` and the group mutex) and every read of the
//! video, the frame, the encoders, their groups and their paired encoders,
//! in its order and under its conditions: the failed flag (an atomic load),
//! the group and `first_received` tests, the walk over the weak paired
//! encoders (it stops at the first one not ready), `video_pause_check()`,
//! the group reconfigure and `info.update`, `start_ts`, the NULL test of
//! `encode_texture2`, both encode calls, `os_gettime_ns()`, the timing
//! entry, the packet send, the deques, the references, the profiler, the
//! texture creates and `obs_host_texture_shared_handle()`. It passes their
//! values here. These keep no state and call nothing in libobs.
//!
//! - `gpu_encode_thread()`, for each frame: [`warming`] and [`warm_next`]
//!   (the frames skipped while the thread starts);
//! - for each encoder: [`divisor_inc`], [`divisor_wrap`] and
//!   [`divisor_skip`] (the frame rate divisor counter), [`next_key`] (the
//!   keyed mutex key the encoder hands on) and [`record_timing`] (the frame
//!   timing entry);
//! - after the walk: [`count_next`], [`requeue`] and [`requeue_ts`] (the
//!   frame goes back to the front of the queue one interval later, or to the
//!   available queue);
//! - `init_gpu_encoding()`: [`p010`] (P010 or NV12 textures).
//!
//! The group start, the paired audio wait and the pts step are the gates
//! `receive_video()` asks, and the thread asks the same shims:
//! [`group_ready`], [`paired_late`] and [`video_pts`], re-exported from
//! [`crate::encoder_audio_start`], so the GPU and raw paths share one
//! implementation.
//!
//! Every sum and difference wraps as the C's arithmetic does (the `int`
//! counts where the C's would overflow, too). No input comes from outside
//! OBS (timestamps from the video thread, counters and keys from libobs,
//! the divisor from the frontend, the format from the video settings), and
//! nothing here can panic on any input. Nothing here divides.

pub use crate::encoder_audio_start::{group_ready, paired_late, video_pts};

/// `NUM_ENCODE_TEXTURE_FRAMES_TO_WAIT` (`obs-internal.h`): the frames the
/// thread skips while it starts.
pub const WAIT_FRAMES: i32 = 1;

/// `VIDEO_FORMAT_P010` (`media-io/video-io.h`).
pub const VIDEO_FORMAT_P010: i32 = 18;

/// `gpu_encode_thread()`: frames are still to be skipped while the thread
/// starts (`wait_frames` is not 0), so this one is.
#[must_use]
pub fn warming(wait_frames: i32) -> bool {
    let _ = wait_frames;
    todo!("encoder_gpu_gates::warming")
}

/// `gpu_encode_thread()`, for a skipped frame: the frames left to skip,
/// `wait_frames - 1`.
#[must_use]
pub fn warm_next(wait_frames: i32) -> i32 {
    let _ = wait_frames;
    todo!("encoder_gpu_gates::warm_next")
}

/// `gpu_encode_thread()`, for each encoder that reaches the divisor: the
/// counter after this frame, `counter + 1` (wrapping, a `u32` sum).
#[must_use]
pub fn divisor_inc(counter: u32) -> u32 {
    let _ = counter;
    todo!("encoder_gpu_gates::divisor_inc")
}

/// `gpu_encode_thread()`: the counter (read again after the increment) has
/// reached `frame_rate_divisor`, so it goes back to 0.
#[must_use]
pub fn divisor_wrap(counter: u32, frame_rate_divisor: u32) -> bool {
    let _ = (counter, frame_rate_divisor);
    todo!("encoder_gpu_gates::divisor_wrap")
}

/// `gpu_encode_thread()`: the counter before the increment was not 0, so
/// the encoder skips this frame.
#[must_use]
pub fn divisor_skip(skip: u32) -> bool {
    let _ = skip;
    todo!("encoder_gpu_gates::divisor_skip")
}

/// `gpu_encode_thread()`, for each encoder that encodes the frame:
/// `lock_count` (already incremented) is the number of encoders taken, so
/// the last one hands on key 0; any other hands on `next_key + 1`
/// (wrapping, a `u64` sum).
#[must_use]
pub fn next_key(lock_count: usize, num_encoders: usize, next_key: u64) -> u64 {
    let _ = (lock_count, num_encoders, next_key);
    todo!("encoder_gpu_gates::next_key")
}

/// `gpu_encode_thread()` after an encode: the frame has a timestamp (not
/// 0), so its timing entry is recorded.
#[must_use]
pub fn record_timing(frame_ts: u64) -> bool {
    let _ = frame_ts;
    todo!("encoder_gpu_gates::record_timing")
}

/// `gpu_encode_thread()` after the walk: the frame's count after this pass,
/// `count - 1`.
#[must_use]
pub fn count_next(count: i32) -> i32 {
    let _ = count;
    todo!("encoder_gpu_gates::count_next")
}

/// `gpu_encode_thread()`: the count (after [`count_next`]) is not 0, so the
/// frame goes back to the front of the queue.
#[must_use]
pub fn requeue(count: i32) -> bool {
    let _ = count;
    todo!("encoder_gpu_gates::requeue")
}

/// `gpu_encode_thread()`, for a requeued frame: its timestamp plus the frame
/// interval (wrapping, a `u64` sum).
#[must_use]
pub fn requeue_ts(timestamp: u64, interval: u64) -> u64 {
    let _ = (timestamp, interval);
    todo!("encoder_gpu_gates::requeue_ts")
}

/// `init_gpu_encoding()`: the video format is `VIDEO_FORMAT_P010`, so the
/// encode textures are P010; any other format gives NV12.
#[must_use]
pub fn p010(format: i32) -> bool {
    let _ = format;
    todo!("encoder_gpu_gates::p010")
}
