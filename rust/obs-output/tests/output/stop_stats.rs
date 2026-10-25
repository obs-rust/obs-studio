//! Tier 1: `obs_output::stop_stats`, the stop statistics of
//! `log_frame_info()`.
//!
//! The `steps_*` cases port `test/cmocka/test_output_stop_stats_steps.c`
//! 1:1: its 10 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones included.
//! The C calls the hidden `obs_output_stop_stats()`; here the safe core is
//! called directly.
//!
//! The `edge_*` cases are new: the `total - dropped` subtraction where the
//! C's `int` overflows (the port wraps), every combination of edge counters
//! and plugin values against a model of the original C, the percentages
//! against the C's expressions over small counts, and a proptest over
//! full-range values.

use obs_output::stop_stats::{Counts, Line, Stats, stats};
use proptest::prelude::*;

fn run(
    total_frames: u32,
    lagged_frames: u32,
    starting_drawn: u32,
    starting_lagged: u32,
    dropped: i32,
    total: i32,
) -> Stats {
    stats(Counts {
        total_frames,
        lagged_frames,
        starting_drawn,
        starting_lagged,
        dropped,
        total,
    })
}

/// The percentages compared bit for bit.
#[track_caller]
fn assert_pct(got: f64, want: f64) {
    assert_eq!(
        got.to_bits(),
        want.to_bits(),
        "percentage {got:e}, want {want:e}"
    );
}

/// The text `log_frame_info()` prints for a percentage (`"%0.1f"`): Rust's
/// `{:.1}` rounds the exact binary value half to even, as glibc's `printf`
/// does.
#[track_caller]
fn assert_pct_text(pct: f64, want: &str) {
    assert_eq!(format!("{pct:.1}"), want);
}

#[track_caller]
fn assert_plain(s: &Stats) {
    assert!(!s.total_line.is_attempted());
    assert!(!s.drawn_line.is_attempted());
    assert!(!s.lag_line);
    assert!(!s.drop_line);
}

