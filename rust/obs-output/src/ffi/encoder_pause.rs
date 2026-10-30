//! C ABI for `libobs/obs-encoder-pause.c.h`, the pause checks of
//! `obs-encoder.c`, declared in `libobs/obs-encoder-pause.h` without
//! `EXPORT`.
//!
//! These only decide. `obs-encoder.c.h` (still C) keeps every lock of
//! `pause->mutex`, the `last_video_ts` store, every read of the pause record
//! and the buffer, both `util_mul_div64()` conversions, every write to the
//! record and the buffer, the data pointer advance of each plane, the
//! `ts_offset` subtraction, `pause_reset()` and every caller, and passes
//! plain values here. So neither `struct pause_data` nor `struct audio_data`
//! is mirrored. These shims take no lock, call nothing in libobs and
//! dereference no pointer. All ten stay hidden (`rust-hidden.txt`).
//!
//! Intentional differences: the core's (a video pause ends on the first
//! frame at or after a set end; a paused buffer after a set end ends the
//! pause, `OBS_ENCODER_PAUSE_AUDIO_PAUSED_RESUME`).

use std::ffi::c_int;

use crate::encoder_pause::{self as core, AudioPaused, AudioStart};

/// `OBS_ENCODER_PAUSE_AUDIO_START_AFTER`.
pub const OBS_ENCODER_PAUSE_AUDIO_START_AFTER: c_int = 0;
/// `OBS_ENCODER_PAUSE_AUDIO_START_INSIDE`.
pub const OBS_ENCODER_PAUSE_AUDIO_START_INSIDE: c_int = 1;
/// `OBS_ENCODER_PAUSE_AUDIO_START_BEFORE`.
pub const OBS_ENCODER_PAUSE_AUDIO_START_BEFORE: c_int = 2;
/// `OBS_ENCODER_PAUSE_AUDIO_PAUSED_DROP`.
pub const OBS_ENCODER_PAUSE_AUDIO_PAUSED_DROP: c_int = 0;
/// `OBS_ENCODER_PAUSE_AUDIO_PAUSED_UNPAUSE`.
pub const OBS_ENCODER_PAUSE_AUDIO_PAUSED_UNPAUSE: c_int = 1;
/// `OBS_ENCODER_PAUSE_AUDIO_PAUSED_RESUME`.
pub const OBS_ENCODER_PAUSE_AUDIO_PAUSED_RESUME: c_int = 2;

/// `bool obs_encoder_pause_off(uint64_t ts_start)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_pause_off(ts_start: u64) -> bool {
    core::off(ts_start)
}

/// `bool obs_encoder_pause_video_end(uint64_t ts_end, uint64_t ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_pause_video_end(ts_end: u64, ts: u64) -> bool {
    core::video_end(ts_end, ts)
}

/// `bool obs_encoder_pause_video_drop(uint64_t ts_start, uint64_t ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_pause_video_drop(ts_start: u64, ts: u64) -> bool {
    core::video_drop(ts_start, ts)
}

/// `uint64_t obs_encoder_pause_audio_end(uint64_t timestamp, uint64_t
/// duration_ns)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_pause_audio_end(timestamp: u64, duration_ns: u64) -> u64 {
    core::audio_end(timestamp, duration_ns)
}

/// `int obs_encoder_pause_audio_start(uint64_t ts_start, uint64_t
/// timestamp, uint64_t end_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_pause_audio_start(
    ts_start: u64,
    timestamp: u64,
    end_ts: u64,
) -> c_int {
    match core::audio_start(ts_start, timestamp, end_ts) {
        AudioStart::After => OBS_ENCODER_PAUSE_AUDIO_START_AFTER,
        AudioStart::Inside => OBS_ENCODER_PAUSE_AUDIO_START_INSIDE,
        AudioStart::Before => OBS_ENCODER_PAUSE_AUDIO_START_BEFORE,
    }
}

/// `int obs_encoder_pause_audio_paused(uint64_t ts_end, uint64_t timestamp,
/// uint64_t end_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_pause_audio_paused(
    ts_end: u64,
    timestamp: u64,
    end_ts: u64,
) -> c_int {
    match core::audio_paused(ts_end, timestamp, end_ts) {
        AudioPaused::Drop => OBS_ENCODER_PAUSE_AUDIO_PAUSED_DROP,
        AudioPaused::Unpause => OBS_ENCODER_PAUSE_AUDIO_PAUSED_UNPAUSE,
        AudioPaused::Resume => OBS_ENCODER_PAUSE_AUDIO_PAUSED_RESUME,
    }
}

/// `uint64_t obs_encoder_pause_audio_cut_ns(uint64_t ts, uint64_t
/// timestamp)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_pause_audio_cut_ns(ts: u64, timestamp: u64) -> u64 {
    core::audio_cut_ns(ts, timestamp)
}

/// `uint32_t obs_encoder_pause_audio_kept(uint64_t cut_frames)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_pause_audio_kept(cut_frames: u64) -> u32 {
    core::audio_kept(cut_frames)
}

/// `uint32_t obs_encoder_pause_audio_left(uint32_t frames, uint64_t
/// cut_frames)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_pause_audio_left(frames: u32, cut_frames: u64) -> u32 {
    core::audio_left(frames, cut_frames)
}

/// `bool obs_encoder_pause_audio_drop(uint32_t frames)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_pause_audio_drop(frames: u32) -> bool {
    core::audio_drop(frames)
}
