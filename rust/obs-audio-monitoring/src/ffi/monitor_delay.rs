//! C ABI for `libobs/audio-monitoring/monitor-delay.c.h` and
//! `monitor-video-sync.c.h`.
//!
//! The WASAPI monitor (`win32/wasapi-output.c`) calls these inside libobs on
//! Windows. The C files have no `EXPORT`, so the symbols stay hidden
//! (`rust-hidden.d`). The six functions keep the C signatures; `enum
//! audio_monitor_delay_verdict` (0 to 2) crosses as `unsigned int`. A null
//! pointer, which the C dereferences, is read as 0 and not written;
//! `wasapi-output.c` always passes its fields and locals.
//!
//! Only Windows calls them. Elsewhere the shims are built but unused, and
//! hidden all the same.

use crate::monitor_delay::{VideoTsStep, diff, judge, stale, stalled, unsynced, video_ts_step};

/// Same contract as C `audio_monitor_delay_stale`.
///
/// # Safety
///
/// `last_recv_time` is null or points to a `uint64_t` the caller may write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_monitor_delay_stale(
    last_recv_time: *mut u64,
    cur_time: u64,
) -> bool {
    // SAFETY: null or a writable uint64_t, as the caller promises.
    match unsafe { last_recv_time.as_mut() } {
        Some(last) => stale(last, cur_time),
        None => stale(&mut 0, cur_time),
    }
}

/// Same contract as C `audio_monitor_delay_diff`.
///
/// # Safety
///
/// `front_ts` is null or points to a `uint64_t` the caller may write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_monitor_delay_diff(
    cur_ts: u64,
    pad_ns: u64,
    last_frame_ts: u64,
    front_ts: *mut u64,
) -> i64 {
    let front = diff(cur_ts, pad_ns, last_frame_ts);
    // SAFETY: null or a writable uint64_t, as the caller promises.
    if let Some(out) = unsafe { front_ts.as_mut() } {
        *out = front.front_ts;
    }
    front.diff
}

/// Same contract as C `audio_monitor_delay_unsynced`.
#[unsafe(no_mangle)]
pub extern "C" fn audio_monitor_delay_unsynced(last_frame_ts: u64, diff: i64) -> bool {
    unsynced(last_frame_ts, diff)
}

/// Same contract as C `audio_monitor_delay_stalled`.
#[unsafe(no_mangle)]
pub extern "C" fn audio_monitor_delay_stalled(time_since_prev: u64) -> bool {
    stalled(time_since_prev)
}

/// Same contract as C `audio_monitor_delay_judge_diff`.
#[unsafe(no_mangle)]
pub extern "C" fn audio_monitor_delay_judge_diff(bad_diff: bool, diff: i64) -> u32 {
    judge(bad_diff, diff).to_c()
}

/// Same contract as C `audio_monitor_track_video_ts`. Reads and writes go
/// through the pointers in the C's order and only where its branch makes
/// them, so both pointers at one `uint64_t` behave as in C.
///
/// # Safety
///
/// `prev_video_ts` and `time_since_prev` are each null or point to a
/// `uint64_t` the caller may write; they may point to the same one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_monitor_track_video_ts(
    prev_video_ts: *mut u64,
    time_since_prev: *mut u64,
    last_frame_ts: u64,
    packet_ns: u64,
) {
    // SAFETY: null or a writable uint64_t, as the caller promises. No
    // reference outlives its call, so the two pointers may be one.
    let prev = unsafe { read(prev_video_ts) };
    match video_ts_step(prev, last_frame_ts) {
        // SAFETY: as above.
        VideoTsStep::First => unsafe { write(prev_video_ts, last_frame_ts) },
        VideoTsStep::Same => {
            // SAFETY: as above.
            let count = unsafe { read(time_since_prev) };
            // SAFETY: as above.
            unsafe { write(time_since_prev, count.wrapping_add(packet_ns)) };
        }
        VideoTsStep::New => {
            // SAFETY: as above; the C stores the timestamp first.
            unsafe { write(prev_video_ts, last_frame_ts) };
            // SAFETY: as above.
            unsafe { write(time_since_prev, 0) };
        }
    }
}

/// `*p`, or 0 for null.
///
/// # Safety
///
/// `p` is null or points to a readable `uint64_t`.
unsafe fn read(p: *const u64) -> u64 {
    // SAFETY: null or readable, as the caller promises.
    unsafe { p.as_ref() }.map_or(0, |v| *v)
}

/// `*p = v`, nothing for null.
///
/// # Safety
///
/// `p` is null or points to a writable `uint64_t`.
unsafe fn write(p: *mut u64, v: u64) {
    // SAFETY: null or writable, as the caller promises.
    if let Some(out) = unsafe { p.as_mut() } {
        *out = v;
    }
}
