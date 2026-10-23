//! C ABI for `libobs/obs-source-deinterlace-pacing.c.h`, the 2x deinterlace
//! frame pacing, declared in `libobs/obs-source-deinterlace-pacing.h`
//! without `EXPORT`.
//!
//! `deinterlace_get_closest_frames()` and `ready_deinterlace_frames()` in
//! `obs-source-deinterlace.c.h` (still C) read the source's fields and
//! `obs->video`, pass the values in and store what comes back. So `struct
//! obs_source` is never mirrored, and these shims take plain integers,
//! touch no memory, take no lock and call nothing in libobs. All six stay
//! hidden (`rust-hidden.txt`).
//!
//! No intentional differences from C: every input is a value, so there is
//! no pointer to check.

use crate::deinterlace_pacing as core;

/// `bool obs_source_deinterlace_pacing_hold(uint64_t frame_ts, uint64_t
/// offset, uint32_t half_duration, uint64_t sys_time)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_pacing_hold(
    frame_ts: u64,
    offset: u64,
    half_duration: u32,
    sys_time: u64,
) -> bool {
    core::hold(frame_ts, offset, half_duration, sys_time)
}

/// `bool obs_source_deinterlace_pacing_wait_for_next(uint64_t timestamp,
/// uint64_t next_timestamp, uint64_t offset, uint64_t sys_time)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_pacing_wait_for_next(
    timestamp: u64,
    next_timestamp: u64,
    offset: u64,
    sys_time: u64,
) -> bool {
    core::wait_for_next(timestamp, next_timestamp, offset, sys_time)
}

/// `uint64_t obs_source_deinterlace_pacing_waiting_frame_ts(uint64_t
/// timestamp, uint64_t next_timestamp)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_pacing_waiting_frame_ts(
    timestamp: u64,
    next_timestamp: u64,
) -> u64 {
    core::waiting_frame_ts(timestamp, next_timestamp)
}

/// `uint32_t obs_source_deinterlace_pacing_half_duration(uint64_t
/// timestamp, uint64_t prev_ts, uint32_t prev_half, uint64_t
/// frame_interval_ns)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_pacing_half_duration(
    timestamp: u64,
    prev_ts: u64,
    prev_half: u32,
    frame_interval_ns: u64,
) -> u32 {
    core::half_duration(timestamp, prev_ts, prev_half, frame_interval_ns)
}

/// `uint64_t obs_source_deinterlace_pacing_buffered_offset(uint64_t
/// video_time, uint64_t last_frame_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_pacing_buffered_offset(
    video_time: u64,
    last_frame_ts: u64,
) -> u64 {
    core::buffered_offset(video_time, last_frame_ts)
}

/// `uint64_t obs_source_deinterlace_pacing_reanchor(uint64_t offset,
/// uint64_t video_time, uint64_t frame_ts, uint32_t half_duration,
/// uint64_t frame_interval_ns)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_pacing_reanchor(
    offset: u64,
    video_time: u64,
    frame_ts: u64,
    half_duration: u32,
    frame_interval_ns: u64,
) -> u64 {
    core::reanchor(
        offset,
        video_time,
        frame_ts,
        half_duration,
        frame_interval_ns,
    )
}
