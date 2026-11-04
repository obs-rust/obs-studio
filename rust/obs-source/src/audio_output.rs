//! Safe core of `libobs/obs-source-audio-output.c.h`: the path that puts one
//! audio packet of a source in its buffer, moved out of
//! `libobs/obs-source.c.h`.
//!
//! The decisions are [`audio_timing`](crate::audio_timing) and
//! [`audio_place`](crate::audio_place), on plain values. This module makes the
//! C's calls around them in its order and under its locks, generic over a
//! [`Source`]: one method per field read or write, lock, buffer operation and
//! call into the rest of libobs. The C shims live in the `obs-source-seam`
//! crate, which implements [`Source`] over
//! `libobs/obs-source-audio-output-seam.h`, so this crate calls nothing in
//! libobs. The fields stay in `struct obs_source`, and each read and write is
//! C, so ThreadSanitizer keeps seeing them. The packet's samples do not cross:
//! [`Packet::data`] is opaque here.
//!
//! What the C does and this keeps (characterized, not endorsed;
//! `test_source_audio_output` pins most of it):
//!
//! - [`output_audio_data`] has two `audio_buf_mutex` regions, with the jump's
//!   own region between them. The graphics thread can set a new
//!   `timing_adjust` in those gaps, and the second region reads the new value.
//! - `timing_adjust` is read twice in the second region, because it is
//!   volatile in the C: once for the packet, once as the value to keep.
//! - The first placed packet leaves `next_audio_sys_ts_min` at the packet's
//!   start, not at its end: [`reset_audio_data`] runs after the store.
//! - A packet that does not fit is dropped with no log, and
//!   `last_audio_input_buf_size` keeps its value then.
//! - The capture callback gets the packet's own timestamp, and runs with
//!   `audio_buf_mutex` released.
//!
//! The `DEBUG_AUDIO` log line of `source_output_audio_place()` is compiled out
//! on every C build and is not here.

#![allow(clippy::todo, unused_variables)]
#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::unwrap_used
)]

/// `MAX_AUDIO_CHANNELS`: the buffers [`reset_audio_data`] empties.
pub const MAX_CHANNELS: usize = 8;

/// One packet, as `struct audio_data` holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packet<D> {
    /// The samples, opaque here.
    pub data: D,
    /// `data->timestamp`.
    pub timestamp: u64,
    /// `data->frames`.
    pub frames: u32,
}

/// The source audio output seam: one method per field read or write, lock,
/// buffer operation and libobs call. A lock method is one lock call, and none
/// of the others takes a lock of its own.
///
/// The caller holds `audio_buf_mutex` around every field except
/// `next_audio_ts_min`, `async_unbuffered`, `async_decoupled` and the
/// resampler's, and around every buffer operation.
pub trait Source {
    /// The samples of a packet, opaque to the core.
    type Data: Copy;

    /// `os_gettime_ns()`.
    fn now(&self) -> u64;
    /// `audio_output_get_sample_rate(obs->audio.audio)`.
    fn sample_rate(&self) -> u32;
    /// `audio_output_get_channels(obs->audio.audio)`.
    fn channels(&self) -> usize;

    /// `pthread_mutex_lock(&source->audio_buf_mutex)`.
    fn lock_audio_buf(&self);
    /// `pthread_mutex_unlock(&source->audio_buf_mutex)`.
    fn unlock_audio_buf(&self);

    /// `source->timing_set`.
    fn timing_set(&self) -> bool;
    /// `source->timing_set = value`.
    fn set_timing_set(&self, value: bool);
    /// `source->timing_adjust`.
    fn timing_adjust(&self) -> u64;
    /// `source->timing_adjust = value`.
    fn set_timing_adjust(&self, value: u64);
    /// `source->next_audio_ts_min`.
    fn next_audio_ts_min(&self) -> u64;
    /// `source->next_audio_ts_min = value`.
    fn set_next_audio_ts_min(&self, value: u64);
    /// `source->next_audio_sys_ts_min`.
    fn next_audio_sys_ts_min(&self) -> u64;
    /// `source->next_audio_sys_ts_min = value`.
    fn set_next_audio_sys_ts_min(&self, value: u64);
    /// `source->audio_ts`.
    fn audio_ts(&self) -> u64;
    /// `source->audio_ts = value`.
    fn set_audio_ts(&self, value: u64);
    /// `source->last_audio_input_buf_size = 0`.
    fn clear_last_buf_size(&self);
    /// `source->sync_offset`.
    fn sync_offset(&self) -> i64;
    /// `source->last_sync_offset`.
    fn last_sync_offset(&self) -> i64;
    /// `source->last_sync_offset = value`.
    fn set_last_sync_offset(&self, value: i64);
    /// `source->resample_offset`.
    fn resample_offset(&self) -> u64;
    /// `source->monitoring_type`.
    fn monitoring_type(&self) -> i32;
    /// `source->async_unbuffered`.
    fn async_unbuffered(&self) -> bool;
    /// `source->async_decoupled`.
    fn async_decoupled(&self) -> bool;

    /// `source->resampler != NULL && source->last_async_compensation`.
    fn compensating(&self) -> bool;
    /// `audio_resampler_set_compensation_error(source->resampler, error)`.
    fn set_compensation_error(&self, error: i64);

    /// `source->audio_input_buf[channel].size`.
    fn buf_size(&self, channel: usize) -> usize;
    /// `deque_pop_front(&source->audio_input_buf[channel], NULL, size)`.
    fn pop_front(&self, channel: usize, size: usize);
    /// `deque_pop_back(&source->audio_input_buf[channel], NULL, size)`.
    fn pop_back(&self, channel: usize, size: usize);
    /// `deque_place(&source->audio_input_buf[channel], placement,
    /// data->data[channel], size)`.
    fn place(&self, channel: usize, placement: usize, data: Self::Data, size: usize);
    /// `deque_push_back(&source->audio_input_buf[channel],
    /// data->data[channel], size)`.
    fn push_back(&self, channel: usize, data: Self::Data, size: usize);

    /// The debug log line of `handle_ts_jump()`.
    fn log_ts_jump(&self, diff: u64, expected: u64, timestamp: u64);
    /// The debug log line of a timestamp past the smoothing threshold.
    fn log_ts_exceeded(&self, diff: u64, expected: u64, timestamp: u64);

    /// `source_muted(source, os_time)`.
    fn muted(&self, os_time: u64) -> bool;
    /// `source_signal_audio_data(source, data, muted)`.
    fn signal_audio_data(&self, data: Self::Data, muted: bool);
}

/// `conv_frames_to_time()`: 0 for no sample rate. `frames` is 32 bits, so the
/// product fits.
#[must_use]
pub fn duration(sample_rate: u32, frames: u32) -> u64 {
    todo!()
}

/// `reset_audio_timing()`. The caller holds `audio_buf_mutex`.
pub fn reset_audio_timing<S: Source>(source: &S, timestamp: u64, os_time: u64) {
    todo!()
}

/// `reset_audio_data()`. The caller holds `audio_buf_mutex`.
pub fn reset_audio_data<S: Source>(source: &S, os_time: u64) {
    todo!()
}

/// `reset_audio_data_at_sys_time()`: empties the buffer for packets that
/// continue at system time `os_time`. The caller holds `audio_buf_mutex`.
pub fn reset_audio_data_at_sys_time<S: Source>(source: &S, os_time: u64) {
    todo!()
}

/// `source_output_audio_data()`. The caller holds `audio_mutex`.
pub fn output_audio_data<S: Source>(source: &S, packet: Packet<S::Data>) {
    todo!()
}
