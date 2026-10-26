//! Safe core of `libobs/obs-output-raw-audio.c.h`: the raw output audio
//! admission and the capture hookup of `obs-output.c`.
//!
//! One function per decision, each asked where the C makes it.
//! `obs-output.c` keeps `data_active()`, every read of the output, the pause
//! mutex and its reads (`last_video_ts` only while `video_start_ts` is 0, and
//! `ts_offset`), `audio_pause_check()`, the deques and the pointer advance of
//! each plane, the `raw_audio` and `raw_audio2` callbacks and the NULL test
//! of `raw_audio2`, `get_first_mixer()`, `audio_output_connect()`,
//! `start_raw_video()`, `reset_packet_data()` under `interleaved_mutex`,
//! `obs_encoder_start()`, the delay's fields, flag and log, and every
//! conversion that divides by the sample rate (the packet's duration, the
//! cut in frames and `audio_frames_to_ns()`). It passes their results here.
//! These keep no state and call nothing in libobs.
//!
//! - `prepare_audio()`: [`passthrough`] (an output without video takes the
//!   packet as it is), [`video_unset`] (asked before the video start is
//!   taken from the pause data, and again after: still 0, the packet
//!   waits), [`before_video`], [`ends_before`] (the packet is dropped) and
//!   the front cut: [`cut_ns`], [`cut_ts`], [`cut_bytes`] (asked for each
//!   plane) and [`cut_frames`].
//! - `default_raw_audio_callback()`: [`start_unset`] (the first packet sets
//!   the audio start), [`block_bytes`], [`push_bytes`] (asked for each
//!   plane), [`can_pop`] (the block loop: strictly more than one block, so
//!   one full block always stays buffered), [`block_ts`], [`paused_ts`]
//!   (under the pause mutex) and [`next_frames`].
//! - `start_raw_audio()`: [`mix_on`], asked for each mix with `raw_audio2`.
//! - `hook_data_capture()`: [`interleave`] (the encoded callback),
//!   [`delay_on`] and [`delay_ns`].
//!
//! Every sum, difference and product wraps as the C's unsigned arithmetic
//! does. No input comes from outside OBS (timestamps from the audio and
//! video threads, sizes, rates, the mixer mask and the delay from libobs and
//! the frontend, the flags from an output plugin), and nothing here can
//! panic on any input. Nothing here divides.

pub use crate::video_checks::OBS_OUTPUT_VIDEO;

/// `AUDIO_OUTPUT_FRAMES` (`media-io/audio-io.h`): the frames of one block.
pub const AUDIO_OUTPUT_FRAMES: u32 = 1024;

/// `prepare_audio()`: the output has no `OBS_OUTPUT_VIDEO` flag, so the
/// packet passes as it is.
#[must_use]
pub fn passthrough(flags: u32) -> bool {
    let _ = flags;
    todo!("raw_audio::passthrough")
}

/// `prepare_audio()`: `video_start_ts` is 0.
#[must_use]
pub fn video_unset(video_start_ts: u64) -> bool {
    let _ = video_start_ts;
    todo!("raw_audio::video_unset")
}

/// `prepare_audio()`: the packet starts before the video.
#[must_use]
pub fn before_video(timestamp: u64, video_start_ts: u64) -> bool {
    let _ = (timestamp, video_start_ts);
    todo!("raw_audio::before_video")
}

/// `prepare_audio()`, for a packet that starts before the video: it ends
/// (`timestamp + duration_ns`, wrapping) at or before the video's start, so
/// it is dropped.
#[must_use]
pub fn ends_before(timestamp: u64, duration_ns: u64, video_start_ts: u64) -> bool {
    let _ = (timestamp, duration_ns, video_start_ts);
    todo!("raw_audio::ends_before")
}

/// `prepare_audio()`: the nanoseconds cut from the packet's front,
/// `video_start_ts - timestamp` (wrapping).
#[must_use]
pub fn cut_ns(video_start_ts: u64, timestamp: u64) -> u64 {
    let _ = (video_start_ts, timestamp);
    todo!("raw_audio::cut_ns")
}

/// `prepare_audio()`: the cut packet's timestamp, `timestamp + cut_ns`
/// (wrapping).
#[must_use]
pub fn cut_ts(timestamp: u64, cut_ns: u64) -> u64 {
    let _ = (timestamp, cut_ns);
    todo!("raw_audio::cut_ts")
}

