//! Safe core of `libobs/obs-output-stop-stats.c.h`: the stop statistics of
//! `log_frame_info()` in `obs-output.c`.
//!
//! When an output with video stops, `log_frame_info()` logs "stopping" and
//! up to four lines: the frames output (with the attempted count when
//! frames were dropped), the frames drawn (with the attempted count when
//! frames lagged), the lagged frames and the dropped frames, each with a
//! percentage. [`stats`] decides the numbers and which lines are logged.
//!
//! `obs-output.c` keeps `obs_video_stats_frame_counts()`, the reads of the
//! output's starting counts and `total_frames`, the plugin's
//! `get_dropped_frames()` (through `obs_output_get_frames_dropped()`), in
//! that order, and every `blog()` with its format string, so the log text
//! cannot change. It passes the values here and logs from the result. This
//! keeps no state and calls nothing in libobs.
//!
//! The counters are `u32` and wrap as the C's do. `dropped` is the plugin's
//! `int`, any value: `total - dropped` wraps (the C's `int` subtraction is
//! undefined where it overflows; every compiler OBS supports wraps). The
//! percentages are the C's `double` expressions, in the same order, so they
//! are bit for bit the C's. Nothing here can panic on any input.

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

/// What `log_frame_info()` reads before it logs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// The video's drawn frames now (`obs_video_stats_frame_counts()`).
    pub total_frames: u32,
    /// The video's lagged frames now.
    pub lagged_frames: u32,
    /// The drawn frames when the output started (`starting_drawn_count`).
    pub starting_drawn: u32,
    /// The lagged frames when the output started (`starting_lagged_count`).
    pub starting_lagged: u32,
    /// The plugin's dropped frames (`obs_output_get_frames_dropped()`).
    pub dropped: i32,
    /// The frames the output was given (`total_frames`).
    pub total: i32,
}

/// One of the two count lines: the count alone, or the count that made it
/// with the attempted count after it (`"%d (%d attempted)"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line<T> {
    /// `"Total ...: %d"`.
    Plain(T),
    /// `"Total ...: %d (%d attempted)"`.
    Attempted {
        /// The count that made it: `total - dropped` or `drawn - lagged`.
        done: T,
        /// The attempted count: `total` or `drawn`.
        attempted: T,
    },
}

impl<T: Copy> Line<T> {
    /// The first number the line prints.
    #[must_use]
    pub fn first(self) -> T {
        match self {
            Line::Plain(n) | Line::Attempted { done: n, .. } => n,
        }
    }

    /// True for [`Line::Attempted`].
    #[must_use]
    pub fn is_attempted(self) -> bool {
        matches!(self, Line::Attempted { .. })
    }
}

/// What `log_frame_info()` logs after "stopping".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    /// Frames drawn since the output started (wrapping).
    pub drawn: u32,
    /// Frames lagged since the output started (wrapping).
    pub lagged: u32,
    /// `lagged / drawn * 100` as `f64`, 0 when nothing was drawn.
    pub percentage_lagged: f64,
    /// `dropped / total * 100` as `f64`, 0 when nothing was dropped; an
    /// infinity when `total` is 0 (never logged then).
    pub percentage_dropped: f64,
    /// "Total frames output".
    pub total_line: Line<i32>,
    /// "Total drawn frames".
    pub drawn_line: Line<u32>,
    /// "Number of lagged frames due to rendering lag/stalls": `lagged` and
    /// `percentage_lagged`.
    pub lag_line: bool,
    /// "Number of dropped frames due to insufficient bandwidth/connection
    /// stalls": `dropped` and `percentage_dropped`.
    pub drop_line: bool,
}

/// `log_frame_info()`'s numbers and lines from what it read.
#[must_use]
pub fn stats(counts: Counts) -> Stats {
    let Counts {
        total_frames,
        lagged_frames,
        starting_drawn,
        starting_lagged,
        dropped,
        total,
    } = counts;
    let drawn = total_frames.wrapping_sub(starting_drawn);
    let lagged = lagged_frames.wrapping_sub(starting_lagged);

    // The C's double expressions: converted, divided, then multiplied.
    let percentage_lagged = if drawn != 0 {
        f64::from(lagged) / f64::from(drawn) * 100.0
    } else {
        0.0
    };
    let percentage_dropped = if dropped != 0 {
        f64::from(dropped) / f64::from(total) * 100.0
    } else {
        0.0
    };

    let total_line = if dropped == 0 || total == 0 {
        Line::Plain(total)
    } else {
        Line::Attempted {
            done: total.wrapping_sub(dropped),
            attempted: total,
        }
    };
    let drawn_line = if lagged == 0 || drawn == 0 {
        Line::Plain(drawn)
    } else {
        Line::Attempted {
            done: drawn.wrapping_sub(lagged),
            attempted: drawn,
        }
    };

    Stats {
        drawn,
        lagged,
        percentage_lagged,
        percentage_dropped,
        total_line,
        drawn_line,
        lag_line: drawn != 0 && lagged != 0,
        drop_line: total != 0 && dropped != 0,
    }
}
