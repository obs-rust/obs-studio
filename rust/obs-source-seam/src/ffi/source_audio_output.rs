//! C ABI for `libobs/obs-source-audio-output.c.h`, the path that puts one audio
//! packet of a source in its buffer. `obs-source.c.h` calls these inside
//! libobs: they are declared in `obs-internal.h` without `EXPORT` and stay
//! hidden (`rust-hidden.d`).
//!
//! The functions are the safe core's (`obs_source::audio_output`), run on
//! [`Seam`], which implements its `Source` over
//! `libobs/obs-source-audio-output-seam.h`, still C in `libobs.core-source`:
//! one seam call per method. `struct obs_source` is never mirrored, the fields
//! stay in C, so ThreadSanitizer keeps seeing their reads and writes, and the
//! packet's samples stay behind the `struct audio_data` pointer. The locking
//! is the C's.
//!
//! As in the C, none of these checks its source or its packet. Every caller
//! has checked them.

use std::ffi::{c_int, c_void};

use obs_source::audio_output::{self as core, Packet, Source};

unsafe extern "C" {
    fn obs_source_audio_output_seam_now() -> u64;
    fn obs_source_audio_output_seam_sample_rate() -> u32;
    fn obs_source_audio_output_seam_channels() -> usize;

    fn obs_source_audio_output_seam_data_timestamp(data: *const c_void) -> u64;
    fn obs_source_audio_output_seam_data_frames(data: *const c_void) -> u32;

    fn obs_source_audio_output_seam_lock_audio_buf(source: *const c_void);
    fn obs_source_audio_output_seam_unlock_audio_buf(source: *const c_void);

    fn obs_source_audio_output_seam_get_timing_set(source: *const c_void) -> bool;
    fn obs_source_audio_output_seam_set_timing_set(source: *mut c_void, value: bool);
    fn obs_source_audio_output_seam_get_timing_adjust(source: *const c_void) -> u64;
    fn obs_source_audio_output_seam_set_timing_adjust(source: *mut c_void, value: u64);
    fn obs_source_audio_output_seam_get_next_audio_ts_min(source: *const c_void) -> u64;
    fn obs_source_audio_output_seam_set_next_audio_ts_min(source: *mut c_void, value: u64);
    fn obs_source_audio_output_seam_get_next_audio_sys_ts_min(source: *const c_void) -> u64;
    fn obs_source_audio_output_seam_set_next_audio_sys_ts_min(source: *mut c_void, value: u64);
    fn obs_source_audio_output_seam_get_audio_ts(source: *const c_void) -> u64;
    fn obs_source_audio_output_seam_set_audio_ts(source: *mut c_void, value: u64);
    fn obs_source_audio_output_seam_clear_last_buf_size(source: *mut c_void);
    fn obs_source_audio_output_seam_get_sync_offset(source: *const c_void) -> i64;
    fn obs_source_audio_output_seam_get_last_sync_offset(source: *const c_void) -> i64;
    fn obs_source_audio_output_seam_set_last_sync_offset(source: *mut c_void, value: i64);
    fn obs_source_audio_output_seam_get_resample_offset(source: *const c_void) -> u64;
    fn obs_source_audio_output_seam_get_monitoring_type(source: *const c_void) -> c_int;
    fn obs_source_audio_output_seam_get_async_unbuffered(source: *const c_void) -> bool;
    fn obs_source_audio_output_seam_get_async_decoupled(source: *const c_void) -> bool;
    fn obs_source_audio_output_seam_get_last_output_frame_ts(source: *const c_void) -> u64;

    fn obs_source_audio_output_seam_compensating(source: *const c_void) -> bool;
    fn obs_source_audio_output_seam_set_compensation_error(source: *mut c_void, error: i64);

    fn obs_source_audio_output_seam_buf_size(source: *const c_void, channel: usize) -> usize;
    fn obs_source_audio_output_seam_pop_front(source: *mut c_void, channel: usize, size: usize);
    fn obs_source_audio_output_seam_pop_back(source: *mut c_void, channel: usize, size: usize);
    fn obs_source_audio_output_seam_place(
        source: *mut c_void,
        channel: usize,
        placement: usize,
        data: *const c_void,
        size: usize,
    );
    fn obs_source_audio_output_seam_push_back(
        source: *mut c_void,
        channel: usize,
        data: *const c_void,
        size: usize,
    );

    fn obs_source_audio_output_seam_log_ts_jump(
        source: *const c_void,
        diff: u64,
        expected: u64,
        timestamp: u64,
    );
    fn obs_source_audio_output_seam_log_ts_exceeded(
        source: *const c_void,
        diff: u64,
        expected: u64,
        timestamp: u64,
    );

    fn obs_source_audio_output_seam_muted(source: *mut c_void, os_time: u64) -> bool;
    fn obs_source_audio_output_seam_signal_audio_data(
        source: *mut c_void,
        data: *const c_void,
        muted: bool,
    );
}