/// `prepare_audio()`, for each plane: the bytes its data advances,
/// `audio_size` times the low 32 bits of `cut_frames` (wrapping).
#[must_use]
pub fn cut_bytes(audio_size: usize, cut_frames: u64) -> usize {
    let _ = (audio_size, cut_frames);
    todo!("raw_audio::cut_bytes")
}

/// `prepare_audio()`: the frames left, `frames` minus the low 32 bits of
/// `cut_frames` (wrapping).
#[must_use]
pub fn cut_frames(frames: u32, cut_frames: u64) -> u32 {
    let _ = (frames, cut_frames);
    todo!("raw_audio::cut_frames")
}

/// `default_raw_audio_callback()`: `audio_start_ts` is 0, so this packet's
/// timestamp becomes the audio's start.
#[must_use]
pub fn start_unset(audio_start_ts: u64) -> bool {
    let _ = audio_start_ts;
    todo!("raw_audio::start_unset")
}

/// `default_raw_audio_callback()`: the bytes of one block of one plane,
/// [`AUDIO_OUTPUT_FRAMES`] times `audio_size` (wrapping).
#[must_use]
pub fn block_bytes(audio_size: usize) -> usize {
    let _ = audio_size;
    todo!("raw_audio::block_bytes")
}

/// `default_raw_audio_callback()`, for each plane: the bytes pushed,
/// `frames` times `audio_size` (wrapping).
#[must_use]
pub fn push_bytes(frames: u32, audio_size: usize) -> usize {
    let _ = (frames, audio_size);
    todo!("raw_audio::push_bytes")
}

/// `default_raw_audio_callback()`: the block loop pops a block while plane
/// 0 holds strictly more than one.
#[must_use]
pub fn can_pop(buffered: usize, block_bytes: usize) -> bool {
    let _ = (buffered, block_bytes);
    todo!("raw_audio::can_pop")
}

/// `default_raw_audio_callback()`: a block's timestamp before the pause
/// offset, `audio_start_ts + frames_ns` (wrapping).
#[must_use]
pub fn block_ts(audio_start_ts: u64, frames_ns: u64) -> u64 {
    let _ = (audio_start_ts, frames_ns);
    todo!("raw_audio::block_ts")
}

/// `default_raw_audio_callback()`, under the pause mutex: `timestamp +
/// ts_offset` (wrapping).
#[must_use]
pub fn paused_ts(timestamp: u64, ts_offset: u64) -> u64 {
    let _ = (timestamp, ts_offset);
    todo!("raw_audio::paused_ts")
}

/// `default_raw_audio_callback()`: `total_audio_frames` after a block, plus
/// [`AUDIO_OUTPUT_FRAMES`] (wrapping).
#[must_use]
pub fn next_frames(total_audio_frames: usize) -> usize {
    let _ = total_audio_frames;
    todo!("raw_audio::next_frames")
}

/// `start_raw_audio()` with `raw_audio2`, for each mix below
/// `MAX_AUDIO_MIXES`: bit `idx` of `mixer_mask` is set. An `idx` past the
/// width of `usize` (the C's shift is undefined there; the C asks only
/// 0 to 5) is not set.
#[must_use]
pub fn mix_on(mixer_mask: usize, idx: usize) -> bool {
    let _ = (mixer_mask, idx);
    todo!("raw_audio::mix_on")
}

/// `hook_data_capture()`, for an encoded output: `interleave_packets()`
/// when it has both video and audio, else `default_encoded_callback()`.
#[must_use]
pub fn interleave(has_video: bool, has_audio: bool) -> bool {
    let _ = (has_video, has_audio);
    todo!("raw_audio::interleave")
}

/// `hook_data_capture()`: `delay_sec` is not 0, so `process_delay()` wraps
/// the callback.
#[must_use]
pub fn delay_on(delay_sec: u32) -> bool {
    let _ = delay_sec;
    todo!("raw_audio::delay_on")
}

/// `hook_data_capture()`: `active_delay_ns`, `delay_sec` seconds in
/// nanoseconds (a `u64` product; `u32::MAX` seconds fit).
#[must_use]
pub fn delay_ns(delay_sec: u32) -> u64 {
    let _ = delay_sec;
    todo!("raw_audio::delay_ns")
}
