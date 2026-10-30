//! C ABI for `libobs/obs-output-pause.c.h`, the pause plan of
//! `obs-output.c`, declared in `libobs/obs-output-pause.h` without
//! `EXPORT`.
//!
//! These only decide. `obs-output.c.h` (still C) keeps `obs_output_valid()`,
//! every atomic on `active`, `paused` and the encoders' `paused` (each
//! loaded where it always was), every `pause.mutex` lock and unlock, the
//! frame interval read and `os_gettime_ns()`, the first video encoder, the
//! record stores, the signals and the log, and passes plain values here. So
//! `struct obs_output`, `struct obs_encoder` and `struct pause_data` are
//! never mirrored; the encoders come in as opaque ids (their addresses),
//! never dereferenced. These shims take no lock and call nothing in libobs.
//! The only pointers are the two slot arrays the lock set reads and the two
//! masks it writes. All seven stay hidden (`rust-hidden.txt`).
//!
//! Intentional differences: the core's (each distinct encoder locked and
//! updated once, a frame interval of 0 giving the clock).

use crate::output_pause::{self as core, LockSet, MAX_AUDIO_ENCODERS, MAX_VIDEO_ENCODERS};

/// `bool obs_output_pause_allowed(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_pause_allowed(flags: u32) -> bool {
    core::allowed(flags)
}

/// `bool obs_output_pause_already(bool paused, bool pause)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_pause_already(paused: bool, pause: bool) -> bool {
    core::already(paused, pause)
}

/// `void obs_output_pause_lock_set(const uintptr_t video[10], const
/// uintptr_t audio[6], uint32_t *video_lock, uint32_t *audio_lock)`.
///
/// # Safety
///
/// As the C: `video` points to `MAX_OUTPUT_VIDEO_ENCODERS` readable
/// `uintptr_t` and `audio` to `MAX_OUTPUT_AUDIO_ENCODERS`, and `video_lock`
/// and `audio_lock` each to a writable `uint32_t`.
/// `obs_encoded_output_pause()` passes four locals.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_pause_lock_set(
    video: *const [usize; MAX_VIDEO_ENCODERS],
    audio: *const [usize; MAX_AUDIO_ENCODERS],
    video_lock: *mut u32,
    audio_lock: *mut u32,
) {
    // SAFETY: the caller's contract; `uintptr_t` is `usize` on every target
    // libobs builds for.
    let (video, audio) = unsafe { (&*video, &*audio) };
    let LockSet { video: v, audio: a } = core::lock_set(video, audio);
    // SAFETY: the caller's contract.
    unsafe {
        video_lock.write(v);
        audio_lock.write(a);
    }
}

/// `uint64_t obs_output_pause_closest_ts(uint64_t last_video_ts, uint64_t
/// interval, uint64_t now)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_pause_closest_ts(last_video_ts: u64, interval: u64, now: u64) -> u64 {
    core::closest_ts(last_video_ts, interval, now)
}

/// `bool obs_output_pause_started(uint64_t ts_start)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_pause_started(ts_start: u64) -> bool {
    core::started(ts_start)
}

/// `bool obs_output_pause_ended(uint64_t ts_end)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_pause_ended(ts_end: u64) -> bool {
    core::ended(ts_end)
}

/// `uint64_t obs_output_pause_offset(uint64_t ts_offset, uint64_t ts_start,
/// uint64_t ts_end)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_pause_offset(ts_offset: u64, ts_start: u64, ts_end: u64) -> u64 {
    core::offset(ts_offset, ts_start, ts_end)
}
