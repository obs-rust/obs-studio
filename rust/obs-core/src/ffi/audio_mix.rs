//! C ABI for `libobs/obs-audio-mix.c.h`.
//!
//! `obs-audio.c` calls the five functions inside libobs. They are declared
//! in `obs-audio-mix.h` without `EXPORT`, so the shims stay hidden
//! (`libobs/cmake/rust-hidden.txt`). The buffering fields of
//! `struct obs_core_audio` come by pointer in `struct obs_audio_buffering`,
//! and the queue is the C `struct deque` of `struct ts_info`, pushed in
//! place through `obs_util::ffi::deque` with bmem memory, so `obs.c` frees
//! it with `deque_free` as before.
//!
//! Intentional differences, where C dereferences a null pointer or reads
//! out of bounds and libobs never gets there: a null `mixes`, `buf`, `ts`,
//! `buffering` or field pointer does nothing (the buffering functions then
//! return a step with `added` false); a null mix or source channel buffer
//! is skipped; `channels` above `MAX_AUDIO_CHANNELS` is cut to it. The
//! safe core lists the rest ([`crate::audio_mix`]).

use core::ffi::c_void;
use core::ptr;
use core::slice;

use obs_util::ffi::deque::{deque, deque_push_front};

use crate::audio_mix::{
    AUDIO_OUTPUT_FRAMES, Buffering, MAX_AUDIO_CHANNELS, MAX_AUDIO_MIXES, Step, TsInfo,
    buffering_add, buffering_fixed, buffering_maxed, mix_into, mix_start, time_to_frames,
};

/// `struct ts_info` (`obs-audio-mix.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct ts_info {
    pub start: u64,
    pub end: u64,
}

impl From<ts_info> for TsInfo {
    fn from(ts: ts_info) -> Self {
        Self {
            start: ts.start,
            end: ts.end,
        }
    }
}

impl From<TsInfo> for ts_info {
    fn from(ts: TsInfo) -> Self {
        Self {
            start: ts.start,
            end: ts.end,
        }
    }
}

/// `struct audio_output_data` (`media-io/audio-io.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct audio_output_data {
    pub data: [*mut f32; MAX_AUDIO_CHANNELS],
}

/// `struct obs_audio_buffering` (`obs-audio-mix.h`): the buffering fields
/// of `struct obs_core_audio`, by pointer.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct obs_audio_buffering {
    pub buffered_ts: *mut u64,
    pub buffered_timestamps: *mut deque,
    pub buffering_wait_ticks: *mut u64,
    pub total_buffering_ticks: *mut i32,
    pub max_buffering_ticks: i32,
}

/// `struct obs_audio_buffering_step` (`obs-audio-mix.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_audio_buffering_step {
    pub added: bool,
    pub max_reached: bool,
    pub ticks: i32,
    pub ms: usize,
    pub total_ms: usize,
}

impl From<Step> for obs_audio_buffering_step {
    fn from(step: Step) -> Self {
        Self {
            added: step.added,
            max_reached: step.max_reached,
            ticks: step.ticks,
            ms: step.ms,
            total_ms: step.total_ms,
        }
    }
}

/// `obs_audio_mix_time_to_frames()`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_audio_mix_time_to_frames(sample_rate: usize, t: u64) -> usize {
    time_to_frames(sample_rate, t)
}

/// `obs_audio_mix_add()`: adds the source's audio in `buf` to the first
/// `channels` channels of every mix in `mixes`.
///
/// # Safety
///
/// `mixes` points to `MAX_AUDIO_MIXES` `struct audio_output_data` and `buf`
/// to the source's `audio_output_buf` (`MAX_AUDIO_MIXES` rows of
/// `MAX_AUDIO_CHANNELS` pointers); each pointer they hold for a mixed
/// channel is null or `AUDIO_OUTPUT_FRAMES` floats, and no mix buffer
/// overlaps a source buffer. `ts` is a valid `struct ts_info`. Null
/// `mixes`, `buf` or `ts` does nothing.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_audio_mix_add(
    mixes: *mut audio_output_data,
    buf: *mut [*mut f32; MAX_AUDIO_CHANNELS],
    audio_ts: u64,
    ts: *const ts_info,
    channels: usize,
    sample_rate: usize,
) {
    if mixes.is_null() || buf.is_null() || ts.is_null() {
        return;
    }
    // SAFETY: `ts` is a valid struct (the caller's contract).
    let ts = TsInfo::from(unsafe { *ts });
    let Some(start) = mix_start(audio_ts, ts, sample_rate) else {
        return;
    };
    // SAFETY: MAX_AUDIO_MIXES mixes and source rows (the contract).
    let (mixes, rows) = unsafe {
        (
            slice::from_raw_parts(mixes, MAX_AUDIO_MIXES),
            slice::from_raw_parts(buf, MAX_AUDIO_MIXES),
        )
    };
    for (mix, row) in mixes.iter().zip(rows) {
        for (&dst, &src) in mix.data.iter().zip(row).take(channels) {
            if dst.is_null() || src.is_null() {
                continue;
            }
            // SAFETY: both are AUDIO_OUTPUT_FRAMES floats that do not
            // overlap (the contract).
            let (dst, src) = unsafe {
                (
                    slice::from_raw_parts_mut(dst, AUDIO_OUTPUT_FRAMES),
                    slice::from_raw_parts(src, AUDIO_OUTPUT_FRAMES),
                )
            };
            mix_into(dst, src, start);
        }
    }
}

