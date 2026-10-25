//! C ABI for `libobs/obs-output-stop-stats.c.h`, the stop statistics of
//! `log_frame_info()` in `obs-output.c`, declared in
//! `libobs/obs-output-stop-stats.h` without `EXPORT`.
//!
//! `obs-output.c.h` (still C) calls `obs_video_stats_frame_counts()`, reads
//! the output's starting counts, calls `obs_output_get_frames_dropped()`
//! (the plugin's `get_dropped_frames()`) and reads `total_frames`, in that
//! order, and passes the values here by value. It logs every line itself,
//! with its format strings, from the [`obs_output_stop_stats`] returned
//! (Tier 3 checks its layout against the C). So `struct obs_output` is never
//! mirrored, and this shim takes no lock, calls nothing in libobs and
//! dereferences no pointer. It stays hidden (`rust-hidden.txt`).
//!
//! Intentional difference: `total - dropped` wraps where the C's `int`
//! subtraction overflows (undefined in C; every compiler OBS supports wraps).

use std::os::raw::c_int;

use crate::stop_stats::{self as core, Counts};

/// `struct obs_output_stop_stats` (`obs-output-stop-stats.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_output_stop_stats {
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

/// `struct obs_output_stop_stats obs_output_stop_stats(uint32_t
/// total_frames, uint32_t lagged_frames, uint32_t starting_drawn, uint32_t
/// starting_lagged, int dropped, int total)`: each line's first number,
/// and whether it is the attempted form. The attempted numbers are the C's
/// `total` and `drawn`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_stop_stats(
    total_frames: u32,
    lagged_frames: u32,
    starting_drawn: u32,
    starting_lagged: u32,
    dropped: c_int,
    total: c_int,
) -> obs_output_stop_stats {
    let s = core::stats(Counts {
        total_frames,
        lagged_frames,
        starting_drawn,
        starting_lagged,
        dropped,
        total,
    });
    obs_output_stop_stats {
        drawn: s.drawn,
        lagged: s.lagged,
        percentage_lagged: s.percentage_lagged,
        percentage_dropped: s.percentage_dropped,
        frames_output: s.total_line.first(),
        output_attempted: s.total_line.is_attempted(),
        frames_drawn: s.drawn_line.first(),
        drawn_attempted: s.drawn_line.is_attempted(),
        lag_line: s.lag_line,
        drop_line: s.drop_line,
    }
}
