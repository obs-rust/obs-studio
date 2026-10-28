//! C ABI for `libobs/obs-encoder-packet-ts.c.h`, the packet timestamp
//! decisions of `obs-encoder.c`, declared in `libobs/obs-encoder-packet-ts.h`
//! without `EXPORT`.
//!
//! These only decide. `obs-encoder.c.h` (still C) runs the encode callbacks,
//! reads the clock and adds the timing entries, takes `pause.mutex` and
//! `callbacks_mutex` where it always did, tests the packet's type and
//! keyframe, copies and removes the entry found, counts the frames and
//! writes every answer back. `struct encoder_packet_timebase` crosses as a
//! `#[repr(C)]` mirror whose layout Tier 3 checks against the C; the timing
//! entries are the caller's `encoder_packet_time` array, read only. All
//! five stay hidden (`rust-hidden.txt`).
//!
//! The intentional differences are the core's (in `dts_usec`, the inputs
//! where C is undefined; unreachable).

use super::output_delay::encoder_packet_time;
use crate::encoder_packet_ts::{self as ts, NOT_FOUND, Timebase};

/// `struct obs_encoder_packet_timebase` (`obs-encoder-packet-ts.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_encoder_packet_timebase {
    pub num: i32,
    pub den: i32,
}

impl From<Timebase> for obs_encoder_packet_timebase {
    fn from(timebase: Timebase) -> Self {
        Self {
            num: timebase.num,
            den: timebase.den,
        }
    }
}

/// `struct obs_encoder_packet_timebase obs_encoder_packet_ts_timebase(
/// uint32_t timebase_num, uint32_t timebase_den, uint32_t
/// frame_rate_divisor)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_packet_ts_timebase(
    timebase_num: u32,
    timebase_den: u32,
    frame_rate_divisor: u32,
) -> obs_encoder_packet_timebase {
    ts::timebase(timebase_num, timebase_den, frame_rate_divisor).into()
}

/// `int64_t obs_encoder_packet_ts_dts_usec(int64_t dts, int32_t
/// timebase_den)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_packet_ts_dts_usec(dts: i64, timebase_den: i32) -> i64 {
    ts::dts_usec(dts, timebase_den)
}

/// `int64_t obs_encoder_packet_ts_system(uint64_t start_ts, int64_t
/// dts_usec, int64_t offset_usec)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_packet_ts_system(
    start_ts: u64,
    dts_usec: i64,
    offset_usec: i64,
) -> i64 {
    ts::system(start_ts, dts_usec, offset_usec)
}

/// `int64_t obs_encoder_packet_ts_paused(int64_t sys_dts_usec, uint64_t
/// pause_ts_offset)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_packet_ts_paused(sys_dts_usec: i64, pause_ts_offset: u64) -> i64 {
    ts::paused(sys_dts_usec, pause_ts_offset)
}

/// `size_t obs_encoder_packet_ts_find(const struct encoder_packet_time
/// *times, size_t num, int64_t pts)`: `OBS_ENCODER_PACKET_TS_NOT_FOUND`
/// when no entry has this `pts`.
///
/// # Safety
///
/// `times` points to `num` initialized entries, or `num` is 0 (then
/// `times` may be NULL). Nothing writes them during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_packet_ts_find(
    times: *const encoder_packet_time,
    num: usize,
    pts: i64,
) -> usize {
    let times: &[encoder_packet_time] = if num == 0 || times.is_null() {
        &[]
    } else {
        // SAFETY: the caller's contract: `num` initialized entries at
        // `times`, which nothing writes during the call.
        unsafe { core::slice::from_raw_parts(times, num) }
    };
    ts::find(times, pts).unwrap_or(NOT_FOUND)
}
