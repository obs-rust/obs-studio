//! C ABI for `libobs/obs-video-frame-timing.c.h`, the frame timing decisions
//! of the graphics thread loop in `obs-video.c`, declared in
//! `libobs/obs-video-frame-timing.h` without `EXPORT`.
//!
//! These only decide. `obs-video.c.h` (still C) reads the clock (it passes
//! the times here), calls the tick callbacks and ticks the sources under
//! their mutexes, runs the profiler, calls `obs_video_stats_set_timing()` and
//! stores the context sums. The structs cross as `#[repr(C)]` mirrors whose
//! layout Tier 3 checks against the C. No pointer comes here. Both stay
//! hidden (`rust-hidden.txt`).
//!
//! The one intentional difference is the core's (a report with the frame
//! count wrapped to 0; unreachable).

use crate::video_frame_timing::{self as timing, Stats, Sums, Tick};

/// `struct obs_video_frame_tick` (`obs-video-frame-timing.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_video_frame_tick {
    pub delta_ns: u64,
    pub seconds: f32,
}

/// `struct obs_video_frame_sums` (`obs-video-frame-timing.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_video_frame_sums {
    pub frame_time_total_ns: u64,
    pub fps_total_ns: u64,
    pub fps_total_frames: u32,
}

/// `struct obs_video_frame_stats` (`obs-video-frame-timing.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_video_frame_stats {
    pub sums: obs_video_frame_sums,
    pub report: bool,
    pub fps: f64,
    pub avg_frame_time_ns: u64,
}

impl From<Tick> for obs_video_frame_tick {
    fn from(tick: Tick) -> Self {
        Self {
            delta_ns: tick.delta_ns,
            seconds: tick.seconds,
        }
    }
}

impl From<obs_video_frame_sums> for Sums {
    fn from(sums: obs_video_frame_sums) -> Self {
        Self {
            frame_time_total_ns: sums.frame_time_total_ns,
            fps_total_ns: sums.fps_total_ns,
            fps_total_frames: sums.fps_total_frames,
        }
    }
}

impl From<Sums> for obs_video_frame_sums {
    fn from(sums: Sums) -> Self {
        Self {
            frame_time_total_ns: sums.frame_time_total_ns,
            fps_total_ns: sums.fps_total_ns,
            fps_total_frames: sums.fps_total_frames,
        }
    }
}

impl From<Stats> for obs_video_frame_stats {
    fn from(stats: Stats) -> Self {
        Self {
            sums: stats.sums.into(),
            report: stats.report,
            fps: stats.fps,
            avg_frame_time_ns: stats.avg_frame_time_ns,
        }
    }
}

/// `struct obs_video_frame_tick obs_video_frame_timing_tick(uint64_t
/// cur_time, uint64_t last_time, uint64_t interval_ns)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_frame_timing_tick(
    cur_time: u64,
    last_time: u64,
    interval_ns: u64,
) -> obs_video_frame_tick {
    timing::tick(cur_time, last_time, interval_ns).into()
}

/// `struct obs_video_frame_stats obs_video_frame_timing_stats(struct
/// obs_video_frame_sums sums, uint64_t frame_time_ns, uint64_t video_time,
/// uint64_t last_time)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_video_frame_timing_stats(
    sums: obs_video_frame_sums,
    frame_time_ns: u64,
    video_time: u64,
    last_time: u64,
) -> obs_video_frame_stats {
    timing::stats(sums.into(), frame_time_ns, video_time, last_time).into()
}
