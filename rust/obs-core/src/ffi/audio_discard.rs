//! C ABI for `libobs/obs-audio-discard.c.h`.
//!
//! `obs-audio.c` calls the three functions inside libobs, holding the
//! source's `audio_buf_mutex`. They are declared in `obs-audio-discard.h`
//! without `EXPORT`, so the shims stay hidden
//! (`libobs/cmake/rust-hidden.txt`). The fields of `struct obs_source` come
//! by pointer in `struct obs_audio_input`; the input buffers are the C
//! `struct deque`s, popped in place through `obs_util::ffi::deque`, so the
//! source frees them with `deque_free` as before.
//!
//! `timing_set` is `volatile` in `struct obs_source`, and other threads
//! read and set it without the `audio_buf_mutex`. The shim never reads it
//! and writes it (volatile) only when the source's audio restarts, as the C
//! does; the other fields are written back after every call.
//!
//! Intentional differences, where C dereferences a null pointer, reads out
//! of bounds or corrupts a deque, and libobs never gets there: a null `in`,
//! `ts` or field pointer does nothing and returns the default step (no
//! rerender, the `DONE` path, no stop check); `channels` above
//! `MAX_AUDIO_CHANNELS` is cut to it; a channel that holds less than the
//! bytes to pop (channel 0's count, popped from every channel) is emptied,
//! where the C's `deque_pop_front` underflows the deque's size. libobs
//! gives every input channel of a source the same bytes. The safe core
//! lists the rest ([`crate::audio_discard`]).

use core::ptr;

use obs_util::ffi::deque::{deque, deque_pop_front};

use crate::audio_discard::{
    DiscardPath, DiscardStep, IgnoreStep, InputState, Pop, StopCheck, discard_tick, if_stopped,
    ignore,
};
use crate::audio_mix::{MAX_AUDIO_CHANNELS, TsInfo};
use crate::ffi::audio_mix::ts_info;

/// `struct obs_audio_input` (`obs-audio-discard.h`): the fields of
/// `struct obs_source` the steps read and change, by pointer.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct obs_audio_input {
    pub buf: *mut deque,
    pub audio_ts: *mut u64,
    pub last_size: *mut usize,
    pub pending: *mut bool,
    pub pending_stop: *mut bool,
    pub timing_set: *mut bool,
}

/// `struct obs_audio_ignore_step` (`obs-audio-discard.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_audio_ignore_step {
    pub rerender: bool,
    pub no_timestamp: bool,
    pub buffered: usize,
    pub dropped: usize,
    pub lag: u64,
    pub warn: bool,
    pub warn_lag: u64,
}

impl From<IgnoreStep> for obs_audio_ignore_step {
    fn from(step: IgnoreStep) -> Self {
        Self {
            rerender: step.rerender,
            no_timestamp: step.no_timestamp,
            buffered: step.buffered,
            dropped: step.dropped,
            lag: step.lag,
            warn: step.warn,
            warn_lag: step.warn_lag,
        }
    }
}

/// `enum obs_audio_stop_check` (`obs-audio-discard.h`).
pub const OBS_AUDIO_STOP_NONE: i32 = 0;
pub const OBS_AUDIO_STOP_PENDING: i32 = 1;
pub const OBS_AUDIO_STOP_CLEARED: i32 = 2;

/// `enum obs_audio_discard_path` (`obs-audio-discard.h`).
pub const OBS_AUDIO_DISCARD_RENDER_SOURCE: i32 = 0;
pub const OBS_AUDIO_DISCARD_AHEAD: i32 = 1;
pub const OBS_AUDIO_DISCARD_BEHIND: i32 = 2;
pub const OBS_AUDIO_DISCARD_START_AT_END: i32 = 3;
pub const OBS_AUDIO_DISCARD_PENDING: i32 = 4;
pub const OBS_AUDIO_DISCARD_DONE: i32 = 5;

fn stop_check(check: StopCheck) -> i32 {
    match check {
        StopCheck::None => OBS_AUDIO_STOP_NONE,
        StopCheck::Pending => OBS_AUDIO_STOP_PENDING,
        StopCheck::Cleared => OBS_AUDIO_STOP_CLEARED,
    }
}

fn discard_path(path: DiscardPath) -> i32 {
    match path {
        DiscardPath::RenderSource => OBS_AUDIO_DISCARD_RENDER_SOURCE,
        DiscardPath::Ahead => OBS_AUDIO_DISCARD_AHEAD,
        DiscardPath::Behind => OBS_AUDIO_DISCARD_BEHIND,
        DiscardPath::StartAtEnd => OBS_AUDIO_DISCARD_START_AT_END,
        DiscardPath::Pending => OBS_AUDIO_DISCARD_PENDING,
        DiscardPath::Done => OBS_AUDIO_DISCARD_DONE,
    }
}

/// `struct obs_audio_discard_step` (`obs-audio-discard.h`). The C enums
/// are `int`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_audio_discard_step {
    pub path: i32,
    pub stop: i32,
}

impl Default for obs_audio_discard_step {
    fn default() -> Self {
        Self {
            path: OBS_AUDIO_DISCARD_DONE,
            stop: OBS_AUDIO_STOP_NONE,
        }
    }
}

impl From<DiscardStep> for obs_audio_discard_step {
    fn from(step: DiscardStep) -> Self {
        Self {
            path: discard_path(step.path),
            stop: stop_check(step.stop),
        }
    }
}

