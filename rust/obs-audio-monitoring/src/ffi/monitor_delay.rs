//! C ABI for `libobs/audio-monitoring/monitor-delay.c.h`.
//!
//! The WASAPI monitor (`win32/wasapi-output.c`) calls these inside libobs on
//! Windows. The C file has no `EXPORT`, so the symbols stay hidden
//! (`rust-hidden.txt`). The five functions keep the C signatures; `enum
//! audio_monitor_delay_verdict` (0 to 2) crosses as `unsigned int`. A null
//! pointer, which the C dereferences, is read as 0 and not written;
//! `wasapi-output.c` always passes its fields and locals.
//!
//! Only Windows calls them. Elsewhere the shims are built but unused, and
//! hidden all the same.

use crate::monitor_delay::{diff, judge, stale, stalled, unsynced};

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
