//! `oracle_*` build of `libobs/obs-video-frame-timing.c.h`
//! (oracle/video_frame_timing.c): the frame timing decisions of the graphics
//! thread loop, renamed.
//!
//! The structs come back as the `Oracle*` mirrors below;
//! [`oracle_video_frame_timing_layout`] says how the C sees all three.

/// Index of each entry in `oracle_video_frame_timing_layout`, in the C's
/// order.
pub const LAYOUT_NAMES: [&str; 15] = [
    "sizeof(struct obs_video_frame_tick)",
    "alignof(struct obs_video_frame_tick)",
    "offsetof(struct obs_video_frame_tick, delta_ns)",
    "offsetof(struct obs_video_frame_tick, seconds)",
    "sizeof(struct obs_video_frame_sums)",
    "alignof(struct obs_video_frame_sums)",
    "offsetof(struct obs_video_frame_sums, frame_time_total_ns)",
    "offsetof(struct obs_video_frame_sums, fps_total_ns)",
    "offsetof(struct obs_video_frame_sums, fps_total_frames)",
    "sizeof(struct obs_video_frame_stats)",
    "alignof(struct obs_video_frame_stats)",
    "offsetof(struct obs_video_frame_stats, sums)",
    "offsetof(struct obs_video_frame_stats, report)",
    "offsetof(struct obs_video_frame_stats, fps)",
    "offsetof(struct obs_video_frame_stats, avg_frame_time_ns)",
];

/// `struct obs_video_frame_tick` as the oracle returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OracleTick {
    pub delta_ns: u64,
    pub seconds: f32,
}

/// `struct obs_video_frame_sums` as the oracle takes and returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OracleSums {
    pub frame_time_total_ns: u64,
    pub fps_total_ns: u64,
    pub fps_total_frames: u32,
}

/// `struct obs_video_frame_stats` as the oracle returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OracleStats {
    pub sums: OracleSums,
    pub report: bool,
    pub fps: f64,
    pub avg_frame_time_ns: u64,
}

unsafe extern "C" {
    pub fn oracle_obs_video_frame_timing_tick(
        cur_time: u64,
        last_time: u64,
        interval_ns: u64,
    ) -> OracleTick;

    pub fn oracle_obs_video_frame_timing_stats(
        sums: OracleSums,
        frame_time_ns: u64,
        video_time: u64,
        last_time: u64,
    ) -> OracleStats;

    pub fn oracle_video_frame_timing_layout(which: usize) -> usize;
}
