//! Tier 1: `obs_core::video_frame_timing`, the frame timing decisions of the
//! graphics thread loop.
//!
//! The `steps_*` cases port `test/cmocka/test_video_frame_timing_steps.c`
//! 1:1: its 7 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones included.
//! The C calls the hidden `obs_video_frame_timing_*` functions; here the safe
//! core is called directly. Floats are compared bit for bit.
//!
//! The `edge_*` cases are new: proptests over full-range times, intervals and
//! sums against models written from the C's expressions, and the intentional
//! difference for a report with the frame count wrapped to 0.

use obs_core::video_frame_timing::{Stats, Sums, Tick, WINDOW_NS, stats, tick};
use proptest::prelude::*;

const SEC: u64 = 1_000_000_000;
const FPS25: u64 = 40_000_000;
const FPS30: u64 = 33_333_333;
const FPS60: u64 = 16_666_667;
/// 29.97 fps, `1e9 * 1001 / 30000` rounded.
const NTSC: u64 = 33_366_700;
const T0: u64 = 1_000_000_000_000;

// ---------------------------------------------------------------------------
// helpers

/// `(float)((double)delta / 1e9)`.
#[allow(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]
fn seconds_of(delta: u64) -> f32 {
    (delta as f64 / 1e9) as f32
}

#[allow(clippy::as_conversions, clippy::cast_precision_loss)]
fn f(value: u64) -> f64 {
    value as f64
}

fn check_tick(what: &str, cur: u64, last: u64, interval: u64, delta: u64, seconds: f32) {
    let got = tick(cur, last, interval);
    assert_eq!(got.delta_ns, delta, "{what}: delta");
    assert_eq!(
        got.seconds.to_bits(),
        seconds.to_bits(),
        "{what}: seconds {} vs {seconds}",
        got.seconds
    );
}

fn sums_of(frame_time_total: u64, fps_total: u64, frames: u32) -> Sums {
    Sums {
        frame_time_total_ns: frame_time_total,
        fps_total_ns: fps_total,
        fps_total_frames: frames,
    }
}

/// The C's stats, from its expressions, with wrapping unsigned arithmetic
/// and the IEEE conversions; a 0 frame count at a report gives 0 (the C
/// divides by zero there).
#[allow(clippy::as_conversions, clippy::cast_precision_loss)]
fn model_stats(sums: Sums, frame_time: u64, video_time: u64, last_time: u64) -> Stats {
    let mut s = sums;
    s.frame_time_total_ns = s.frame_time_total_ns.wrapping_add(frame_time);
    s.fps_total_ns = s
        .fps_total_ns
        .wrapping_add(video_time.wrapping_sub(last_time));
    s.fps_total_frames = s.fps_total_frames.wrapping_add(1);
    if s.fps_total_ns < WINDOW_NS {
        return Stats {
            sums: s,
            ..Stats::default()
        };
    }
    Stats {
        sums: Sums::default(),
        report: true,
        fps: f64::from(s.fps_total_frames) / (s.fps_total_ns as f64 / 1e9),
        avg_frame_time_ns: s
            .frame_time_total_ns
            .checked_div(u64::from(s.fps_total_frames))
            .unwrap_or(0),
    }
}

// ---------------------------------------------------------------------------
// the tick time

#[test]
fn steps_tick_first_frame_is_one_interval() {
    check_tick(
        "first frame, 25 fps",
        T0,
        0,
        FPS25,
        FPS25,
        seconds_of(FPS25),
    );
    check_tick(
        "first frame, 60 fps",
        T0,
        0,
        FPS60,
        FPS60,
        seconds_of(FPS60),
    );
    check_tick(
        "normal frame",
        T0 + FPS25,
        T0,
        FPS25,
        FPS25,
        seconds_of(FPS25),
    );
    check_tick(
        "a late frame, 3 intervals",
        T0 + 3 * FPS30,
        T0,
        FPS30,
        3 * FPS30,
        seconds_of(3 * FPS30),
    );
    check_tick("29.97 fps", T0 + NTSC, T0, NTSC, NTSC, seconds_of(NTSC));
    check_tick("same time", T0, T0, FPS25, 0, 0.0);
}

#[test]
#[allow(clippy::float_cmp, clippy::excessive_precision)]
fn steps_tick_seconds_round_to_float() {
    assert_eq!(tick(T0 + FPS25, T0, FPS25).seconds, 0.04_f32);
    #[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
    let third = 0.033_333_333_f64 as f32;
    assert_eq!(tick(T0 + FPS30, T0, FPS30).seconds, third);
    assert_eq!(tick(T0 + SEC, T0, FPS25).seconds, 1.0_f32);
}

#[test]
fn steps_tick_backwards_clock_wraps() {
    check_tick(
        "1 ns backwards",
        T0 - 1,
        T0,
        FPS25,
        u64::MAX,
        seconds_of(u64::MAX),
    );
    check_tick(
        "1 s backwards",
        T0 - SEC,
        T0,
        FPS25,
        u64::MAX - SEC + 1,
        seconds_of(u64::MAX - SEC + 1),
    );
    check_tick(
        "first frame at time 0",
        0,
        0,
        FPS25,
        FPS25,
        seconds_of(FPS25),
    );
    check_tick(
        "first frame earlier than one interval",
        5,
        0,
        FPS25,
        FPS25,
        seconds_of(FPS25),
    );
}

// ---------------------------------------------------------------------------
// the frame statistics

