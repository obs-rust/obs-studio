//! Tier 2 layout and Tier 3 parity: the shim of
//! `obs_output::ffi::output_stop_stats` (and the safe core under it) against
//! the original `libobs/obs-output-stop-stats.c.h`, compiled as
//! `oracle_obs_output_stop_stats` (obs-c-oracle, oracle/output_stop_stats.c).
//!
//! Every output is compared exactly, on all three sides (oracle, shim,
//! core): the counts as integers, the flags as bools and the two
//! percentages as `f64` bits. The core's `Line` is the C's first number and
//! its attempted flag; its attempted numbers are the C caller's `total` and
//! `drawn`.
//!
//! Runs: the layout of the struct; every combination of the four video
//! counters (now and at the start) in 0..=7 with every dropped and total
//! in -8..=8 (1,183,744 calls); the four counters at 12 edges each (around
//! 0, 2^31 and `u32::MAX`, so every wrap); every pair of 21 edge `int`
//! values for dropped and total; the percentages over drawn 1..=1024 and
//! lagged 0..=1024; and proptests (20,000 cases each) over full-range
//! counters and plugin values, and over small differences from any start.
//!
//! Nothing traps: the divisions are `double`.
//!
//! Intentional difference: where the C's `int` `total - dropped`
//! overflows (both non-zero), it is undefined in C and wraps in the port
//! (`wrapping_sub`); every compiler OBS supports wraps there. Tier 1 pins
//! the wrap, and here the oracle is not called for those inputs; the shim
//! and the core are still compared with each other and with the wrapped
//! value. Otherwise no input is excluded. `test_output_stop_stats_steps`
//! runs the C test against the C; the real-libobs output tests run against
//! the hybrid libobs, where this shim replaces the C.

use std::ffi::c_int;
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::output_stop_stats::{
    LAYOUT_NAMES, Stats as OracleStats, oracle_obs_output_stop_stats,
    oracle_output_stop_stats_layout,
};
use obs_output::ffi::output_stop_stats::{
    obs_output_stop_stats, obs_output_stop_stats as ShimStats,
};
use obs_output::stop_stats::{self as core, Counts, Line};
use proptest::prelude::*;

#[test]
fn layout_matches_c() {
    let ours = [
        size_of::<ShimStats>(),
        align_of::<ShimStats>(),
        offset_of!(ShimStats, drawn),
        offset_of!(ShimStats, lagged),
        offset_of!(ShimStats, percentage_lagged),
        offset_of!(ShimStats, percentage_dropped),
        offset_of!(ShimStats, frames_output),
        offset_of!(ShimStats, output_attempted),
        offset_of!(ShimStats, frames_drawn),
        offset_of!(ShimStats, drawn_attempted),
        offset_of!(ShimStats, lag_line),
        offset_of!(ShimStats, drop_line),
    ];
    let oracle_side = [
        size_of::<OracleStats>(),
        align_of::<OracleStats>(),
        offset_of!(OracleStats, drawn),
        offset_of!(OracleStats, lagged),
        offset_of!(OracleStats, percentage_lagged),
        offset_of!(OracleStats, percentage_dropped),
        offset_of!(OracleStats, frames_output),
        offset_of!(OracleStats, output_attempted),
        offset_of!(OracleStats, frames_drawn),
        offset_of!(OracleStats, drawn_attempted),
        offset_of!(OracleStats, lag_line),
        offset_of!(OracleStats, drop_line),
    ];
    for (i, name) in LAYOUT_NAMES.iter().enumerate() {
        // SAFETY: an index into a constant table.
        let c = unsafe { oracle_output_stop_stats_layout(i) };
        assert_eq!(ours[i], c, "{name}: shim struct");
        assert_eq!(oracle_side[i], c, "{name}: oracle binding");
    }
    // SAFETY: an index past the table.
    assert_eq!(
        unsafe { oracle_output_stop_stats_layout(LAYOUT_NAMES.len()) },
        usize::MAX
    );
}