/// The live, non-NULL `obs_source_t` a shim was handed.
struct Seam(*mut c_void);

// SAFETY (every method): `self.0` is the live source the C caller passed, each
// call is the one seam function that names it, and the core holds the locks
// the seam header asks for.
impl Source for Seam {
    type Data = *const c_void;

    fn now(&self) -> u64 {
        // SAFETY: no arguments.
        unsafe { obs_source_audio_output_seam_now() }
    }
    fn sample_rate(&self) -> u32 {
        // SAFETY: no arguments; libobs has its audio while a source runs.
        unsafe { obs_source_audio_output_seam_sample_rate() }
    }
    fn channels(&self) -> usize {
        // SAFETY: no arguments; libobs has its audio while a source runs.
        unsafe { obs_source_audio_output_seam_channels() }
    }
    fn lock_audio_buf(&self) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_lock_audio_buf(self.0) }
    }
    fn unlock_audio_buf(&self) {
        // SAFETY: see the impl; the core locked it.
        unsafe { obs_source_audio_output_seam_unlock_audio_buf(self.0) }
    }
    fn timing_set(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_get_timing_set(self.0) }
    }
    fn set_timing_set(&self, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_set_timing_set(self.0, value) }
    }
    fn timing_adjust(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_get_timing_adjust(self.0) }
    }
    fn set_timing_adjust(&self, value: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_set_timing_adjust(self.0, value) }
    }
    fn next_audio_ts_min(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_get_next_audio_ts_min(self.0) }
    }
    fn set_next_audio_ts_min(&self, value: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_set_next_audio_ts_min(self.0, value) }
    }
    fn next_audio_sys_ts_min(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_get_next_audio_sys_ts_min(self.0) }
    }
    fn set_next_audio_sys_ts_min(&self, value: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_set_next_audio_sys_ts_min(self.0, value) }
    }
    fn audio_ts(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_get_audio_ts(self.0) }
    }
    fn set_audio_ts(&self, value: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_set_audio_ts(self.0, value) }
    }
    fn clear_last_buf_size(&self) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_clear_last_buf_size(self.0) }
    }
    fn sync_offset(&self) -> i64 {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_get_sync_offset(self.0) }
    }
    fn last_sync_offset(&self) -> i64 {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_get_last_sync_offset(self.0) }
    }
    fn set_last_sync_offset(&self, value: i64) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_set_last_sync_offset(self.0, value) }
    }
    fn resample_offset(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_get_resample_offset(self.0) }
    }
    fn monitoring_type(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_get_monitoring_type(self.0) }
    }
    fn async_unbuffered(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_get_async_unbuffered(self.0) }
    }
    fn async_decoupled(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_get_async_decoupled(self.0) }
    }
    fn last_output_frame_ts(&self) -> u64 {
        // SAFETY: a live source; the caller holds its audio_buf_mutex.
        unsafe { obs_source_audio_output_seam_get_last_output_frame_ts(self.0) }
    }
    fn compensating(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_compensating(self.0) }
    }
    fn set_compensation_error(&self, error: i64) {
        // SAFETY: see the impl; `compensating()` answered true.
        unsafe { obs_source_audio_output_seam_set_compensation_error(self.0, error) }
    }
    fn buf_size(&self, channel: usize) -> usize {
        // SAFETY: see the impl; the core asks below MAX_AUDIO_CHANNELS.
        unsafe { obs_source_audio_output_seam_buf_size(self.0, channel) }
    }
    fn pop_front(&self, channel: usize, size: usize) {
        // SAFETY: see the impl; `size` is the buffer's size.
        unsafe { obs_source_audio_output_seam_pop_front(self.0, channel, size) }
    }
    fn pop_back(&self, channel: usize, size: usize) {
        // SAFETY: see the impl; `size` is what the placed packet left over.
        unsafe { obs_source_audio_output_seam_pop_back(self.0, channel, size) }
    }
    fn place(&self, channel: usize, placement: usize, data: *const c_void, size: usize) {
        // SAFETY: see the impl; `data` is the caller's live packet, with
        // `size` bytes per output channel.
        unsafe { obs_source_audio_output_seam_place(self.0, channel, placement, data, size) }
    }
    fn push_back(&self, channel: usize, data: *const c_void, size: usize) {
        // SAFETY: as for `place`.
        unsafe { obs_source_audio_output_seam_push_back(self.0, channel, data, size) }
    }
    fn log_ts_jump(&self, diff: u64, expected: u64, timestamp: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_log_ts_jump(self.0, diff, expected, timestamp) }
    }
    fn log_ts_exceeded(&self, diff: u64, expected: u64, timestamp: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_log_ts_exceeded(self.0, diff, expected, timestamp) }
    }
    fn muted(&self, os_time: u64) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_audio_output_seam_muted(self.0, os_time) }
    }
    fn signal_audio_data(&self, data: *const c_void, muted: bool) {
        // SAFETY: see the impl; `data` is the caller's live packet.
        unsafe { obs_source_audio_output_seam_signal_audio_data(self.0, data, muted) }
    }
}

