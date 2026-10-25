//! C ABI for `libobs/obs-encoder-audio-start.c.h`, the encoder audio start
//! alignment and the setter gates of `obs-encoder.c`, declared in
//! `libobs/obs-encoder-audio-start.h` without `EXPORT`.
//!
//! `obs-encoder.c.h` (still C) reads every field of the encoder, its group
//! and its paired encoders where it always did, takes the group mutex and
//! the weak references, keeps the deques, `do_encode()`, the pause checks,
//! the logs and both `util_mul_div64()` conversions (the division by the
//! sample rate traps on a zero rate on MSVC x64), and passes plain values
//! here. The packet plan comes back as [`obs_encoder_audio_start_plan`]
//! (Tier 3 checks its layout against the C). So `struct obs_encoder` is
//! never mirrored, and these shims take no lock, call nothing in libobs and
//! dereference no pointer. All stay hidden (`rust-hidden.txt`).
//!
//! No intentional differences.

use std::os::raw::c_int;

use crate::encoder_audio_start::{self as core, PacketStart};

/// `struct obs_encoder_audio_start_plan` (`obs-encoder-audio-start.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_encoder_audio_start_plan {
    pub wait: bool,
    pub truncate: bool,
    pub clear: bool,
    pub from_buffer: bool,
}

/// `bool obs_encoder_audio_start_group_ready(uint64_t group_start, uint64_t
/// timestamp)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_group_ready(group_start: u64, timestamp: u64) -> bool {
    core::group_ready(group_start, timestamp)
}

/// `bool obs_encoder_audio_start_paired_late(uint64_t first_raw_ts,
/// uint64_t timestamp)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_paired_late(first_raw_ts: u64, timestamp: u64) -> bool {
    core::paired_late(first_raw_ts, timestamp)
}

/// `int64_t obs_encoder_audio_start_video_pts(int64_t cur_pts, uint32_t
/// timebase_num, uint32_t frame_rate_divisor)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_video_pts(
    cur_pts: i64,
    timebase_num: u32,
    frame_rate_divisor: u32,
) -> i64 {
    core::video_pts(cur_pts, timebase_num, frame_rate_divisor)
}

/// `size_t obs_encoder_audio_start_size(uint32_t frames, size_t
/// blocksize)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_size(frames: u32, blocksize: usize) -> usize {
    core::packet_size(frames, blocksize)
}

/// `bool obs_encoder_audio_start_paired(uint64_t start_ts, bool
/// has_paired)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_paired(start_ts: u64, has_paired: bool) -> bool {
    core::paired_start(start_ts, has_paired)
}

/// `bool obs_encoder_audio_start_unpaired(uint64_t start_ts, bool
/// has_paired)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_unpaired(start_ts: u64, has_paired: bool) -> bool {
    core::unpaired_start(start_ts, has_paired)
}

/// `bool obs_encoder_audio_start_no_video(uint64_t v_start_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_no_video(v_start_ts: u64) -> bool {
    core::no_video(v_start_ts)
}

/// `struct obs_encoder_audio_start_plan
/// obs_encoder_audio_start_packet(uint64_t timestamp, uint64_t duration_ns,
/// uint64_t v_start_ts)`: [`PacketStart::Wait`] sets `wait` alone,
/// [`PacketStart::Start`] its three flags.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_packet(
    timestamp: u64,
    duration_ns: u64,
    v_start_ts: u64,
) -> obs_encoder_audio_start_plan {
    match core::packet_start(timestamp, duration_ns, v_start_ts) {
        PacketStart::Wait => obs_encoder_audio_start_plan {
            wait: true,
            ..obs_encoder_audio_start_plan::default()
        },
        PacketStart::Start {
            truncate,
            clear,
            from_buffer,
        } => obs_encoder_audio_start_plan {
            wait: false,
            truncate,
            clear,
            from_buffer,
        },
    }
}

/// `uint64_t obs_encoder_audio_start_offset_ns(uint64_t v_start_ts,
/// uint64_t a_start_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_offset_ns(v_start_ts: u64, a_start_ts: u64) -> u64 {
    core::offset_ns(v_start_ts, a_start_ts)
}

/// `size_t obs_encoder_audio_start_offset_size(uint64_t offset_frames,
/// size_t blocksize)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_offset_size(
    offset_frames: u64,
    blocksize: usize,
) -> usize {
    core::offset_size(offset_frames, blocksize)
}

/// `bool obs_encoder_audio_start_buffer_offset(uint64_t first_raw_ts,
/// uint64_t v_start_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_buffer_offset(
    first_raw_ts: u64,
    v_start_ts: u64,
) -> bool {
    core::buffer_offset(first_raw_ts, v_start_ts)
}

/// `size_t obs_encoder_audio_start_push_size(size_t size, size_t
/// offset_size)`: 0 for `None` (nothing pushed); `Some` is never 0.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_push_size(size: usize, offset_size: usize) -> usize {
    core::push_size(size, offset_size).unwrap_or(0)
}

/// `bool obs_encoder_audio_start_can_send(size_t buffered, size_t
/// framesize_bytes)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_can_send(
    buffered: usize,
    framesize_bytes: usize,
) -> bool {
    core::can_send(buffered, framesize_bytes)
}

/// `int64_t obs_encoder_audio_start_audio_pts(int64_t cur_pts, size_t
/// framesize)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_audio_pts(cur_pts: i64, framesize: usize) -> i64 {
    core::audio_pts(cur_pts, framesize)
}

/// `bool obs_encoder_audio_start_not_video(int type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_not_video(encoder_type: c_int) -> bool {
    core::not_video(encoder_type)
}

/// `bool obs_encoder_audio_start_divisor_ok(uint32_t frame_rate_divisor)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_divisor_ok(frame_rate_divisor: u32) -> bool {
    core::divisor_ok(frame_rate_divisor)
}
