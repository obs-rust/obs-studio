//! C ABI for `libobs/obs-output-raw-audio.c.h`, the raw output audio
//! admission and the capture hookup of `obs-output.c`, declared in
//! `libobs/obs-output-raw-audio.h` without `EXPORT`.
//!
//! `obs-output.c.h` (still C) calls `data_active()`, reads every field of
//! the output where it always did, takes the pause mutex (reading
//! `last_video_ts` only while `video_start_ts` is 0, and `ts_offset`),
//! keeps `audio_pause_check()`, the deques and each plane's pointer
//! advance, the `raw_audio` and `raw_audio2` callbacks, the mixer
//! connections, the encoder starts, the delay's fields and log, and every
//! conversion that divides by the sample rate (the division traps on a zero
//! rate on MSVC x64), and passes plain values here. So `struct obs_output`
//! is never mirrored, and these shims take no lock, call nothing in libobs
//! and dereference no pointer. They stay hidden (`rust-hidden.txt`).
//!
//! Intentional difference: [`obs_output_raw_audio_mix_on`] with an index
//! past the width of `size_t`, where the C's shift is undefined, says not
//! set. `start_raw_audio()` asks only 0 to 5.

use crate::raw_audio as core;

/// `bool obs_output_raw_audio_passthrough(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_passthrough(flags: u32) -> bool {
    core::passthrough(flags)
}

/// `bool obs_output_raw_audio_video_unset(uint64_t video_start_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_video_unset(video_start_ts: u64) -> bool {
    core::video_unset(video_start_ts)
}

/// `bool obs_output_raw_audio_before_video(uint64_t timestamp, uint64_t
/// video_start_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_before_video(timestamp: u64, video_start_ts: u64) -> bool {
    core::before_video(timestamp, video_start_ts)
}

/// `bool obs_output_raw_audio_ends_before(uint64_t timestamp, uint64_t
/// duration_ns, uint64_t video_start_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_ends_before(
    timestamp: u64,
    duration_ns: u64,
    video_start_ts: u64,
) -> bool {
    core::ends_before(timestamp, duration_ns, video_start_ts)
}

/// `uint64_t obs_output_raw_audio_cut_ns(uint64_t video_start_ts, uint64_t
/// timestamp)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_cut_ns(video_start_ts: u64, timestamp: u64) -> u64 {
    core::cut_ns(video_start_ts, timestamp)
}

/// `uint64_t obs_output_raw_audio_cut_ts(uint64_t timestamp, uint64_t
/// cut_ns)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_cut_ts(timestamp: u64, cut_ns: u64) -> u64 {
    core::cut_ts(timestamp, cut_ns)
}

/// `size_t obs_output_raw_audio_cut_bytes(size_t audio_size, uint64_t
/// cut_frames)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_cut_bytes(audio_size: usize, cut_frames: u64) -> usize {
    core::cut_bytes(audio_size, cut_frames)
}

/// `uint32_t obs_output_raw_audio_cut_frames(uint32_t frames, uint64_t
/// cut_frames)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_cut_frames(frames: u32, cut_frames: u64) -> u32 {
    core::cut_frames(frames, cut_frames)
}

/// `bool obs_output_raw_audio_start_unset(uint64_t audio_start_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_start_unset(audio_start_ts: u64) -> bool {
    core::start_unset(audio_start_ts)
}

/// `size_t obs_output_raw_audio_block_bytes(size_t audio_size)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_block_bytes(audio_size: usize) -> usize {
    core::block_bytes(audio_size)
}

/// `size_t obs_output_raw_audio_push_bytes(uint32_t frames, size_t
/// audio_size)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_push_bytes(frames: u32, audio_size: usize) -> usize {
    core::push_bytes(frames, audio_size)
}

/// `bool obs_output_raw_audio_can_pop(size_t buffered, size_t
/// block_bytes)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_can_pop(buffered: usize, block_bytes: usize) -> bool {
    core::can_pop(buffered, block_bytes)
}

/// `uint64_t obs_output_raw_audio_block_ts(uint64_t audio_start_ts,
/// uint64_t frames_ns)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_block_ts(audio_start_ts: u64, frames_ns: u64) -> u64 {
    core::block_ts(audio_start_ts, frames_ns)
}

/// `uint64_t obs_output_raw_audio_paused_ts(uint64_t timestamp, uint64_t
/// ts_offset)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_paused_ts(timestamp: u64, ts_offset: u64) -> u64 {
    core::paused_ts(timestamp, ts_offset)
}

/// `size_t obs_output_raw_audio_next_frames(size_t total_audio_frames)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_next_frames(total_audio_frames: usize) -> usize {
    core::next_frames(total_audio_frames)
}

/// `bool obs_output_raw_audio_mix_on(size_t mixer_mask, size_t idx)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_mix_on(mixer_mask: usize, idx: usize) -> bool {
    core::mix_on(mixer_mask, idx)
}

/// `bool obs_output_raw_audio_interleave(bool has_video, bool has_audio)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_interleave(has_video: bool, has_audio: bool) -> bool {
    core::interleave(has_video, has_audio)
}

/// `bool obs_output_raw_audio_delay_on(uint32_t delay_sec)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_delay_on(delay_sec: u32) -> bool {
    core::delay_on(delay_sec)
}

/// `uint64_t obs_output_raw_audio_delay_ns(uint32_t delay_sec)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_delay_ns(delay_sec: u32) -> u64 {
    core::delay_ns(delay_sec)
}
