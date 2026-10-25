//! C ABI for `libobs/obs-audio-min-ts.c.h`.
//!
//! `obs-audio.c` calls the three functions inside libobs, for each audio
//! source under its `audio_buf_mutex`. They are declared in
//! `obs-audio-min-ts.h` without `EXPORT`, so the shims stay hidden
//! (`libobs/cmake/rust-hidden.txt`). The values come in plainly;
//! `audio_pending` and the minimum come by pointer.
//!
//! Intentional differences, where C dereferences a null pointer and libobs
//! never gets there: a null `min_ts` or `pending` does nothing and returns
//! false. The safe core lists the rest ([`crate::audio_min_ts`]).

use crate::audio_min_ts::{BufferingChoice, buffering, earlier, insufficient};

/// `enum obs_audio_buffering_choice` (`obs-audio-min-ts.h`), as `int`.
pub const OBS_AUDIO_BUFFERING_NONE: i32 = 0;
pub const OBS_AUDIO_BUFFERING_FIXED: i32 = 1;
pub const OBS_AUDIO_BUFFERING_ADD: i32 = 2;

/// `obs_audio_min_ts_earlier()`.
///
/// # Safety
///
/// `min_ts` is null or a valid `uint64_t`. A null `min_ts` returns false.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_audio_min_ts_earlier(
    pending: bool,
    audio_ts: u64,
    min_ts: *mut u64,
) -> bool {
    // SAFETY: null or valid (the contract).
    let Some(min_ts) = (unsafe { min_ts.as_mut() }) else {
        return false;
    };
    earlier(pending, audio_ts, min_ts)
}

/// `obs_audio_min_ts_insufficient()`.
///
/// # Safety
///
/// `pending` is null or the source's valid `audio_pending`, not accessed
/// elsewhere during the call (the caller holds the `audio_buf_mutex`). A
/// null `pending` returns false.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_audio_min_ts_insufficient(
    audio_render: bool,
    pending: *mut bool,
    audio_ts: u64,
    buffered: usize,
    sample_rate: usize,
    min_ts: u64,
) -> bool {
    // SAFETY: null or valid (the contract).
    let Some(pending) = (unsafe { pending.as_mut() }) else {
        return false;
    };
    insufficient(
        audio_render,
        pending,
        audio_ts,
        buffered,
        sample_rate,
        min_ts,
    )
}

/// `obs_audio_min_ts_buffering()`; returns an `enum
/// obs_audio_buffering_choice`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_audio_min_ts_buffering(
    fixed_buffer: bool,
    maxed: bool,
    min_ts: u64,
    tick_start: u64,
) -> i32 {
    match buffering(fixed_buffer, maxed, min_ts, tick_start) {
        BufferingChoice::None => OBS_AUDIO_BUFFERING_NONE,
        BufferingChoice::Fixed => OBS_AUDIO_BUFFERING_FIXED,
        BufferingChoice::Add => OBS_AUDIO_BUFFERING_ADD,
    }
}