/// One side's result as plain values, the percentages as bits.
#[derive(Debug, PartialEq, Eq)]
struct Seen {
    drawn: u32,
    lagged: u32,
    percentage_lagged: u64,
    percentage_dropped: u64,
    frames_output: i32,
    output_attempted: bool,
    frames_drawn: u32,
    drawn_attempted: bool,
    lag_line: bool,
    drop_line: bool,
}

fn from_oracle(s: OracleStats) -> Seen {
    Seen {
        drawn: s.drawn,
        lagged: s.lagged,
        percentage_lagged: s.percentage_lagged.to_bits(),
        percentage_dropped: s.percentage_dropped.to_bits(),
        frames_output: s.frames_output,
        output_attempted: s.output_attempted,
        frames_drawn: s.frames_drawn,
        drawn_attempted: s.drawn_attempted,
        lag_line: s.lag_line,
        drop_line: s.drop_line,
    }
}

fn from_shim(s: ShimStats) -> Seen {
    Seen {
        drawn: s.drawn,
        lagged: s.lagged,
        percentage_lagged: s.percentage_lagged.to_bits(),
        percentage_dropped: s.percentage_dropped.to_bits(),
        frames_output: s.frames_output,
        output_attempted: s.output_attempted,
        frames_drawn: s.frames_drawn,
        drawn_attempted: s.drawn_attempted,
        lag_line: s.lag_line,
        drop_line: s.drop_line,
    }
}

/// The core's result in the C's shape; checks that each attempted line
/// carries the caller's `total` and `drawn`.
fn from_core(c: Counts) -> Seen {
    let s = core::stats(c);
    if let Line::Attempted { attempted, .. } = s.total_line {
        assert_eq!(attempted, c.total, "{c:?}: attempted total");
    }
    if let Line::Attempted { attempted, .. } = s.drawn_line {
        assert_eq!(attempted, s.drawn, "{c:?}: attempted drawn");
    }
    Seen {
        drawn: s.drawn,
        lagged: s.lagged,
        percentage_lagged: s.percentage_lagged.to_bits(),
        percentage_dropped: s.percentage_dropped.to_bits(),
        frames_output: s.total_line.first(),
        output_attempted: s.total_line.is_attempted(),
        frames_drawn: s.drawn_line.first(),
        drawn_attempted: s.drawn_line.is_attempted(),
        lag_line: s.lag_line,
        drop_line: s.drop_line,
    }
}

/// The C's `total - dropped` is evaluated (both non-zero) and overflows.
fn c_undefined(dropped: c_int, total: c_int) -> bool {
    dropped != 0 && total != 0 && total.checked_sub(dropped).is_none()
}

/// Compares the three sides on one input; returns false when the oracle
/// was skipped (the C is undefined there).
fn check(c: Counts) -> bool {
    let shim = from_shim(obs_output_stop_stats(
        c.total_frames,
        c.lagged_frames,
        c.starting_drawn,
        c.starting_lagged,
        c.dropped,
        c.total,
    ));
    let ours = from_core(c);
    assert_eq!(shim, ours, "{c:?}: shim against core");
    if c_undefined(c.dropped, c.total) {
        assert!(shim.output_attempted, "{c:?}");
        assert_eq!(shim.frames_output, c.total.wrapping_sub(c.dropped), "{c:?}");
        return false;
    }
    // SAFETY: plain values; total - dropped is within int (checked above).
    let oracle = from_oracle(unsafe {
        oracle_obs_output_stop_stats(
            c.total_frames,
            c.lagged_frames,
            c.starting_drawn,
            c.starting_lagged,
            c.dropped,
            c.total,
        )
    });
    assert_eq!(oracle, shim, "{c:?}: oracle against shim");
    true
}

