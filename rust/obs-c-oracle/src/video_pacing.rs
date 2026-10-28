//! `oracle_*` build of `libobs/obs-video-pacing.c.h` (oracle/video_pacing.c):
//! the frame pacing decisions of `video_sleep()`, renamed.
//!
//! The step comes back as [`OraclePacingStep`];
//! [`oracle_video_pacing_layout`] says how the C sees it.

use core::ffi::c_int;

/// Index of each entry in `oracle_video_pacing_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 4] = [
    "sizeof(struct obs_video_pacing_step)",
    "alignof(struct obs_video_pacing_step)",
    "offsetof(struct obs_video_pacing_step, frame_time)",
    "offsetof(struct obs_video_pacing_step, count)",
];

/// `struct obs_video_pacing_step` as the oracle returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OraclePacingStep {
    pub frame_time: u64,
    pub count: c_int,
}

unsafe extern "C" {
    pub fn oracle_obs_video_pacing_step(
        frame_time: u64,
        interval_ns: u64,
        slept: bool,
        now: u64,
    ) -> OraclePacingStep;

    pub fn oracle_obs_video_pacing_group_starts(
        num_encoders_started: u32,
        num_encoders: usize,
        start_timestamp: u64,
    ) -> bool;

    pub fn oracle_video_pacing_layout(which: usize) -> usize;
}