/// `void reset_audio_timing(obs_source_t *source, uint64_t timestamp, uint64_t
/// os_time)`.
///
/// # Safety
/// `source` is a live source, and the caller holds its `audio_buf_mutex`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reset_audio_timing(source: *mut c_void, timestamp: u64, os_time: u64) {
    core::reset_audio_timing(&Seam(source), timestamp, os_time);
}

/// `void reset_audio_data(obs_source_t *source, uint64_t os_time)`.
///
/// # Safety
/// `source` is a live source, and the caller holds its `audio_buf_mutex`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reset_audio_data(source: *mut c_void, os_time: u64) {
    core::reset_audio_data(&Seam(source), os_time);
}

/// `void reset_audio_data_at_sys_time(obs_source_t *source, uint64_t os_time)`.
///
/// # Safety
/// `source` is a live source, and the caller holds its `audio_buf_mutex`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reset_audio_data_at_sys_time(source: *mut c_void, os_time: u64) {
    core::reset_audio_data_at_sys_time(&Seam(source), os_time);
}

/// `void source_output_audio_data(obs_source_t *source, const struct audio_data
/// *data)`.
///
/// # Safety
/// `source` is a live source, the caller holds its `audio_mutex`, and `data` is
/// a live packet with the output's channels.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn source_output_audio_data(source: *mut c_void, data: *const c_void) {
    // SAFETY: `data` is a live packet, per the contract above.
    let (timestamp, frames) = unsafe {
        (
            obs_source_audio_output_seam_data_timestamp(data),
            obs_source_audio_output_seam_data_frames(data),
        )
    };
    core::output_audio_data(
        &Seam(source),
        Packet {
            data,
            timestamp,
            frames,
        },
    );
}
