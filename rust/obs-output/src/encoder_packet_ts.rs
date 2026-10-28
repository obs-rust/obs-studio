//! Safe core of `libobs/obs-encoder-packet-ts.c.h`: the packet timestamp
//! decisions of `obs-encoder.c`.
//!
//! They run for each packet of each encoder (video and audio, stream and
//! recording). `obs-encoder.c` keeps the encode callbacks,
//! `os_gettime_ns()` and the timing entries it adds, `pause.mutex`,
//! `callbacks_mutex` and the callback walk, the darray (it copies and
//! removes the entry found), the video type test, the first keyframe test,
//! the frame count, `full_stop()` and the logs. It passes their values
//! here. These keep no state, take no lock and call nothing.
//!
//! - [`timebase`] (`do_encode()` and the GPU encode thread): the packet's
//!   timebase, `timebase_num * frame_rate_divisor` and `timebase_den`,
//!   stored as `int32_t`;
//! - [`dts_usec`] (`packet_dts_usec()`): a packet's DTS in microseconds,
//!   which `send_off_encoder_packet()` also takes as the encoder's
//!   `offset_usec` on the first received packet;
//! - [`system`] (`send_off_encoder_packet()`): the packet's `dts_usec` on
//!   the system clock, from the encoder's start time less the first
//!   packet's offset;
//! - [`paused`] (`send_off_encoder_packet()`, under `pause.mutex`):
//!   `sys_dts_usec` after the pause offset;
//! - [`find`] (`send_off_encoder_packet()`): the newest timing entry of a
//!   video packet's PTS. The interleaver's
//!   [`crate::interleave::find_packet_time`] takes the first instead.
//!
//! No input comes from outside OBS (the timebase from the video or audio
//! settings, the timestamps from the encoder plugin and libobs' clock), but
//! nothing here can panic on any input. The C's unsigned sums wrap, and so
//! do these; a sum above `INT64_MAX` is stored negative, as on every target
//! libobs builds for.
//!
//! Intentional differences, unreachable (undefined in C): in [`dts_usec`],
//! `dts * 1000000` beyond `i64` wraps, a `timebase_den` of 0 gives 0, and
//! `i64::MIN / -1` gives `i64::MIN`.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::interleave::PacketTime;

/// `OBS_ENCODER_PACKET_TS_NOT_FOUND` (`DARRAY_INVALID`): the C ABI's
/// "no entry" for [`find`].
pub const NOT_FOUND: usize = usize::MAX;

/// Microseconds per second (`MICROSECOND_DEN`).
pub const MICROSECOND_DEN: i64 = 1_000_000;

/// The timebase of `struct encoder_packet` (`struct
/// obs_encoder_packet_timebase`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Timebase {
    /// `timebase_num * frame_rate_divisor`, as `uint32_t`, stored as
    /// `int32_t`.
    pub num: i32,
    /// `timebase_den`, stored as `int32_t`.
    pub den: i32,
}

/// `do_encode()` and the GPU encode thread: the encoder's `timebase_num`,
/// `timebase_den` and `frame_rate_divisor`.
#[must_use]
pub fn timebase(timebase_num: u32, timebase_den: u32, frame_rate_divisor: u32) -> Timebase {
    Timebase {
        num: timebase_num.wrapping_mul(frame_rate_divisor).cast_signed(),
        den: timebase_den.cast_signed(),
    }
}

/// `packet_dts_usec()`: `dts * 1000000 / timebase_den`, truncated toward 0.
#[must_use]
pub fn dts_usec(dts: i64, timebase_den: i32) -> i64 {
    let product = dts.wrapping_mul(MICROSECOND_DEN);
    // None only where C is undefined, the intentional differences (module
    // docs): a timebase_den of 0 gives 0, and i64::MIN / -1 wraps to
    // i64::MIN, the product itself.
    match product.checked_div(i64::from(timebase_den)) {
        Some(usec) => usec,
        None if timebase_den == 0 => 0,
        None => product,
    }
}

/// `send_off_encoder_packet()`: `start_ts / 1000 + dts_usec - offset_usec`
/// as C computes it (unsigned, because `start_ts` is), stored as `int64_t`.
/// `start_ts` is the encoder's start time in ns, `dts_usec` the packet's DTS
/// in microseconds, `offset_usec` the first received packet's.
#[must_use]
pub fn system(start_ts: u64, dts_usec: i64, offset_usec: i64) -> i64 {
    start_ts
        .wrapping_div(1000)
        .wrapping_add(dts_usec.cast_unsigned())
        .wrapping_sub(offset_usec.cast_unsigned())
        .cast_signed()
}

/// `send_off_encoder_packet()`: `sys_dts_usec += pause_ts_offset / 1000`, as
/// C computes it (unsigned), stored as `int64_t`. `pause_ts_offset` is the
/// encoder's `pause.ts_offset` in ns.
#[must_use]
pub fn paused(sys_dts_usec: i64, pause_ts_offset: u64) -> i64 {
    sys_dts_usec
        .cast_unsigned()
        .wrapping_add(pause_ts_offset.wrapping_div(1000))
        .cast_signed()
}

/// `send_off_encoder_packet()`: the index of the newest entry with this
/// `pts`.
#[must_use]
pub fn find<T: PacketTime>(times: &[T], pts: i64) -> Option<usize> {
    times.iter().rposition(|t| t.pts() == pts)
}
