//! Safe core of `libobs/obs-source-audio-place.c.h`: the source audio
//! placement and action timeline of `obs-source.c`.
//!
//! - [`started`] and [`older`]: the reset of `source_output_audio_place()`
//!   (no `audio_ts`, or an older packet), the push-back rule of
//!   `source_output_audio_data()` and the action clamp of
//!   `apply_audio_actions()`;
//! - [`size`], [`offset`], [`fits`] and [`pop_back`]: the packet's bytes,
//!   its byte placement (`get_buf_placement()`), the drop past
//!   [`MAX_BUF_SIZE`] and how much is popped off the back of each channel;
//! - [`outputs`]: nothing for a monitor-only source;
//! - [`action_frame`], [`action_stops`], [`fill_end`] and [`mix_on`]: each
//!   action's frame, the stop at [`FRAMES`], how far the previous volume
//!   fills and the mixes multiplied.
//!
//! Each decision is its own function, asked at its own point, so
//! `obs-source.c` keeps every read where it was: `audio_ts` (again after
//! `reset_audio_data()`, and only where the C's `&&` and `||` reach it),
//! `monitoring_type`, the channels' sizes and `audio_mixers` once per mix.
//! It keeps `audio_buf_mutex` and `audio_actions_mutex`, the sample rate
//! and channel count, `reset_audio_data()`, the deques, `da_erase()`,
//! `apply_audio_action()`, every `get_source_volume()` call (before the
//! loop and after each applied action), the volume array and
//! `multiply_vol_data()`. Nothing here keeps state, takes a lock or calls
//! libobs.

/// `AUDIO_OUTPUT_FRAMES` (`media-io/audio-io.h`).
pub const FRAMES: usize = 1024;
/// `MAX_BUF_SIZE` (`obs-source.c`): `1000 * AUDIO_OUTPUT_FRAMES *
/// sizeof(float)` bytes per channel.
pub const MAX_BUF_SIZE: usize = 1000 * FRAMES * size_of::<f32>();
/// `OBS_MONITORING_TYPE_MONITOR_ONLY` (`enum obs_monitoring_type`, `obs.h`).
pub const MONITOR_ONLY: i32 = 1;

/// Whether the source has an audio timestamp: `audio_ts != 0`.
#[must_use]
pub fn started(audio_ts: u64) -> bool {
    let _ = audio_ts;
    todo!("audio_place::started")
}

/// Whether `timestamp` is older than `audio_ts`.
#[must_use]
pub fn older(timestamp: u64, audio_ts: u64) -> bool {
    let _ = (timestamp, audio_ts);
    todo!("audio_place::older")
}

/// A packet's bytes per channel: `frames * sizeof(float)` in `size_t`.
#[must_use]
pub fn size(frames: u32) -> usize {
    let _ = frames;
    todo!("audio_place::size")
}

/// `get_buf_placement()` in bytes: `(size_t)util_mul_div64(timestamp -
/// audio_ts, sample_rate, 1e9) * sizeof(float)`.
#[must_use]
pub fn offset(timestamp: u64, audio_ts: u64, sample_rate: u32) -> usize {
    let _ = (timestamp, audio_ts, sample_rate);
    todo!("audio_place::offset")
}

/// Whether `size` bytes from `start` stay within [`MAX_BUF_SIZE`].
#[must_use]
pub fn fits(start: usize, size: usize) -> bool {
    let _ = (start, size);
    todo!("audio_place::fits")
}

/// The bytes popped off the back of a channel after the place:
/// `buf_size - (placement + size)`.
#[must_use]
pub fn pop_back(buf_size: usize, placement: usize, size: usize) -> usize {
    let _ = (buf_size, placement, size);
    todo!("audio_place::pop_back")
}

/// Whether the packet goes to the buffers: not [`MONITOR_ONLY`].
#[must_use]
pub fn outputs(monitoring_type: i32) -> bool {
    let _ = monitoring_type;
    todo!("audio_place::outputs")
}

/// An action's frame: `conv_time_to_frames(sample_rate, timestamp -
/// audio_ts)`.
#[must_use]
pub fn action_frame(timestamp: u64, audio_ts: u64, sample_rate: usize) -> usize {
    let _ = (timestamp, audio_ts, sample_rate);
    todo!("audio_place::action_frame")
}

/// Whether the action loop stops at this frame: `frame >= FRAMES`.
#[must_use]
pub fn action_stops(frame: usize) -> bool {
    let _ = frame;
    todo!("audio_place::action_stops")
}

/// How far the previous volume fills before an action at `new_frame_num`.
#[must_use]
pub fn fill_end(frame_num: usize, new_frame_num: usize) -> usize {
    let _ = (frame_num, new_frame_num);
    todo!("audio_place::fill_end")
}

/// Whether the volume array multiplies mix `mix`: bit `mix` of
/// `audio_mixers`.
#[must_use]
pub fn mix_on(audio_mixers: u32, mix: usize) -> bool {
    let _ = (audio_mixers, mix);
    todo!("audio_place::mix_on")
}
