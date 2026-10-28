//! Tier 2 layout and Tier 3 parity: the shims of
//! `obs_core::ffi::video_frame_timing` (and the safe core under them) against
//! the original `libobs/obs-video-frame-timing.c.h`, compiled as
//! `oracle_obs_video_frame_timing_*` (obs-c-oracle,
//! oracle/video_frame_timing.c).
//!
//! Every call runs the shim, the C and the safe core, and compares the
//! results exactly: the delta and the sums as integers, the report as a bool,
//! the `float` seconds and the `double` fps bit for bit.
//!
//! Runs: the layout of the three structs; exhaustive over the tick inputs at
//! 0, 1, common frame times and intervals and the `u64` edges; the
//! statistics around the one-second window (just under, at, over), with sums
//! at the `u64` and `u32` edges; a frame-by-frame run at 25, 29.97, 30 and
//! 60 fps over 10 s; and proptests over the full ranges.
//!
//! The C is undefined for a report with the frame count wrapped to 0 (it
//! divides by zero). Those inputs are not given to the C; there the shim is
//! compared with the core, which gives an average of 0. That is the one
//! intentional difference, unreachable (2^32 frames without one second of
//! frame time). `test_video_frame_timing_steps` runs the C test against the
//! C; the real-libobs source tick and profiler tests run against the hybrid
//! libobs, where these shims replace the C.

use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::video_frame_timing::{
    LAYOUT_NAMES, OracleSums, oracle_obs_video_frame_timing_stats,
    oracle_obs_video_frame_timing_tick, oracle_video_frame_timing_layout,
};
use obs_core::ffi::video_frame_timing::{
    obs_video_frame_stats, obs_video_frame_sums, obs_video_frame_tick,
    obs_video_frame_timing_stats, obs_video_frame_timing_tick,
};
use obs_core::video_frame_timing::{self as timing, Sums, WINDOW_NS};
use proptest::prelude::*;

const FPS25: u64 = 40_000_000;
const FPS30: u64 = 33_333_333;
const FPS60: u64 = 16_666_667;
const NTSC: u64 = 33_366_700;
const T0: u64 = 1_000_000_000_000;

// ---------------------------------------------------------------------------
// one call on all three sides

fn check_tick(cur: u64, last: u64, interval: u64) {
    let shim = obs_video_frame_timing_tick(cur, last, interval);
    // SAFETY: plain values in, a struct out; the oracle keeps no state.
    let c = unsafe { oracle_obs_video_frame_timing_tick(cur, last, interval) };
    let core = timing::tick(cur, last, interval);
    let what = format!("cur {cur} last {last} interval {interval}");
    assert_eq!(shim.delta_ns, c.delta_ns, "shim vs C delta: {what}");
    assert_eq!(
        shim.seconds.to_bits(),
        c.seconds.to_bits(),
        "shim vs C seconds: {what}"
    );
    assert_eq!(shim.delta_ns, core.delta_ns, "shim vs core delta: {what}");
    assert_eq!(
        shim.seconds.to_bits(),
        core.seconds.to_bits(),
        "shim vs core seconds: {what}"
    );
}

fn sums(frame_time_total: u64, fps_total: u64, frames: u32) -> obs_video_frame_sums {
    obs_video_frame_sums {
        frame_time_total_ns: frame_time_total,
        fps_total_ns: fps_total,
        fps_total_frames: frames,
    }
}

fn same(
    a: obs_video_frame_stats,
    sums: obs_video_frame_sums,
    report: bool,
    fps: f64,
    avg: u64,
) -> bool {
    a.sums == sums
        && a.report == report
        && a.fps.to_bits() == fps.to_bits()
        && a.avg_frame_time_ns == avg
}

fn check_stats(
    s: obs_video_frame_sums,
    frame_time: u64,
    video_time: u64,
    last_time: u64,
) -> obs_video_frame_stats {
    let shim = obs_video_frame_timing_stats(s, frame_time, video_time, last_time);
    let core = timing::stats(Sums::from(s), frame_time, video_time, last_time);
    let what = format!("{s:?} frame {frame_time} video {video_time} last {last_time}");
    assert!(
        same(
            shim,
            core.sums.into(),
            core.report,
            core.fps,
            core.avg_frame_time_ns
        ),
        "shim vs core: {what}: {shim:?} vs {core:?}"
    );
    let reported_frames = s.fps_total_frames.wrapping_add(1);
    let fps_total = s
        .fps_total_ns
        .wrapping_add(video_time.wrapping_sub(last_time));
    if fps_total >= WINDOW_NS && reported_frames == 0 {
        // The C divides by zero here; the core's 0 is the intentional
        // difference.
        assert_eq!(shim.avg_frame_time_ns, 0);
        return shim;
    }
    let os = OracleSums {
        frame_time_total_ns: s.frame_time_total_ns,
        fps_total_ns: s.fps_total_ns,
        fps_total_frames: s.fps_total_frames,
    };
    // SAFETY: plain values in, a struct out; the oracle keeps no state.
    let c = unsafe { oracle_obs_video_frame_timing_stats(os, frame_time, video_time, last_time) };
    let c_sums = sums(
        c.sums.frame_time_total_ns,
        c.sums.fps_total_ns,
        c.sums.fps_total_frames,
    );
    assert!(
        same(shim, c_sums, c.report, c.fps, c.avg_frame_time_ns),
        "shim vs C: {what}: {shim:?} vs {c:?}"
    );
    shim
}