/// The source's input behind `input`: the field pointers and their values,
/// and channel 0's size, or `None` when a pointer is null.
///
/// # Safety
///
/// `input` is null or a valid `struct obs_audio_input` whose pointers are
/// null or valid; `buf` points to at least one deque.
unsafe fn read(input: *const obs_audio_input) -> Option<(obs_audio_input, InputState, usize)> {
    if input.is_null() {
        return None;
    }
    // SAFETY: a valid struct (the contract).
    let fields = unsafe { *input };
    if fields.buf.is_null()
        || fields.audio_ts.is_null()
        || fields.last_size.is_null()
        || fields.pending.is_null()
        || fields.pending_stop.is_null()
        || fields.timing_set.is_null()
    {
        return None;
    }
    // SAFETY: the field pointers are valid (checked non-null, the
    // contract), and `buf` holds channel 0.
    let (state, size) = unsafe {
        (
            InputState {
                audio_ts: *fields.audio_ts,
                last_size: *fields.last_size,
                pending: *fields.pending,
                pending_stop: *fields.pending_stop,
            },
            (*fields.buf).size,
        )
    };
    Some((fields, state, size))
}

/// Writes the state back to the fields, and pops `pop` from the first
/// `channels` deques.
///
/// # Safety
///
/// The field pointers of `fields` are valid (as [`read`] returned them),
/// and `buf` points to `channels` valid deques with bmem memory (cut to
/// `MAX_AUDIO_CHANNELS`), not accessed elsewhere during the call.
unsafe fn write(fields: &obs_audio_input, state: &InputState, channels: usize, pop: Pop) {
    // SAFETY: valid field pointers (the contract).
    unsafe {
        *fields.audio_ts = state.audio_ts;
        *fields.last_size = state.last_size;
        *fields.pending = state.pending;
        *fields.pending_stop = state.pending_stop;
    }
    for ch in 0..channels.min(MAX_AUDIO_CHANNELS) {
        // SAFETY: `channels` deques behind `buf` (the contract).
        let dq = unsafe { fields.buf.add(ch) };
        let size = match pop {
            Pop::None => return,
            Pop::Bytes(n) => n,
            // The C reads channel 0's size again for each channel: after
            // channel 0, it is 0.
            Pop::Channel0(n) if ch == 0 => n,
            Pop::Channel0(_) => 0,
            // SAFETY: a valid deque (the contract).
            Pop::EachAll => unsafe { (*dq).size },
        };
        // SAFETY: a valid deque (the contract); a null data drops the bytes,
        // cut at what the deque holds.
        unsafe { deque_pop_front(dq, ptr::null_mut(), size.min((*dq).size)) };
    }
}

/// `obs_audio_discard_ignore()`.
///
/// # Safety
///
/// `input` is a valid `struct obs_audio_input`: valid field pointers, and
/// `buf` points to `channels` valid deques with bmem memory (the source's
/// `audio_input_buf`), not accessed elsewhere during the call (the caller
/// holds the source's `audio_buf_mutex`). A null `input` or field pointer
/// does nothing.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_audio_discard_ignore(
    input: *const obs_audio_input,
    channels: usize,
    sample_rate: usize,
    start_ts: u64,
) -> obs_audio_ignore_step {
    // SAFETY: the caller's contract.
    let Some((fields, mut state, size)) = (unsafe { read(input) }) else {
        return obs_audio_ignore_step::default();
    };
    let (step, pop) = ignore(&mut state, size, sample_rate, start_ts);
    // SAFETY: the pointers read checked, and the caller's contract.
    unsafe {
        write(&fields, &state, channels, pop);
        if step.restarted {
            ptr::write_volatile(fields.timing_set, false);
        }
    }
    step.into()
}

/// `obs_audio_discard_if_stopped()`; returns an `enum obs_audio_stop_check`.
///
/// # Safety
///
/// As [`obs_audio_discard_ignore`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_audio_discard_if_stopped(
    input: *const obs_audio_input,
    channels: usize,
) -> i32 {
    // SAFETY: the caller's contract.
    let Some((fields, mut state, size)) = (unsafe { read(input) }) else {
        return OBS_AUDIO_STOP_NONE;
    };
    let (check, pop) = if_stopped(&mut state, size);
    // SAFETY: the pointers read checked, and the caller's contract.
    unsafe { write(&fields, &state, channels, pop) };
    stop_check(check)
}

/// `obs_audio_discard_tick()`.
///
/// # Safety
///
/// As [`obs_audio_discard_ignore`], and `ts` is null or a valid
/// `struct ts_info`. A null `ts` does nothing.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_audio_discard_tick(
    input: *const obs_audio_input,
    audio_render: bool,
    channels: usize,
    sample_rate: usize,
    ts: *const ts_info,
) -> obs_audio_discard_step {
    if ts.is_null() {
        return obs_audio_discard_step::default();
    }
    // SAFETY: the caller's contract.
    let Some((fields, mut state, size)) = (unsafe { read(input) }) else {
        return obs_audio_discard_step::default();
    };
    // SAFETY: a valid struct (the contract).
    let ts = TsInfo::from(unsafe { *ts });
    let (step, pop) = discard_tick(&mut state, audio_render, size, sample_rate, ts);
    // SAFETY: the pointers read checked, and the caller's contract.
    unsafe { write(&fields, &state, channels, pop) };
    step.into()
}
