//! Tier 2 layout and Tier 3 parity: the shims of `obs_core::ffi::video_pacing`
//! (and the safe core under them) against the original
//! `libobs/obs-video-pacing.c.h`, compiled as `oracle_obs_video_pacing_*`
//! (obs-c-oracle, oracle/video_pacing.c).
//!
//! Every call runs the shim, the C and the safe core, and compares the
//! results exactly: the new frame time and the `int` count of the step, the
//! bool of the group start.
//!
//! Runs: the layout of `struct obs_video_pacing_step`; exhaustive over the
//! sleep result, frame times and clock values at 0, 1, a frame time, and the
//! `u64` and `i64` edges, against intervals at 1, common frame rates and the
//! `u64` edges; the step around every whole multiple of the interval from 0
//! to 20,000 frames late; the count edges around `INT_MAX`, `UINT_MAX` and
//! 2^32; the group start over the `uint32_t` and `size_t` edges; and
//! proptests over full-range frame times, intervals, clock values and group
//! counts.
//!
//! The C is undefined for an interval of 0 after a late wake-up (it divides
//! by zero). Those inputs are not given to the C; there the shim is compared
//! with the core, which gives a count of 0. That is the one intentional
//! difference, unreachable (libobs computes the interval from the frame
//! rate). `test_video_pacing_steps` runs the C test against the C; the
//! real-libobs video and encoder tests run against the hybrid libobs, where
//! these shims replace the C.

use core::ffi::c_int;
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::video_pacing::{
    LAYOUT_NAMES, OraclePacingStep, oracle_obs_video_pacing_group_starts,
    oracle_obs_video_pacing_step, oracle_video_pacing_layout,
};
use obs_core::ffi::video_pacing::{
    obs_video_pacing_group_starts, obs_video_pacing_step, obs_video_pacing_step as PacingStep,
};
use obs_core::video_pacing as pacing;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// one call on all three sides

fn check_step(frame_time: u64, interval: u64, slept: bool, now: u64) {
    let shim = obs_video_pacing_step(frame_time, interval, slept, now);
    let core = pacing::step(frame_time, interval, slept, now);
    assert_eq!(
        (shim.frame_time, shim.count),
        (core.frame_time, core.count),
        "shim vs core: frame time {frame_time} interval {interval} slept {slept} now {now}"
    );
    if interval == 0 && !slept {
        // The C divides by zero here; the core's count of 0 is the
        // intentional difference.
        assert_eq!(core.count, 0);
        assert_eq!(core.frame_time, frame_time);
        return;
    }
    // SAFETY: plain values in, a struct out; the oracle keeps no state.
    let c: OraclePacingStep =
        unsafe { oracle_obs_video_pacing_step(frame_time, interval, slept, now) };
    assert_eq!(
        (shim.frame_time, shim.count),
        (c.frame_time, c.count),
        "shim vs C: frame time {frame_time} interval {interval} slept {slept} now {now}"
    );
}

fn check_group(started: u32, encoders: usize, start_timestamp: u64) {
    let shim = obs_video_pacing_group_starts(started, encoders, start_timestamp);
    // SAFETY: plain values in, a bool out; the oracle keeps no state.
    let c = unsafe { oracle_obs_video_pacing_group_starts(started, encoders, start_timestamp) };
    let core = pacing::group_starts(started, encoders, start_timestamp);
    assert_eq!(
        (shim, shim),
        (c, core),
        "started {started} encoders {encoders} start {start_timestamp}"
    );
}

// ---------------------------------------------------------------------------
// layout

fn layout(name: &str) -> usize {
    let which = LAYOUT_NAMES
        .iter()
        .position(|n| *n == name)
        .unwrap_or_else(|| panic!("no layout entry {name}"));
    // SAFETY: an index into the oracle's own table.
    unsafe { oracle_video_pacing_layout(which) }
}

#[test]
fn parity_layout() {
    assert_eq!(
        size_of::<PacingStep>(),
        layout("sizeof(struct obs_video_pacing_step)")
    );
    assert_eq!(
        align_of::<PacingStep>(),
        layout("alignof(struct obs_video_pacing_step)")
    );
    assert_eq!(
        offset_of!(PacingStep, frame_time),
        layout("offsetof(struct obs_video_pacing_step, frame_time)")
    );
    assert_eq!(
        offset_of!(PacingStep, count),
        layout("offsetof(struct obs_video_pacing_step, count)")
    );
    // SAFETY: past the end of the table; the oracle answers (size_t)-1.
    assert_eq!(
        unsafe { oracle_video_pacing_layout(LAYOUT_NAMES.len()) },
        usize::MAX
    );
}

