//! C ABI for `libobs/obs-source-async-pacing.c.h`, the async video frame
//! pacing, declared in `libobs/obs-source-async-pacing.h` without `EXPORT`.
//!
//! `get_closest_frame()` in `obs-source.c.h` (still C, under `async_mutex`)
//! copies the timestamps of `async_frames`, oldest first, into an array of
//! its own, calls [`obs_source_async_pacing`] with the source's
//! `last_frame_ts`, `last_sys_timestamp` and `async_unbuffered`, and
//! applies the [`obs_source_async_pacing_result`] it returns: the erases,
//! `remove_async_frame()`, the store of `last_frame_ts` and the frame
//! taken. So `struct obs_source` is never mirrored, no frame pointer
//! crosses, and the shim reads only the timestamps, writes nothing through
//! a pointer, takes no lock and calls nothing in libobs. It stays hidden
//! (`rust-hidden.txt`).
//!
//! Intentional difference from C: a null `timestamps` with a count above 0
//! is read as an empty queue (nothing dropped or taken, `last_frame_ts`
//! kept); the C reads through the null pointer. libobs never calls it with
//! an empty queue, and passes its own array of `count` timestamps.

use crate::async_pacing::{self as core, State};

/// `struct obs_source_async_pacing_result` (`obs-source-async-pacing.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_source_async_pacing_result {
    pub drop_front: usize,
    pub last_frame_ts: u64,
    pub take_first: bool,
}

/// `struct obs_source_async_pacing_result obs_source_async_pacing(const
/// uint64_t *timestamps, size_t count, uint64_t sys_time, uint64_t
/// last_frame_ts, uint64_t last_sys_timestamp, bool async_unbuffered)`.
///
/// # Safety
///
/// When `count` is above 0, `timestamps` is null or points to `count`
/// initialized, aligned `uint64_t`s that nothing writes during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_async_pacing(
    timestamps: *const u64,
    count: usize,
    sys_time: u64,
    last_frame_ts: u64,
    last_sys_timestamp: u64,
    async_unbuffered: bool,
) -> obs_source_async_pacing_result {
    let timestamps: &[u64] = if count == 0 || timestamps.is_null() {
        &[]
    } else {
        // SAFETY: non-null, with count readable timestamps (caller contract).
        unsafe { std::slice::from_raw_parts(timestamps, count) }
    };
    let pacing = core::pace(
        timestamps,
        sys_time,
        State {
            last_frame_ts,
            last_sys_timestamp,
            async_unbuffered,
        },
    );
    obs_source_async_pacing_result {
        drop_front: pacing.drop_front,
        last_frame_ts: pacing.last_frame_ts,
        take_first: pacing.take_first,
    }
}
