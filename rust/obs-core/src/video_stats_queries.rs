//! Safe core of `libobs/obs-video-stats-queries.c.h`: the five public
//! video-statistics queries (#5364), `obs_get_video_frame_time()`,
//! `obs_get_active_fps()`, `obs_get_average_frame_time_ns()`,
//! `obs_get_total_frames()` and `obs_get_lagged_frames()`.
//!
//! Each query is one call of its own `obs_video_stats_*` reader on the
//! current core's `video.stats` (one lock, one read, one unlock: the
//! synchronization of #175), returned unchanged. The five are independent
//! reads, never one combined snapshot, and a live core is required: there
//! are no no-core defaults.

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

/// The current core's `video.stats` readers, each one locked read.
pub trait VideoStats {
    /// `obs_video_stats_frame_time()`.
    fn frame_time(&mut self) -> u64;
    /// `obs_video_stats_fps()`.
    fn fps(&mut self) -> f64;
    /// `obs_video_stats_avg_frame_time_ns()`.
    fn avg_frame_time_ns(&mut self) -> u64;
    /// `obs_video_stats_total_frames()`.
    fn total_frames(&mut self) -> u32;
    /// `obs_video_stats_lagged_frames()`.
    fn lagged_frames(&mut self) -> u32;
}

/// `obs_get_video_frame_time`.
pub fn video_frame_time(stats: &mut impl VideoStats) -> u64 {
    let _ = stats;
    todo!("obs_get_video_frame_time port")
}

/// `obs_get_active_fps`.
pub fn active_fps(stats: &mut impl VideoStats) -> f64 {
    let _ = stats;
    todo!("obs_get_active_fps port")
}

/// `obs_get_average_frame_time_ns`.
pub fn average_frame_time_ns(stats: &mut impl VideoStats) -> u64 {
    let _ = stats;
    todo!("obs_get_average_frame_time_ns port")
}

/// `obs_get_total_frames`.
pub fn total_frames(stats: &mut impl VideoStats) -> u32 {
    let _ = stats;
    todo!("obs_get_total_frames port")
}

/// `obs_get_lagged_frames`.
pub fn lagged_frames(stats: &mut impl VideoStats) -> u32 {
    let _ = stats;
    todo!("obs_get_lagged_frames port")
}