#[test]
fn small_ranges_exhaustive() {
    let mut calls = 0u32;
    for total_frames in 0..=7u32 {
        for starting_drawn in 0..=7u32 {
            for lagged_frames in 0..=7u32 {
                for starting_lagged in 0..=7u32 {
                    for dropped in -8..=8 {
                        for total in -8..=8 {
                            assert!(check(Counts {
                                total_frames,
                                lagged_frames,
                                starting_drawn,
                                starting_lagged,
                                dropped,
                                total,
                            }));
                            calls += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(calls, 4096 * 289);
}

const EDGE_U32: [u32; 12] = [
    0,
    1,
    2,
    3,
    1000,
    0x7fff_ffff,
    0x8000_0000,
    0x8000_0001,
    u32::MAX - 2,
    u32::MAX - 1,
    u32::MAX,
    0x1234_5678,
];

const EDGE_I32: [i32; 21] = [
    i32::MIN,
    i32::MIN + 1,
    i32::MIN + 2,
    -0x4000_0000,
    -1000,
    -3,
    -2,
    -1,
    0,
    1,
    2,
    3,
    60,
    600,
    1000,
    0x3fff_ffff,
    0x4000_0000,
    0x4000_0001,
    i32::MAX - 2,
    i32::MAX - 1,
    i32::MAX,
];

#[test]
fn edge_counters_exhaustive() {
    for &total_frames in &EDGE_U32 {
        for &starting_drawn in &EDGE_U32 {
            for &lagged_frames in &EDGE_U32 {
                for &starting_lagged in &EDGE_U32 {
                    for &(dropped, total) in &[(0, 0), (3, 600), (-3, 600), (3, 0)] {
                        assert!(check(Counts {
                            total_frames,
                            lagged_frames,
                            starting_drawn,
                            starting_lagged,
                            dropped,
                            total,
                        }));
                    }
                }
            }
        }
    }
}

#[test]
fn edge_plugin_values_exhaustive() {
    let mut skipped = 0u32;
    for &dropped in &EDGE_I32 {
        for &total in &EDGE_I32 {
            for &(now, start) in &[(0u32, 0u32), (900, 100), (5, u32::MAX - 15)] {
                let c = Counts {
                    total_frames: now,
                    lagged_frames: now / 3,
                    starting_drawn: start,
                    starting_lagged: start / 7,
                    dropped,
                    total,
                };
                if !check(c) {
                    skipped += 1;
                }
            }
        }
    }
    // the overflowing pairs: compared shim against core and the wrap only
    assert!(skipped > 0);
}

#[test]
fn percentages_exhaustive() {
    for drawn in 1..=1024u32 {
        for lagged in 0..=1024u32 {
            assert!(check(Counts {
                total_frames: drawn,
                lagged_frames: lagged,
                starting_drawn: 0,
                starting_lagged: 0,
                dropped: lagged as i32,
                total: drawn as i32,
            }));
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    /// Full-range counters and plugin values, negative and overflowing
    /// ones included.
    #[test]
    fn full_range(
        total_frames in any::<u32>(),
        lagged_frames in any::<u32>(),
        starting_drawn in any::<u32>(),
        starting_lagged in any::<u32>(),
        dropped in any::<i32>(),
        total in any::<i32>(),
    ) {
        check(Counts {
            total_frames,
            lagged_frames,
            starting_drawn,
            starting_lagged,
            dropped,
            total,
        });
    }

    /// Small differences from any start, as a real output has.
    #[test]
    fn near_start(
        start in any::<u32>(),
        lagged_start in any::<u32>(),
        drawn in 0u32..100_000,
        lagged in 0u32..100_000,
        dropped in -10i32..100_000,
        total in 0i32..100_000,
    ) {
        let c = Counts {
            total_frames: start.wrapping_add(drawn),
            lagged_frames: lagged_start.wrapping_add(lagged),
            starting_drawn: start,
            starting_lagged: lagged_start,
            dropped,
            total,
        };
        prop_assert!(check(c), "{:?}: the oracle was skipped", c);
    }
}