/// The original C's arithmetic, step by step, with `total - dropped`
/// wrapping where the C overflows.
fn model(c: Counts) -> Stats {
    let drawn = c.total_frames.wrapping_sub(c.starting_drawn);
    let lagged = c.lagged_frames.wrapping_sub(c.starting_lagged);
    let mut percentage_lagged = 0.0f32 as f64;
    let mut percentage_dropped = 0.0f32 as f64;
    if drawn != 0 {
        percentage_lagged = lagged as f64 / drawn as f64 * 100.0;
    }
    if c.dropped != 0 {
        percentage_dropped = c.dropped as f64 / c.total as f64 * 100.0;
    }
    let total_line = if c.dropped == 0 || c.total == 0 {
        Line::Plain(c.total)
    } else {
        Line::Attempted {
            done: c.total.wrapping_sub(c.dropped),
            attempted: c.total,
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
        drop_line: c.total != 0 && c.dropped != 0,
    }
}

#[track_caller]
fn assert_same(got: Stats, want: Stats) {
    assert_eq!(got.drawn, want.drawn);
    assert_eq!(got.lagged, want.lagged);
    assert_pct(got.percentage_lagged, want.percentage_lagged);
    assert_pct(got.percentage_dropped, want.percentage_dropped);
    assert_eq!(got.total_line, want.total_line);
    assert_eq!(got.drawn_line, want.drawn_line);
    assert_eq!(got.lag_line, want.lag_line);
    assert_eq!(got.drop_line, want.drop_line);
}

// --------------------------------------------------------------------------
// test_output_stop_stats_steps.c

#[test]
fn steps_zero_counters() {
    let s = run(0, 0, 0, 0, 0, 0);
    assert_eq!(s.drawn, 0);
    assert_eq!(s.lagged, 0);
    assert_eq!(s.total_line, Line::Plain(0));
    assert_eq!(s.drawn_line, Line::Plain(0));
    assert_plain(&s);
    assert_pct(s.percentage_lagged, 0.0);
    assert_pct(s.percentage_dropped, 0.0);

    // the same counters now and at the start
    let s = run(5000, 7, 5000, 7, 0, 0);
    assert_eq!(s.drawn, 0);
    assert_eq!(s.lagged, 0);
    assert_plain(&s);
}

#[test]
fn steps_typical_stop() {
    let s = run(1000, 50, 100, 10, 30, 600);
    assert_eq!(s.drawn, 900);
    assert_eq!(s.lagged, 40);
    assert_eq!(
        s.total_line,
        Line::Attempted {
            done: 570,
            attempted: 600
        }
    );
    assert_eq!(
        s.drawn_line,
        Line::Attempted {
            done: 860,
            attempted: 900
        }
    );
    assert!(s.lag_line);
    assert!(s.drop_line);
    assert_pct(s.percentage_lagged, 40.0 / 900.0 * 100.0);
    assert_pct(s.percentage_dropped, 30.0 / 600.0 * 100.0);
    assert_pct_text(s.percentage_lagged, "4.4");
    assert_pct_text(s.percentage_dropped, "5.0");

    // no lag and no drops: plain lines only
    let s = run(1000, 50, 100, 50, 0, 900);
    assert_eq!(s.drawn, 900);
    assert_eq!(s.drawn_line, Line::Plain(900));
    assert_eq!(s.total_line, Line::Plain(900));
    assert_plain(&s);
    assert_pct(s.percentage_lagged, 0.0);
    assert_pct(s.percentage_dropped, 0.0);
}

#[test]
fn steps_counters_wrapped() {
    let s = run(5, 2, u32::MAX - 15, u32::MAX - 1, 0, 21);
    assert_eq!(s.drawn, 21);
    assert_eq!(s.lagged, 4);
    assert_eq!(
        s.drawn_line,
        Line::Attempted {
            done: 17,
            attempted: 21
        }
    );
    assert!(s.lag_line);
    assert_pct(s.percentage_lagged, 4.0 / 21.0 * 100.0);

    // one past: the whole range
    let s = run(0, 0, 1, 1, 0, 0);
    assert_eq!(s.drawn, u32::MAX);
    assert_eq!(s.lagged, u32::MAX);
    // lagged equal to drawn: 0 drawn on time, 100%
    assert_eq!(
        s.drawn_line,
        Line::Attempted {
            done: 0,
            attempted: u32::MAX
        }
    );
    assert_pct(s.percentage_lagged, 100.0);
    assert_pct_text(s.percentage_lagged, "100.0");

    let s = run(u32::MAX, u32::MAX, 0, 0, 0, 0);
    assert_eq!(s.drawn, u32::MAX);
    assert_eq!(s.lagged, u32::MAX);
}

#[test]
fn steps_lagged_without_drawn() {
    let s = run(100, 13, 100, 10, 0, 0);
    assert_eq!(s.drawn, 0);
    assert_eq!(s.lagged, 3);
    assert_eq!(s.drawn_line, Line::Plain(0));
    assert!(!s.lag_line);
    assert_pct(s.percentage_lagged, 0.0);
}

/// Characterized, not endorsed: drawn - lagged wraps and the percentage is
/// over 100.
#[test]
fn steps_lagged_above_drawn() {
    let s = run(10, 30, 0, 0, 0, 0);
    assert_eq!(s.drawn, 10);
    assert_eq!(s.lagged, 30);
    assert_eq!(
        s.drawn_line,
        Line::Attempted {
            done: u32::MAX - 19,
            attempted: 10
        }
    );
    assert!(s.lag_line);
    assert_pct(s.percentage_lagged, 300.0);
}

#[test]
fn steps_dropped_values() {
    // none dropped: plain line with total
    let s = run(0, 0, 0, 0, 0, 600);
    assert_eq!(s.total_line, Line::Plain(600));
    assert!(!s.drop_line);
    assert_pct(s.percentage_dropped, 0.0);

    // all dropped: 0 output, 100%
    let s = run(0, 0, 0, 0, 600, 600);
    assert_eq!(
        s.total_line,
        Line::Attempted {
            done: 0,
            attempted: 600
        }
    );
    assert!(s.drop_line);
    assert_pct(s.percentage_dropped, 100.0);

    // more dropped than attempted: a negative "Total frames output" and more
    // than 100% (characterized, not endorsed)
    let s = run(0, 0, 0, 0, 700, 600);
    assert_eq!(
        s.total_line,
        Line::Attempted {
            done: -100,
            attempted: 600
        }
    );
    assert!(s.drop_line);
    assert_pct(s.percentage_dropped, 700.0 / 600.0 * 100.0);
    assert_pct_text(s.percentage_dropped, "116.7");

    // a negative count from the plugin: more output than attempted and a
    // negative percentage (characterized, not endorsed)
    let s = run(0, 0, 0, 0, -5, 100);
    assert_eq!(
        s.total_line,
        Line::Attempted {
            done: 105,
            attempted: 100
        }
    );
    assert!(s.drop_line);
    assert_pct(s.percentage_dropped, -5.0);
    assert_pct_text(s.percentage_dropped, "-5.0");
}

#[test]
fn steps_dropped_without_total() {
    let s = run(0, 0, 0, 0, 3, 0);
    assert_eq!(s.total_line, Line::Plain(0));
    assert!(!s.drop_line);
    assert!(s.percentage_dropped.is_infinite());
    assert!(s.percentage_dropped > 0.0);

    let s = run(0, 0, 0, 0, -3, 0);
    assert!(!s.drop_line);
    assert!(s.percentage_dropped.is_infinite());
    assert!(s.percentage_dropped < 0.0);
}

#[test]
fn steps_int_extremes() {
    let attempted = |done, attempted| Line::Attempted { done, attempted };

    let s = run(0, 0, 0, 0, i32::MAX, i32::MAX);
    assert_eq!(s.total_line, attempted(0, i32::MAX));
    assert_pct(s.percentage_dropped, 100.0);

    let s = run(0, 0, 0, 0, i32::MAX, 1);
    assert_eq!(s.total_line, attempted(1 - i32::MAX, 1));
    assert_pct(s.percentage_dropped, f64::from(i32::MAX) / 1.0 * 100.0);

    let s = run(0, 0, 0, 0, i32::MIN, -1);
    assert_eq!(s.total_line, attempted(i32::MAX, -1));
    assert_pct(s.percentage_dropped, f64::from(i32::MIN) / -1.0 * 100.0);

    let s = run(0, 0, 0, 0, i32::MIN, i32::MIN);
    assert_eq!(s.total_line, attempted(0, i32::MIN));
    assert_pct(s.percentage_dropped, 100.0);

    let s = run(0, 0, 0, 0, -1, i32::MAX - 1);
    assert_eq!(s.total_line, attempted(i32::MAX, i32::MAX - 1));

    let s = run(0, 0, 0, 0, 1, i32::MIN + 1);
    assert_eq!(s.total_line, attempted(i32::MIN, i32::MIN + 1));
    assert_pct(s.percentage_dropped, 1.0 / f64::from(i32::MIN + 1) * 100.0);

    // INT_MIN dropped with total 0: the plain line, no subtraction
    let s = run(0, 0, 0, 0, i32::MIN, 0);
    assert_eq!(s.total_line, Line::Plain(0));
    assert!(!s.drop_line);
    assert!(s.percentage_dropped.is_infinite());

    // total INT_MIN (the output's counter wrapped) with none dropped
    let s = run(0, 0, 0, 0, 0, i32::MIN);
    assert_eq!(s.total_line, Line::Plain(i32::MIN));
    assert!(!s.drop_line);
}

#[test]
fn steps_lines_independent() {
    let ints = [i32::MIN, -2, -1, 0, 1, 2, i32::MAX];
    let u32s = [0, 1, 2, u32::MAX - 1, u32::MAX];
    for dropped in ints {
        for total in ints {
            let attempted = dropped != 0 && total != 0;
            let diff = match total.checked_sub(dropped) {
                Some(diff) => diff,
                None if attempted => continue, // undefined in the C
                None => 0,
            };
            for drawn in u32s {
                for lagged in u32s {
                    let s = run(drawn, lagged, 0, 0, dropped, total);
                    assert_eq!(s.drawn, drawn);
                    assert_eq!(s.lagged, lagged);
                    assert_eq!(s.total_line.is_attempted(), attempted);
                    assert_eq!(s.drop_line, attempted);
                    assert_eq!(s.total_line.first(), if attempted { diff } else { total });
                    let both = drawn != 0 && lagged != 0;
                    assert_eq!(s.drawn_line.is_attempted(), both);
                    assert_eq!(s.lag_line, both);
                    assert_eq!(
                        s.drawn_line.first(),
                        if both {
                            drawn.wrapping_sub(lagged)
                        } else {
                            drawn
                        }
                    );
                    assert_pct(
                        s.percentage_lagged,
                        if drawn != 0 {
                            f64::from(lagged) / f64::from(drawn) * 100.0
                        } else {
                            0.0
                        },
                    );
                    assert_pct(
                        s.percentage_dropped,
                        if dropped != 0 {
                            f64::from(dropped) / f64::from(total) * 100.0
                        } else {
                            0.0
                        },
                    );
                }
            }
        }
    }
}

#[test]
fn steps_percentages_exact() {
    let mut differ = 0u32;
    for drawn in 1u32..=200 {
        for lagged in 0u32..=200 {
            let s = run(drawn, lagged, 0, 0, lagged as i32, drawn as i32);
            let want = f64::from(lagged) / f64::from(drawn) * 100.0;
            assert_pct(s.percentage_lagged, want);
            if lagged != 0 {
                assert_pct(s.percentage_dropped, want);
            }
            if want.to_bits() != (f64::from(lagged) * 100.0 / f64::from(drawn)).to_bits() {
                differ += 1;
            }
        }
    }
    // the order matters for some of them
    assert!(differ > 0);

    // 100.0 / 3 would be 33.333333333333336
    let s = run(3, 1, 0, 0, 1, 3);
    assert_pct(s.percentage_lagged, 33.333_333_333_333_33);
    assert_pct_text(s.percentage_lagged, "33.3");
    let s = run(3, 2, 0, 0, 2, 3);
    assert_pct(s.percentage_lagged, 66.666_666_666_666_66);
    assert_pct_text(s.percentage_lagged, "66.7");
    let s = run(1000, 1, 0, 0, 1, 2000);
    assert_pct_text(s.percentage_lagged, "0.1");
    assert_pct_text(s.percentage_dropped, "0.1");
    let s = run(2000, 1, 0, 0, 1, 4000);
    assert_pct_text(s.percentage_lagged, "0.1");
    assert_pct(s.percentage_dropped, 0.025);
    assert_pct_text(s.percentage_dropped, "0.0");

    // the largest counts are exact in a double
    let s = run(u32::MAX, u32::MAX - 1, 0, 0, i32::MAX - 1, i32::MAX);
    assert_pct(
        s.percentage_lagged,
        f64::from(u32::MAX - 1) / f64::from(u32::MAX) * 100.0,
    );
    assert_pct(
        s.percentage_dropped,
        f64::from(i32::MAX - 1) / f64::from(i32::MAX) * 100.0,
    );
    assert_pct_text(s.percentage_lagged, "100.0");
}

// --------------------------------------------------------------------------
// Edge cases

/// Where the C's `int` `total - dropped` overflows (undefined; every
/// compiler OBS supports wraps), the port wraps.
#[test]
fn edge_total_minus_dropped_wraps() {
    let attempted = |done, attempted| Line::Attempted { done, attempted };
    let s = run(0, 0, 0, 0, i32::MIN, 5);
    assert_eq!(s.total_line, attempted(i32::MIN + 5, 5));
    assert!(s.drop_line);
    let s = run(0, 0, 0, 0, i32::MIN, i32::MAX);
    assert_eq!(s.total_line, attempted(-1, i32::MAX));
    let s = run(0, 0, 0, 0, -1, i32::MAX);
    assert_eq!(s.total_line, attempted(i32::MIN, i32::MAX));
    let s = run(0, 0, 0, 0, 1, i32::MIN);
    assert_eq!(s.total_line, attempted(i32::MAX, i32::MIN));
    let s = run(0, 0, 0, 0, i32::MAX, -2);
    assert_eq!(s.total_line, attempted(i32::MAX, -2));
    // the percentage is not affected: it is a double division
    assert_pct(s.percentage_dropped, f64::from(i32::MAX) / -2.0 * 100.0);
}

const EDGE_U32: [u32; 12] = [
    0,
    1,
    2,
    3,
    59,
    60,
    1000,
    0x7fff_ffff,
    0x8000_0000,
    0x8000_0001,
    u32::MAX - 1,
    u32::MAX,
];

const EDGE_I32: [i32; 13] = [
    i32::MIN,
    i32::MIN + 1,
    -1000,
    -2,
    -1,
    0,
    1,
    2,
    60,
    1000,
    i32::MAX - 1,
    i32::MAX,
    0x4000_0000,
];

/// Every combination of edge counters (now and at the start) against the
/// model of the C.
#[test]
fn edge_counters_against_model() {
    for &total_frames in &EDGE_U32 {
        for &starting_drawn in &EDGE_U32 {
            for &lagged_frames in &EDGE_U32 {
                for &starting_lagged in &EDGE_U32 {
                    let c = Counts {
                        total_frames,
                        lagged_frames,
                        starting_drawn,
                        starting_lagged,
                        dropped: 3,
                        total: 600,
                    };
                    assert_same(stats(c), model(c));
                }
            }
        }
    }
}

/// Every pair of edge plugin values against the model of the C.
#[test]
fn edge_dropped_against_model() {
    for &dropped in &EDGE_I32 {
        for &total in &EDGE_I32 {
            for &(now, start) in &[(0u32, 0u32), (100, 40), (5, u32::MAX)] {
                let c = Counts {
                    total_frames: now,
                    lagged_frames: now / 2,
                    starting_drawn: start,
                    starting_lagged: start / 3,
                    dropped,
                    total,
                };
                assert_same(stats(c), model(c));
            }
        }
    }
}

/// `Line::first` and `is_attempted` read the variant.
#[test]
fn edge_line_helpers() {
    assert_eq!(Line::Plain(7).first(), 7);
    assert!(!Line::Plain(7).is_attempted());
    let l = Line::Attempted {
        done: 3,
        attempted: 9,
    };
    assert_eq!(l.first(), 3);
    assert!(l.is_attempted());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// Full-range counters and plugin values against the model of the C.
    #[test]
    fn edge_full_range_against_model(
        total_frames in any::<u32>(),
        lagged_frames in any::<u32>(),
        starting_drawn in any::<u32>(),
        starting_lagged in any::<u32>(),
        dropped in any::<i32>(),
        total in any::<i32>(),
    ) {
        let c = Counts {
            total_frames,
            lagged_frames,
            starting_drawn,
            starting_lagged,
            dropped,
            total,
        };
        assert_same(stats(c), model(c));
    }

    /// Small differences, as a real output has, against the model.
    #[test]
    fn edge_near_start_against_model(
        start in any::<u32>(),
        drawn in 0u32..10_000,
        lagged in 0u32..10_000,
        dropped in -10i32..10_000,
        total in 0i32..10_000,
    ) {
        let c = Counts {
            total_frames: start.wrapping_add(drawn),
            lagged_frames: start.wrapping_add(lagged),
            starting_drawn: start,
            starting_lagged: start,
            dropped,
            total,
        };
        assert_same(stats(c), model(c));
    }
}
