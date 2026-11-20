//! The five public video-statistics queries over the current core's real
//! `video.stats`, reached through the private adapters of
//! `libobs/obs-video-stats-queries-fields.c.h` (#5364).
//!
//! The stats layout and its mutex stay in C: each query asks the adapter for
//! the current core's stats and calls the one `obs_video_stats_*` reader the
//! safe core picks, which locks, reads and unlocks as before (#175). A live
//! core is required, as in C.
use core::ffi::c_void;

use crate::video_stats_queries::{
    VideoStats, active_fps, average_frame_time_ns, lagged_frames, total_frames, video_frame_time,
};

unsafe extern "C" {
    fn obs_video_stats_queries_stats() -> *mut c_void;
    fn obs_video_stats_queries_frame_time(stats: *mut c_void) -> u64;
    fn obs_video_stats_queries_fps(stats: *mut c_void) -> f64;
    fn obs_video_stats_queries_avg_frame_time_ns(stats: *mut c_void) -> u64;
    fn obs_video_stats_queries_total_frames(stats: *mut c_void) -> u32;
    fn obs_video_stats_queries_lagged_frames(stats: *mut c_void) -> u32;
}

/// The current core's `struct obs_video_stats`.
struct Stats(*mut c_void);

impl VideoStats for Stats {
    fn frame_time(&mut self) -> u64 {
        // SAFETY: `self.0` is the live core's initialized stats (see `current`).
        unsafe { obs_video_stats_queries_frame_time(self.0) }
    }
    fn fps(&mut self) -> f64 {
        // SAFETY: as in `frame_time`.
        unsafe { obs_video_stats_queries_fps(self.0) }
    }
    fn avg_frame_time_ns(&mut self) -> u64 {
        // SAFETY: as in `frame_time`.
        unsafe { obs_video_stats_queries_avg_frame_time_ns(self.0) }
    }
    fn total_frames(&mut self) -> u32 {
        // SAFETY: as in `frame_time`.
        unsafe { obs_video_stats_queries_total_frames(self.0) }
    }
    fn lagged_frames(&mut self) -> u32 {
        // SAFETY: as in `frame_time`.
        unsafe { obs_video_stats_queries_lagged_frames(self.0) }
    }
}

/// The current core's stats.
///
/// # Safety
/// The current core is live, with its stats initialized.
unsafe fn current() -> Stats {
    // SAFETY: the adapter only forms the address of the live core's stats.
    Stats(unsafe { obs_video_stats_queries_stats() })
}

/// # Safety
/// The current core is live, with its stats initialized (NULL is outside
/// the valid domain and faults, as in C).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_video_frame_time() -> u64 {
    // SAFETY: the caller upholds the same contract.
    video_frame_time(&mut unsafe { current() })
}

/// # Safety
/// As for [`obs_get_video_frame_time`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_active_fps() -> f64 {
    // SAFETY: the caller upholds the same contract.
    active_fps(&mut unsafe { current() })
}

/// # Safety
/// As for [`obs_get_video_frame_time`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_average_frame_time_ns() -> u64 {
    // SAFETY: the caller upholds the same contract.
    average_frame_time_ns(&mut unsafe { current() })
}

/// # Safety
/// As for [`obs_get_video_frame_time`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_total_frames() -> u32 {
    // SAFETY: the caller upholds the same contract.
    total_frames(&mut unsafe { current() })
}

/// # Safety
/// As for [`obs_get_video_frame_time`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_lagged_frames() -> u32 {
    // SAFETY: the caller upholds the same contract.
    lagged_frames(&mut unsafe { current() })
}
