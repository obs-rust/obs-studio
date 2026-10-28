//! Safe core of `libobs/obs-video-frame-timing.c.h`: the frame timing
//! decisions of the graphics thread loop in `obs-video.c`.
//!
//! `obs_graphics_thread_loop()` runs once per frame. `obs-video.c` keeps
//! `os_gettime_ns()` (it passes the times), the tick callbacks and the source
//! list with their mutexes, the profiler, `obs_video_stats_set_timing()` and
//! the context fields (it passes them in and stores what comes back). These
//! keep no state, take no lock and call nothing.
//!
//! - [`tick`] (`tick_sources()`): the time since the last tick (one interval
//!   on the first frame, when the last tick time is 0) and its seconds as an
//!   `f32`, which every tick callback and every source's `video_tick` gets.
//! - [`stats`] (the end of `obs_graphics_thread_loop()`): the render time,
//!   frame time and frame sums, and once the frame time sum reaches one
//!   second, the fps, the average render time and the reset sums.
//!
//! No input comes from outside OBS (the times come from libobs' clock, the
//! interval from its video settings), but nothing here can panic on any
//! input. The C's unsigned arithmetic wraps, and so does this; the
//! conversions to `double` and `float` are the same IEEE roundings.
//!
//! Intentional difference, unreachable: a report with the frame count
//! wrapped to 0 (2^32 frames without one second of frame time) divides by
//! zero in the C (undefined, a crash on x86). The port reports an average
//! render time of 0 there.

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

/// One second in nanoseconds: the statistics window.
pub const WINDOW_NS: u64 = 1_000_000_000;

/// The time `tick_sources()` gives the tick callbacks and the sources
/// (`struct obs_video_frame_tick`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tick {
    /// The time since the last tick.
    pub delta_ns: u64,
    /// `delta_ns` in seconds, as the C's `(float)((double)delta / 1e9)`.
    pub seconds: f32,
}

/// The sums of `struct obs_graphics_context` between two reports
/// (`struct obs_video_frame_sums`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Sums {
    /// `context->frame_time_total_ns`.
    pub frame_time_total_ns: u64,
    /// `context->fps_total_ns`.
    pub fps_total_ns: u64,
    /// `context->fps_total_frames`.
    pub fps_total_frames: u32,
}

/// What the end of `obs_graphics_thread_loop()` does with the sums
/// (`struct obs_video_frame_stats`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// The new context sums (all 0 after a report).
    pub sums: Sums,
    /// Report `fps` and `avg_frame_time_ns` through
    /// `obs_video_stats_set_timing()`.
    pub report: bool,
    /// The frames per second over the sums (0 without a report).
    pub fps: f64,
    /// The average render time of a frame over the sums (0 without a
    /// report).
    pub avg_frame_time_ns: u64,
}

/// `tick_sources()`: `cur_time` is the frame time, `last_time` the time of
/// the last tick (0 before the first frame), `interval_ns` the frame
/// interval.
#[must_use]
pub fn tick(cur_time: u64, last_time: u64, interval_ns: u64) -> Tick {
    let last_time = if last_time == 0 {
        cur_time.wrapping_sub(interval_ns)
    } else {
        last_time
    };
    let delta_ns = cur_time.wrapping_sub(last_time);
    Tick {
        delta_ns,
        seconds: f32_of(f64_of(delta_ns) / 1e9),
    }
}

/// The end of `obs_graphics_thread_loop()`: `sums` are the context's,
/// `frame_time_ns` this frame's render time, `video_time` the frame time
/// after `video_sleep()`, `last_time` the time of this frame's tick.
#[must_use]
pub fn stats(sums: Sums, frame_time_ns: u64, video_time: u64, last_time: u64) -> Stats {
    let sums = Sums {
        frame_time_total_ns: sums.frame_time_total_ns.wrapping_add(frame_time_ns),
        fps_total_ns: sums
            .fps_total_ns
            .wrapping_add(video_time.wrapping_sub(last_time)),
        fps_total_frames: sums.fps_total_frames.wrapping_add(1),
    };
    if sums.fps_total_ns < WINDOW_NS {
        return Stats {
            sums,
            ..Stats::default()
        };
    }
    Stats {
        sums: Sums::default(),
        report: true,
        fps: f64::from(sums.fps_total_frames) / (f64_of(sums.fps_total_ns) / 1e9),
        // A frame count of 0: the intentional difference (module docs).
        avg_frame_time_ns: sums
            .frame_time_total_ns
            .checked_div(u64::from(sums.fps_total_frames))
            .unwrap_or(0),
    }
}

/// `(double)` of a `uint64_t`: round to nearest, as C does.
#[allow(clippy::as_conversions, clippy::cast_precision_loss)]
const fn f64_of(value: u64) -> f64 {
    value as f64
}

/// `(float)` of a `double`: round to nearest, as C does.
#[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
const fn f32_of(value: f64) -> f32 {
    value as f32
}