/// The fields behind `buffering`, or `None` when a pointer is null.
///
/// # Safety
///
/// `buffering` is null or a valid `struct obs_audio_buffering` whose field
/// pointers are null or valid.
unsafe fn read(buffering: *const obs_audio_buffering) -> Option<(obs_audio_buffering, Buffering)> {
    if buffering.is_null() {
        return None;
    }
    // SAFETY: a valid struct (the contract).
    let fields = unsafe { *buffering };
    if fields.buffered_ts.is_null()
        || fields.buffered_timestamps.is_null()
        || fields.buffering_wait_ticks.is_null()
        || fields.total_buffering_ticks.is_null()
    {
        return None;
    }
    // SAFETY: the field pointers are valid (checked non-null, the contract).
    let state = unsafe {
        Buffering {
            buffered_ts: *fields.buffered_ts,
            wait_ticks: *fields.buffering_wait_ticks,
            total_ticks: *fields.total_buffering_ticks,
            max_ticks: fields.max_buffering_ticks,
        }
    };
    Some((fields, state))
}

/// Writes the state back to the fields.
///
/// # Safety
///
/// The field pointers of `fields` are valid (as [`read`] returned them).
unsafe fn write(fields: &obs_audio_buffering, state: &Buffering) {
    // SAFETY: valid field pointers (the contract).
    unsafe {
        *fields.buffered_ts = state.buffered_ts;
        *fields.buffering_wait_ticks = state.wait_ticks;
        *fields.total_buffering_ticks = state.total_ticks;
    }
}

/// A `push_front` for the safe core: one `struct ts_info` to the front of
/// the C deque, as the C's `deque_push_front` does.
///
/// # Safety
///
/// `queue` is a valid `struct deque` of `struct ts_info` with bmem memory,
/// not accessed elsewhere during the call.
unsafe fn push_front_to(queue: *mut deque) -> impl FnMut(TsInfo) {
    move |window| {
        let window = ts_info::from(window);
        // SAFETY: a valid deque (the contract); one whole entry.
        unsafe {
            deque_push_front(
                queue,
                ptr::from_ref(&window).cast::<c_void>(),
                size_of::<ts_info>(),
            );
        }
    }
}

/// `obs_audio_buffering_maxed()`. A null pointer is not at the maximum.
///
/// # Safety
///
/// As [`obs_audio_buffering_fixed`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_audio_buffering_maxed(buffering: *const obs_audio_buffering) -> bool {
    // SAFETY: the caller's contract.
    unsafe { read(buffering) }.is_some_and(|(_, state)| buffering_maxed(&state))
}

/// `obs_audio_buffering_fixed()`.
///
/// # Safety
///
/// `buffering` is null or a valid `struct obs_audio_buffering`: its field
/// pointers are null or point to the fields of one `struct
/// obs_core_audio` (the deque of `struct ts_info` with bmem memory), and
/// nothing else accesses them during the call. `ts` is null or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_audio_buffering_fixed(
    buffering: *const obs_audio_buffering,
    sample_rate: usize,
    ts: *mut ts_info,
) -> obs_audio_buffering_step {
    // SAFETY: the caller's contract.
    let Some((fields, mut state)) = (unsafe { read(buffering) }) else {
        return obs_audio_buffering_step::default();
    };
    if ts.is_null() {
        return obs_audio_buffering_step::default();
    }
    // SAFETY: a valid ts_info (the contract).
    let mut window = TsInfo::from(unsafe { *ts });
    // SAFETY: the deque of the fields (the contract).
    let push = unsafe { push_front_to(fields.buffered_timestamps) };
    let step = buffering_fixed(&mut state, sample_rate, &mut window, push);
    // SAFETY: the field pointers read checked, and a valid ts.
    unsafe {
        write(&fields, &state);
        *ts = window.into();
    }
    step.into()
}

/// `obs_audio_buffering_add()`.
///
/// # Safety
///
/// As [`obs_audio_buffering_fixed`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_audio_buffering_add(
    buffering: *const obs_audio_buffering,
    sample_rate: usize,
    ts: *mut ts_info,
    min_ts: u64,
) -> obs_audio_buffering_step {
    // SAFETY: the caller's contract.
    let Some((fields, mut state)) = (unsafe { read(buffering) }) else {
        return obs_audio_buffering_step::default();
    };
    if ts.is_null() {
        return obs_audio_buffering_step::default();
    }
    // SAFETY: a valid ts_info (the contract).
    let mut window = TsInfo::from(unsafe { *ts });
    // SAFETY: the deque of the fields (the contract).
    let push = unsafe { push_front_to(fields.buffered_timestamps) };
    let step = buffering_add(&mut state, sample_rate, &mut window, min_ts, push);
    // SAFETY: the field pointers read checked, and a valid ts.
    unsafe {
        write(&fields, &state);
        *ts = window.into();
    }
    step.into()
}