#[test]
#[allow(clippy::float_cmp)]
fn steps_stats_accumulate_below_one_second() {
    let s = stats(sums_of(0, 0, 0), 5_000_000, T0 + FPS25, T0);
    assert!(!s.report);
    assert_eq!(s.sums, sums_of(5_000_000, FPS25, 1), "first frame");

    let s = stats(
        sums_of(5_000_000, FPS25 * 23, 23),
        4_000_000,
        T0 + FPS25,
        T0,
    );
    assert!(!s.report);
    assert_eq!(s.sums, sums_of(9_000_000, FPS25 * 24, 24), "frame 24 of 25");
    assert_eq!(s.fps, 0.0);
    assert_eq!(s.avg_frame_time_ns, 0);
}

#[test]
#[allow(clippy::float_cmp)]
fn steps_stats_report_at_one_second() {
    let s = stats(
        sums_of(24 * 2_000_000, FPS25 * 24, 24),
        2_000_000,
        T0 + FPS25,
        T0,
    );
    assert!(s.report);
    assert_eq!(s.fps, 25.0);
    assert_eq!(s.avg_frame_time_ns, 2_000_000);
    assert_eq!(s.sums, sums_of(0, 0, 0), "reset");

    let s = stats(
        sums_of(30 * 1_000_000, NTSC * 29, 29),
        1_000_000,
        T0 + NTSC,
        T0,
    );
    assert!(s.report);
    assert_eq!(s.fps, 30.0 / (f(NTSC * 30) / 1e9));
    assert_eq!(s.avg_frame_time_ns, (30 * 1_000_000 + 1_000_000) / 30);
}

#[test]
#[allow(clippy::float_cmp)]
fn steps_stats_late_frame_ends_the_window() {
    let s = stats(sums_of(1000, FPS60 * 50, 50), 7, T0 + 20 * FPS60, T0);
    assert!(s.report);
    assert_eq!(s.fps, 51.0 / (f(FPS60 * 70) / 1e9));
    assert_eq!(s.avg_frame_time_ns, 1007 / 51);
    assert_eq!(s.sums, sums_of(0, 0, 0), "reset");
}

#[test]
#[allow(clippy::float_cmp)]
fn steps_stats_wrap_at_the_limits() {
    let s = stats(sums_of(0, 0, 0), 10, T0 - 1, T0);
    assert!(s.report);
    assert_eq!(s.fps, 1.0 / (f(u64::MAX) / 1e9));
    assert_eq!(s.avg_frame_time_ns, 10);

    let s = stats(sums_of(u64::MAX, 0, 3), 2, T0, T0);
    assert!(!s.report);
    assert_eq!(s.sums, sums_of(1, 0, 4), "render time sum wraps");

    let s = stats(sums_of(0, 0, u32::MAX - 1), 0, T0, T0);
    assert!(!s.report);
    assert_eq!(s.sums, sums_of(0, 0, u32::MAX), "frame count to UINT32_MAX");
}

// ---------------------------------------------------------------------------
// edges

/// Intentional difference, unreachable in libobs: a report with the frame
/// count wrapped to 0 (the C divides by zero) gives an average of 0.
#[test]
#[allow(clippy::float_cmp)]
fn edge_report_with_zero_frames() {
    let s = stats(sums_of(5, WINDOW_NS, u32::MAX), 0, T0, T0);
    assert!(s.report);
    assert_eq!(s.avg_frame_time_ns, 0);
    assert_eq!(s.fps, 0.0);
    assert_eq!(s.sums, Sums::default());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// Any times and interval.
    #[test]
    fn edge_tick_matches_the_model(cur in any::<u64>(), last in prop_oneof![Just(0_u64), any::<u64>()], interval in any::<u64>()) {
        let last_used = if last == 0 { cur.wrapping_sub(interval) } else { last };
        let delta = cur.wrapping_sub(last_used);
        prop_assert_eq!(tick(cur, last, interval), Tick { delta_ns: delta, seconds: seconds_of(delta) });
    }

    /// Any sums, render time and times.
    #[test]
    fn edge_stats_match_the_model(
        frame_time_total in any::<u64>(),
        fps_total in prop_oneof![0..WINDOW_NS, any::<u64>()],
        frames in any::<u32>(),
        frame_time in any::<u64>(),
        video_time in any::<u64>(),
        last_time in any::<u64>(),
    ) {
        let sums = sums_of(frame_time_total, fps_total, frames);
        let got = stats(sums, frame_time, video_time, last_time);
        let want = model_stats(sums, frame_time, video_time, last_time);
        prop_assert_eq!(got.sums, want.sums);
        prop_assert_eq!(got.report, want.report);
        prop_assert_eq!(got.fps.to_bits(), want.fps.to_bits());
        prop_assert_eq!(got.avg_frame_time_ns, want.avg_frame_time_ns);
    }

    /// A realistic run: frames at a fixed interval report about once a
    /// second, with an fps close to 1e9 / interval.
    #[test]
    fn edge_realistic_run_reports_once_a_second(interval in 10_000_000..=50_000_000_u64, render in 0..10_000_000_u64) {
        let mut sums = Sums::default();
        let mut reports = 0;
        let mut time = T0;
        for _ in 0..(3 * WINDOW_NS / interval) {
            let s = stats(sums, render, time + interval, time);
            time += interval;
            sums = s.sums;
            if s.report {
                reports += 1;
                prop_assert!((s.fps - f(WINDOW_NS) / f(interval)).abs() < 1.0);
                prop_assert_eq!(s.avg_frame_time_ns, render);
            }
        }
        prop_assert!((2..=3).contains(&reports));
    }
}
