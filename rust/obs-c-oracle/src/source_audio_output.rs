//! `oracle_*` builds of `libobs/obs-source-audio-output.c.h`
//! (oracle/source_audio_output.c): the source audio output path, renamed, with
//! the seam (`libobs/obs-source-audio-output-seam.c.h`) compiled under its own
//! names, which the Rust shims call too. A test source is a zeroed
//! `struct obs_source` with its mutex and a log of each call. Sources and
//! packets cross as opaque pointers.

use core::ffi::{c_char, c_void};

/// `struct oracle_audio_output_init`: the starting fields of a test source.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Init {
    /// `timing_adjust`.
    pub timing_adjust: u64,
    /// `next_audio_ts_min`.
    pub next_audio_ts_min: u64,
    /// `next_audio_sys_ts_min`.
    pub next_audio_sys_ts_min: u64,
    /// `audio_ts`.
    pub audio_ts: u64,
    /// `resample_offset`.
    pub resample_offset: u64,
    /// `sync_offset`.
    pub sync_offset: i64,
    /// `last_sync_offset`.
    pub last_sync_offset: i64,
    /// `monitoring_type`.
    pub monitoring_type: i32,
    /// `timing_set`.
    pub timing_set: bool,
    /// `async_unbuffered`.
    pub async_unbuffered: bool,
    /// `async_decoupled`.
    pub async_decoupled: bool,
    /// A resampler and `last_async_compensation`.
    pub compensating: bool,
    /// What `source_muted()` answers.
    pub muted: bool,
}

unsafe extern "C" {
    /// A test source. Free it with [`oracle_audio_output_free`].
    pub fn oracle_audio_output_new() -> *mut c_void;
    /// Frees a test source.
    pub fn oracle_audio_output_free(source: *mut c_void);
    /// Sets the fields. `last_audio_input_buf_size` becomes 7, so a store of 0
    /// shows.
    pub fn oracle_audio_output_set_fields(source: *mut c_void, init: *const Init);
    /// The source the stand-ins log to, and what they answer, on this thread.
    pub fn oracle_audio_output_begin(source: *mut c_void, now: u64, rate: u32, channels: usize);
    /// A stereo packet of float samples. Free it with
    /// [`oracle_audio_output_packet_free`].
    pub fn oracle_audio_output_packet_new(timestamp: u64, frames: u32, value: f32) -> *mut c_void;
    /// Frees a packet.
    pub fn oracle_audio_output_packet_free(data: *mut c_void);
    /// Takes `audio_buf_mutex`, for the reset functions.
    pub fn oracle_audio_output_lock(source: *mut c_void);
    /// Releases `audio_buf_mutex`.
    pub fn oracle_audio_output_unlock(source: *mut c_void);
    /// The log, every field, a hash of each buffer and whether the mutex is
    /// free, as text. Returns its length.
    pub fn oracle_audio_output_state(source: *const c_void, out: *mut c_char, cap: usize) -> usize;

    /// `reset_audio_timing`.
    pub fn oracle_reset_audio_timing(source: *mut c_void, timestamp: u64, os_time: u64);
    /// `reset_audio_data`.
    pub fn oracle_reset_audio_data(source: *mut c_void, os_time: u64);
    /// `reset_audio_data_at_sys_time`.
    pub fn oracle_reset_audio_data_at_sys_time(source: *mut c_void, os_time: u64);
    /// `source_output_audio_data`.
    pub fn oracle_source_output_audio_data(source: *mut c_void, data: *const c_void);
}
