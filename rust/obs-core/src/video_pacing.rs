//! Safe core of `libobs/obs-video-pacing.c.h`: the frame pacing decisions of
//! `video_sleep()` in `obs-video.c`.
//!
//! The graphics thread calls `video_sleep()` once per frame, after the frame
//! is rendered. `obs-video.c` keeps `os_sleepto_ns()` and `os_gettime_ns()`
//! (it passes their results), `obs_video_stats_add_frames()`, the ready
//! encoder groups with their weak references and mutexes, and the
//! `vframe_info` pushes into the mixes' deques with the mix list and its
//! mutex. These keep no state, take no lock and call nothing.
//!
//! - [`step`]: the frame time step. After a sleep that reached the next frame
//!   time, one frame and the next frame time. After a late wake-up, the
//!   whole intervals since the frame time (at least one) and the frame time
//!   that many intervals on, on the interval grid.
//! - [`group_starts`]: whether a ready encoder's group gets its start
//!   timestamp (`video_sleep()` stores the new frame time).
//!
//! No input comes from outside OBS (the frame time and the interval come from
//! libobs' video settings and its clock, the group counts from its own
//! encoders), but nothing here can panic on any input.
//!
//! The C's conversions are kept, as the compilers libobs supports do them:
//! the clock difference is read as `int64_t` (a difference of 2^63 and more
//! is negative, so one interval), a count above `INT_MAX` is truncated to
//! `int` (modulo 2^32), and the new frame time adds the interval times that
//! `int` converted to `uint64_t`, modulo 2^64.
//!
//! Intentional difference, unreachable: an interval of 0 after a late
//! wake-up divides by zero in the C (undefined, a crash on x86). The port
//! gives a count of 0 and leaves the frame time as it is. libobs computes the
//! interval from the frame rate and never passes 0.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use core::ffi::c_int;

/// The frame time step of `video_sleep()` (`struct obs_video_pacing_step`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Step {
    /// The new `video->video_time`.
    pub frame_time: u64,
    /// The frames this step covers; the frames lagged are one less.
    pub count: c_int,
}

/// `video_sleep()`: `frame_time` is `video->video_time`, `interval_ns` the
/// frame interval, `slept` the result of `os_sleepto_ns(frame_time +
/// interval_ns)`, `now` the `os_gettime_ns()` read after a late wake-up (not
/// used when `slept`).
#[must_use]
pub fn step(frame_time: u64, interval_ns: u64, slept: bool, now: u64) -> Step {
    if slept {
        return Step {
            frame_time: frame_time.wrapping_add(interval_ns),
            count: 1,
        };
    }
    // `memcpy()` of the unsigned difference into an `int64_t`, and the
    // interval as `(int64_t)`: the same bits read as signed.
    let diff = i64::from_ne_bytes(now.wrapping_sub(frame_time).to_ne_bytes());
    let interval = i64::from_ne_bytes(interval_ns.to_ne_bytes());
    let clamped_diff = if diff > interval {
        u64::from_ne_bytes(diff.to_ne_bytes())
    } else {
        interval_ns
    };
    let Some(whole) = clamped_diff.checked_div(interval_ns) else {
        // Interval 0: the intentional difference (module docs).
        return Step {
            frame_time,
            count: 0,
        };
    };
    let count = int_of(whole);
    Step {
        frame_time: frame_time.wrapping_add(interval_ns.wrapping_mul(uint64_of(count))),
        count,
    }
}

/// `(int)` of a `uint64_t`: its low 32 bits as a signed `int`.
#[allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]
const fn int_of(value: u64) -> c_int {
    value as u32 as c_int
}

/// An `int` converted to `uint64_t` for the multiplication: negative values
/// wrap (modulo 2^64), as the C's usual arithmetic conversions do.
#[allow(clippy::as_conversions, clippy::cast_sign_loss)]
const fn uint64_of(value: c_int) -> u64 {
    value as i64 as u64
}

/// `video_sleep()`: a ready encoder's group gets its start timestamp once
/// every encoder in it has started and it has none yet.
#[must_use]
pub fn group_starts(num_encoders_started: u32, num_encoders: usize, start_timestamp: u64) -> bool {
    // The C compares the uint32_t with the size_t after promotion.
    usize::try_from(num_encoders_started).map_or(true, |started| started >= num_encoders)
        && start_timestamp == 0
}
