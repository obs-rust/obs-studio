//! obs-output: the stop statistics of `log_frame_info()`
//! (`libobs/obs-output-stop-stats.c.h`). It runs once each time an output
//! with video stops, so it is nowhere near a hot path; the bench shows the
//! shim costs what the C does. Each side runs 1024 calls over fixed inputs
//! (counters around a start, some wrapped, dropped counts from none to all
//! and past them), nothing is allocated, and the results fold into a
//! checksum both must match.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::output_stop_stats::oracle_obs_output_stop_stats;
use obs_output::ffi::output_stop_stats::obs_output_stop_stats;
use std::hint::black_box;

const N: u32 = 1024;

/// Stop `i`: the video counters now and at the start, the plugin's dropped
/// count and the output's total.
fn stop(i: u32) -> (u32, u32, u32, u32, i32, i32) {
    let start = i.wrapping_mul(0x9e37_79b9);
    let drawn = i * 60;
    let lagged = i % 17;
    let total = i32::try_from(drawn).unwrap_or(i32::MAX);
    let dropped = i32::try_from(i % 23).unwrap_or(0) * 3 - 6;
    (
        start.wrapping_add(drawn),
        start.wrapping_add(lagged),
        start,
        start,
        dropped,
        total,
    )
}

fn fold(
    acc: u64,
    drawn: u32,
    frames_output: i32,
    pct_lagged: f64,
    pct_dropped: f64,
    lines: [bool; 4],
) -> u64 {
    lines.iter().fold(
        acc.wrapping_add(u64::from(drawn))
            .wrapping_add(frames_output as u64)
            .wrapping_add(pct_lagged.to_bits())
            .wrapping_add(pct_dropped.to_bits()),
        |a, &l| a.wrapping_mul(3).wrapping_add(u64::from(l)),
    )
}

fn rust() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (t, l, sd, sl, d, total) = black_box(stop(i));
        let s = obs_output_stop_stats(t, l, sd, sl, d, total);
        fold(
            acc,
            s.drawn,
            s.frames_output,
            s.percentage_lagged,
            s.percentage_dropped,
            [
                s.output_attempted,
                s.drawn_attempted,
                s.lag_line,
                s.drop_line,
            ],
        )
    })
}

fn c() -> u64 {
    (0..N).fold(0, |acc, i| {
        let (t, l, sd, sl, d, total) = black_box(stop(i));
        // SAFETY: plain values; total - dropped stays small.
        let s = unsafe { oracle_obs_output_stop_stats(t, l, sd, sl, d, total) };
        fold(
            acc,
            s.drawn,
            s.frames_output,
            s.percentage_lagged,
            s.percentage_dropped,
            [
                s.output_attempted,
                s.drawn_attempted,
                s.lag_line,
                s.drop_line,
            ],
        )
    })
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "output_stop_stats/log_frame_info/1024", rust, c);
}
