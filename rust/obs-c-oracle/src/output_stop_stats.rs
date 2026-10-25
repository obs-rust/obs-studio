//! `oracle_*` build of `libobs/obs-output-stop-stats.c.h`
//! (oracle/output_stop_stats.c): the stop statistics of `log_frame_info()`,
//! renamed.
//!
//! The struct returned by value comes back as [`Stats`], whose layout
//! [`oracle_output_stop_stats_layout`] checks. The divisions are `double`,
//! so nothing traps. `total - dropped` is the C's `int` subtraction: callers
//! keep it within `i32` when dropped and total are both non-zero, where the
//! C is defined.

use core::ffi::c_int;

/// Index of each entry in `oracle_output_stop_stats_layout`, in the C's
/// order.
pub const LAYOUT_NAMES: [&str; 12] = [
    "sizeof(struct obs_output_stop_stats)",
    "alignof(struct obs_output_stop_stats)",
    "offsetof(struct obs_output_stop_stats, drawn)",
    "offsetof(struct obs_output_stop_stats, lagged)",
    "offsetof(struct obs_output_stop_stats, percentage_lagged)",
    "offsetof(struct obs_output_stop_stats, percentage_dropped)",
    "offsetof(struct obs_output_stop_stats, frames_output)",
    "offsetof(struct obs_output_stop_stats, output_attempted)",
    "offsetof(struct obs_output_stop_stats, frames_drawn)",
    "offsetof(struct obs_output_stop_stats, drawn_attempted)",
    "offsetof(struct obs_output_stop_stats, lag_line)",
    "offsetof(struct obs_output_stop_stats, drop_line)",
];

/// `struct obs_output_stop_stats`, as the C returns it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stats {
    pub drawn: u32,
    pub lagged: u32,
    pub percentage_lagged: f64,
    pub percentage_dropped: f64,
    pub frames_output: c_int,
    pub output_attempted: bool,
    pub frames_drawn: u32,
    pub drawn_attempted: bool,
    pub lag_line: bool,
    pub drop_line: bool,
}

unsafe extern "C" {
    /// The C's `total - dropped` is undefined where it overflows `int`
    /// (with both non-zero).
    pub fn oracle_obs_output_stop_stats(
        total_frames: u32,
        lagged_frames: u32,
        starting_drawn: u32,
        starting_lagged: u32,
        dropped: c_int,
        total: c_int,
    ) -> Stats;

    pub fn oracle_output_stop_stats_layout(which: usize) -> usize;
}
