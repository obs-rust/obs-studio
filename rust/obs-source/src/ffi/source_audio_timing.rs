//! C ABI for `libobs/obs-source-audio-timing.c.h`, the source audio
//! timestamp placement, declared in `libobs/obs-source-audio-timing.h`
//! without `EXPORT`.
//!
//! `source_output_audio_data()` in `obs-source.c.h` (still C) fills a
//! struct with the packet and the source's fields, calls
//! [`obs_source_audio_timing_pre_lock`], stores what it returned, takes
//! `audio_buf_mutex`, does the same with
//! [`obs_source_audio_timing_under_lock`], and appends or places the packet.
//! `reset_audio_data_at_sys_time()` calls
//! [`obs_source_audio_timing_buffer_start`]. So `struct obs_source` is never
//! mirrored, and these shims take no lock, touch no source field and call
//! nothing in libobs. All three stay hidden (`rust-hidden.txt`).
//!
//! Each shim reads the struct's input fields and writes its output fields
//! through the pointer, one field at a time, and nothing else: the input
//! fields and the padding keep their bytes, as with the C, which writes the
//! same output fields. No reference to the struct is made, so the shims
//! never read an output field.
//!
//! Intentional difference from C: a null pointer does nothing (the C
//! dereferences it). libobs always passes a struct on its stack.

use crate::audio_timing::{self as core, Action, PreLockInput, UnderLockInput};

/// `enum obs_source_audio_ts_action`, as the C's enum is stored (an `int`
/// or `unsigned int`; every value fits both).
#[allow(non_camel_case_types)]
pub type obs_source_audio_ts_action = u32;
/// `OBS_SOURCE_AUDIO_TS_KEEP`.
pub const OBS_SOURCE_AUDIO_TS_KEEP: obs_source_audio_ts_action = 0;
/// `OBS_SOURCE_AUDIO_TS_RESET`.
pub const OBS_SOURCE_AUDIO_TS_RESET: obs_source_audio_ts_action = 1;
/// `OBS_SOURCE_AUDIO_TS_JUMP`.
pub const OBS_SOURCE_AUDIO_TS_JUMP: obs_source_audio_ts_action = 2;
/// `OBS_SOURCE_AUDIO_TS_SMOOTH`.
pub const OBS_SOURCE_AUDIO_TS_SMOOTH: obs_source_audio_ts_action = 3;
/// `OBS_SOURCE_AUDIO_TS_EXCEEDED`.
pub const OBS_SOURCE_AUDIO_TS_EXCEEDED: obs_source_audio_ts_action = 4;

const fn action_value(action: Action) -> obs_source_audio_ts_action {
    match action {
        Action::Keep => OBS_SOURCE_AUDIO_TS_KEEP,
        Action::Reset => OBS_SOURCE_AUDIO_TS_RESET,
        Action::Jump => OBS_SOURCE_AUDIO_TS_JUMP,
        Action::Smooth => OBS_SOURCE_AUDIO_TS_SMOOTH,
        Action::Exceeded => OBS_SOURCE_AUDIO_TS_EXCEEDED,
    }
}

/// `struct obs_source_audio_timing_pre` (`obs-source-audio-timing.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_source_audio_timing_pre {
    pub timestamp: u64,
    pub os_time: u64,
    pub duration: u64,
    pub next_audio_ts_min: u64,
    pub timing_adjust: u64,
    pub diff: u64,
    pub drift_error: i64,
    pub action: obs_source_audio_ts_action,
    pub timing_set: bool,
    pub async_unbuffered: bool,
    pub async_decoupled: bool,
    pub direct: bool,
    pub reanchor: bool,
    pub continuous: bool,
}