// ---------------------------------------------------------------------------
// layout

fn layout(name: &str) -> usize {
    let which = LAYOUT_NAMES
        .iter()
        .position(|n| *n == name)
        .unwrap_or_else(|| panic!("no layout entry {name}"));
    // SAFETY: an index into the oracle's own table.
    unsafe { oracle_video_frame_timing_layout(which) }
}

#[test]
fn parity_layout() {
    type Tick = obs_video_frame_tick;
    type S = obs_video_frame_sums;
    type St = obs_video_frame_stats;
    let rust = [
        size_of::<Tick>(),
        align_of::<Tick>(),
        offset_of!(Tick, delta_ns),
        offset_of!(Tick, seconds),
        size_of::<S>(),
        align_of::<S>(),
        offset_of!(S, frame_time_total_ns),
        offset_of!(S, fps_total_ns),
        offset_of!(S, fps_total_frames),
        size_of::<St>(),
        align_of::<St>(),
        offset_of!(St, sums),
        offset_of!(St, report),
        offset_of!(St, fps),
        offset_of!(St, avg_frame_time_ns),
    ];
    for (name, value) in LAYOUT_NAMES.iter().zip(rust) {
        assert_eq!(value, layout(name), "{name}");
    }
    // SAFETY: past the end of the table; the oracle answers (size_t)-1.
    assert_eq!(
        unsafe { oracle_video_frame_timing_layout(LAYOUT_NAMES.len()) },
        usize::MAX
    );
}

// ---------------------------------------------------------------------------
// exhaustive edges

const TIMES: [u64; 10] = [
    0,
    1,
    FPS25,
    T0,
    T0 + FPS25,
    (1 << 63) - 1,
    1 << 63,
    u64::MAX - 1,
    u64::MAX,
    T0 - 1,
];

const INTERVALS: [u64; 9] = [
    0,
    1,
    FPS60,
    NTSC,
    FPS30,
    FPS25,
    1_000_000_000,
    1 << 63,
    u64::MAX,
];

#[test]
fn parity_tick_exhaustive() {
    for &cur in &TIMES {
        for &last in &TIMES {
            for &interval in &INTERVALS {
                check_tick(cur, last, interval);
            }
        }
    }
}

#[test]
fn parity_stats_window_edges() {
    let fps_totals = [
        0,
        1,
        WINDOW_NS - FPS25 - 1,
        WINDOW_NS - FPS25,
        WINDOW_NS - 1,
        WINDOW_NS,
        u64::MAX - 1,
        u64::MAX,
    ];
    let frame_totals = [0, 1, 123_456_789, u64::MAX - 2, u64::MAX];
    let frames = [0, 1, 24, 29, 59, u32::MAX - 1];
    let steps = [
        (T0, T0),
        (T0 + FPS25, T0),
        (T0 + NTSC, T0),
        (T0 + 3 * FPS60, T0),
        (T0 - 1, T0),
        (0, u64::MAX),
    ];
    for &ft in &fps_totals {
        for &tt in &frame_totals {
            for &n in &frames {
                for &(video, last) in &steps {
                    for render in [0, 1, 5_000_000, u64::MAX] {
                        check_stats(sums(tt, ft, n), render, video, last);
                    }
                }
            }
        }
    }
}

/// Frame by frame for 10 s at four frame rates, with a late frame now and
/// then: every report and the sums in between agree.
#[test]
fn parity_stats_frame_run() {
    for interval in [FPS25, NTSC, FPS30, FPS60] {
        let mut s = sums(0, 0, 0);
        let mut time = T0;
        let mut reports = 0;
        for i in 0..(10 * WINDOW_NS / interval) {
            let step = if i % 37 == 36 { 3 * interval } else { interval };
            let out = check_stats(s, 2_000_000 + i % 1000, time + step, time);
            check_tick(time + step, time, interval);
            time += step;
            s = out.sums;
            reports += u32::from(out.report);
        }
        assert!(reports >= 9, "{interval}: {reports} reports");
    }
}

// ---------------------------------------------------------------------------
// full range

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8192))]

    #[test]
    fn parity_tick_full_range(cur in any::<u64>(), last in prop_oneof![Just(0_u64), any::<u64>()], interval in any::<u64>()) {
        check_tick(cur, last, interval);
    }

    #[test]
    fn parity_stats_full_range(
        frame_total in any::<u64>(),
        fps_total in prop_oneof![0..WINDOW_NS, any::<u64>()],
        frames in any::<u32>(),
        render in any::<u64>(),
        video in any::<u64>(),
        last in any::<u64>(),
    ) {
        check_stats(sums(frame_total, fps_total, frames), render, video, last);
    }

    #[test]
    fn parity_stats_realistic(
        frame_total in 0..10_000_000_000_u64,
        fps_total in 0..2 * WINDOW_NS,
        frames in 0..200_u32,
        render in 0..50_000_000_u64,
        last in 1..u64::MAX / 2,
        step in 0..200_000_000_u64,
    ) {
        check_stats(sums(frame_total, fps_total, frames), render, last + step, last);
    }
}
