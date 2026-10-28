//! C ABI for `libobs/obs-video-pacing.c.h`, the frame pacing decisions of
//! `video_sleep()` in `obs-video.c`, declared in `libobs/obs-video-pacing.h`
//! without `EXPORT`.
//!
//! These only decide. `obs-video.c.h` (still C) sleeps and reads the clock
//! (it passes the results here), adds the frames to the stats, walks the
//! ready encoder groups under their mutexes and stores a group's start
//! timestamp, and pushes the frame info into the mixes' deques under the mix
//! list's mutex. The step struct crosses as a `#[repr(C)]` mirror whose
//! layout Tier 3 checks against the C. No pointer comes here. Both stay
//! hidden (`rust-hidden.txt`).
//!
//! The one intentional difference is the core's (an interval of 0;
//! unreachable).

use core::ffi::c_int;

use crate::video_pacing::{self as pacing, Step};

/// `struct obs_video_pacing_step` (`obs-video-pacing.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_video_pacing_step {
    pub frame_time: u64,
    pub count: c_int,
}

impl From<Step> for obs_video_pacing_step {
    fn from(step: Step) -> Self {
        Self {
            frame_time: step.frame_time,
            count: step.count,
        }
    }
}

/// `struct obs_video_pacing_step obs_video_pacing_step(uint64_t frame_time,
/// uint64_t interval_ns, bool slept, uint64_t now)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_pacing_step(
    frame_time: u64,
    interval_ns: u64,
    slept: bool,
    now: u64,
) -> obs_video_pacing_step {
    pacing::step(frame_time, interval_ns, slept, now).into()
}

/// `bool obs_video_pacing_group_starts(uint32_t num_encoders_started, size_t
/// num_encoders, uint64_t start_timestamp)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_pacing_group_starts(
    num_encoders_started: u32,
    num_encoders: usize,
    start_timestamp: u64,
) -> bool {
    pacing::group_starts(num_encoders_started, num_encoders, start_timestamp)
}