/// `struct obs_source_audio_timing_under` (`obs-source-audio-timing.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_source_audio_timing_under {
    pub timestamp: u64,
    pub packet_timing_adjust: u64,
    pub data_timestamp: u64,
    pub os_time: u64,
    pub next_audio_ts_min: u64,
    pub resample_offset: u64,
    pub sync_offset: i64,
    pub timing_adjust: u64,
    pub next_audio_sys_ts_min: u64,
    pub last_sync_offset: i64,
    pub continuous: bool,
    pub push_back: bool,
    pub reset_timing: bool,
}

/// `void obs_source_audio_timing_pre_lock(struct
/// obs_source_audio_timing_pre *t)`.
///
/// # Safety
///
/// `t` is null or points to a struct whose input fields (`timestamp`,
/// `os_time`, `duration`, `next_audio_ts_min`, `timing_set`,
/// `async_unbuffered`, `async_decoupled`) are initialized, the flags 0 or
/// 1, and that nothing else accesses during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_audio_timing_pre_lock(t: *mut obs_source_audio_timing_pre) {
    if t.is_null() {
        return;
    }
    // SAFETY: t is valid and its input fields initialized (caller
    // contract); place reads through the pointer make no reference.
    let input = unsafe {
        PreLockInput {
            timestamp: (*t).timestamp,
            os_time: (*t).os_time,
            duration: (*t).duration,
            next_audio_ts_min: (*t).next_audio_ts_min,
            timing_set: (*t).timing_set,
            async_unbuffered: (*t).async_unbuffered,
            async_decoupled: (*t).async_decoupled,
        }
    };
    let out = core::pre_lock(input);
    // SAFETY: as above; each write stores one field.
    unsafe {
        (*t).action = action_value(out.action);
        (*t).timing_adjust = out.timing_adjust;
        (*t).diff = out.diff;
        (*t).drift_error = out.drift_error;
        (*t).direct = out.direct;
        (*t).reanchor = out.reanchor;
        (*t).continuous = out.continuous;
        (*t).timestamp = out.timestamp;
        (*t).next_audio_ts_min = out.next_audio_ts_min;
    }
}

/// `void obs_source_audio_timing_under_lock(struct
/// obs_source_audio_timing_under *t)`.
///
/// # Safety
///
/// `t` is null or points to a struct whose input fields (all but
/// `push_back` and `reset_timing`) are initialized, `continuous` 0 or 1,
/// and that nothing else accesses during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_audio_timing_under_lock(t: *mut obs_source_audio_timing_under) {
    if t.is_null() {
        return;
    }
    // SAFETY: t is valid and its input fields initialized (caller
    // contract); place reads through the pointer make no reference.
    let input = unsafe {
        UnderLockInput {
            timestamp: (*t).timestamp,
            packet_timing_adjust: (*t).packet_timing_adjust,
            data_timestamp: (*t).data_timestamp,
            os_time: (*t).os_time,
            next_audio_ts_min: (*t).next_audio_ts_min,
            resample_offset: (*t).resample_offset,
            sync_offset: (*t).sync_offset,
            timing_adjust: (*t).timing_adjust,
            next_audio_sys_ts_min: (*t).next_audio_sys_ts_min,
            last_sync_offset: (*t).last_sync_offset,
            continuous: (*t).continuous,
        }
    };
    let out = core::under_lock(input);
    // SAFETY: as above; each write stores one field.
    unsafe {
        (*t).reset_timing = out.reset_timing;
        (*t).timing_adjust = out.timing_adjust;
        (*t).next_audio_sys_ts_min = out.next_audio_sys_ts_min;
        (*t).last_sync_offset = out.last_sync_offset;
        (*t).timestamp = out.timestamp;
        (*t).push_back = out.push_back;
    }
}

/// `uint64_t obs_source_audio_timing_buffer_start(uint64_t os_time,
/// int64_t sync_offset, uint64_t resample_offset)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_timing_buffer_start(
    os_time: u64,
    sync_offset: i64,
    resample_offset: u64,
) -> u64 {
    core::buffer_start(os_time, sync_offset, resample_offset)
}