// ---------------------------------------------------------------------------
// exhaustive edges

const FRAME_TIMES: [u64; 9] = [
    0,
    1,
    1_000_000_000_000,
    (1 << 63) - 1,
    1 << 63,
    (1 << 63) + 1,
    u64::MAX - 1,
    u64::MAX,
    40_000_000,
];

const INTERVALS: [u64; 11] = [
    0,
    1,
    2,
    16_666_667,
    33_333_333,
    40_000_000,
    1_000_000_000,
    (1 << 63) - 1,
    1 << 63,
    u64::MAX - 1,
    u64::MAX,
];

fn offsets(interval: u64) -> Vec<u64> {
    let mut out = vec![
        0,
        1,
        2,
        (1 << 63) - 1,
        1 << 63,
        (1 << 63) + 1,
        u64::MAX - 1,
        u64::MAX,
    ];
    for k in [1_u64, 2, 3, 10, 1000] {
        let base = interval.wrapping_mul(k);
        out.extend([base.wrapping_sub(1), base, base.wrapping_add(1)]);
    }
    out
}

#[test]
fn parity_step_exhaustive() {
    for &frame_time in &FRAME_TIMES {
        for &interval in &INTERVALS {
            for offset in offsets(interval) {
                let now = frame_time.wrapping_add(offset);
                check_step(frame_time, interval, true, now);
                check_step(frame_time, interval, false, now);
            }
        }
    }
}

/// Around every whole multiple of a 25 fps and a 30 fps interval, up to
/// 20,000 frames late.
#[test]
fn parity_step_late_range() {
    let frame_time = 1_000_000_000_000_u64;
    for interval in [33_333_333_u64, 40_000_000] {
        for k in 0..=20_000_u64 {
            let base = frame_time + interval * k;
            for now in [base - 1, base, base + 1, base + interval / 2] {
                check_step(frame_time, interval, false, now);
            }
        }
    }
}

/// The quotient around the `int` limits: `INT_MAX`, 2^31, `UINT_MAX`, 2^32
/// and beyond, where the C's `(int)` cast truncates.
#[test]
fn parity_step_count_edges() {
    let quotients: [u64; 10] = [
        0x7fff_fffe,
        0x7fff_ffff,
        0x8000_0000,
        0x8000_0001,
        0xffff_fffe,
        0xffff_ffff,
        0x1_0000_0000,
        0x1_0000_0001,
        0x1_8000_0000,
        0x7fff_ffff_ffff,
    ];
    for interval in [1_u64, 2, 3, 40_000_000] {
        for q in quotients {
            let Some(diff) = q.checked_mul(interval) else {
                continue;
            };
            for now in [diff.wrapping_sub(1), diff, diff.wrapping_add(1)] {
                check_step(0, interval, false, now);
                check_step(12_345, interval, false, now.wrapping_add(12_345));
            }
        }
    }
}

#[test]
fn parity_group_exhaustive() {
    let started = [0_u32, 1, 2, 3, u32::MAX - 1, u32::MAX];
    let encoders = [
        0_usize,
        1,
        2,
        3,
        usize::try_from(u32::MAX).unwrap_or(usize::MAX),
        usize::MAX - 1,
        usize::MAX,
    ];
    let stamps = [0_u64, 1, 1_000_000_000_000, u64::MAX];
    for &s in &started {
        for &e in &encoders {
            for &t in &stamps {
                check_group(s, e, t);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// full range

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8192))]

    #[test]
    fn parity_step_full_range(
        frame_time in any::<u64>(),
        interval in prop_oneof![1..=u64::MAX, 1..=2_000_000_000_u64, Just(0_u64)],
        slept in any::<bool>(),
        now in any::<u64>(),
    ) {
        check_step(frame_time, interval, slept, now);
    }

    #[test]
    fn parity_step_late_near(
        frame_time in any::<u64>(),
        interval in 1..=2_000_000_000_u64,
        late in 0..=u64::from(u32::MAX) * 4,
    ) {
        check_step(frame_time, interval, false, frame_time.wrapping_add(late));
    }

    #[test]
    fn parity_group_full_range(
        started in any::<u32>(),
        encoders in any::<usize>(),
        start_timestamp in prop_oneof![Just(0_u64), any::<u64>()],
    ) {
        check_group(started, encoders, start_timestamp);
    }
}

/// The count type: an `int`, so the truncation above is the C's.
#[test]
fn parity_count_is_int() {
    assert_eq!(size_of::<c_int>(), 4);
}
