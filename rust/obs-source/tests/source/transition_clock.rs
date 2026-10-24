//! Tier 1: `obs_source::transition_clock`, the clock and fit decisions of
//! the transitions.
//!
//! The `steps_*` cases port `test/cmocka/test_transition_clock_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations, the "characterized, not endorsed" cases included. The
//! manual step runs as `obs_transition_tick()` runs it, with
//! `obs_graphics::math_extra::calc_torquef` (a dev-dependency of the test
//! executable, not of `obs-source`).
//!
//! The `edge_*` cases are new: the clock against a model of the x87
//! division written as a long division (`x87_ratio`), with the nearest
//! `f32` (`nearest_ratio`) and a plain `f64` division (`f64_ratio`) beside
//! it for the ratios where the platforms' C differ; the clock's order in
//! time; near-ties of the rounding; the manual step over any `f32` bit
//! pattern; the size fold; the fit's invariants over any `f32` sizes;
//! `mix_spaces` against a rank model; and the timestamps against a filter
//! model.

use obs_graphics::math_extra::calc_torquef;
use obs_source::transition_clock::{
    CS_709_EXTENDED, CS_709_SCRGB, CS_SRGB, CS_SRGB_16F, Clock, Fit, MODE_MANUAL, SCALE_ASPECT,
    SCALE_MAX_ONLY, SCALE_STRETCH, ScaleType, Step, fit, fold, manual, min_ts, mix_spaces, ratio,
    size, time,
};
use proptest::prelude::*;

const AUTO: i32 = 0;
const MANUAL: i32 = MODE_MANUAL;
const MAX_ONLY: i32 = SCALE_MAX_ONLY;
const ASPECT: i32 = SCALE_ASPECT;
const STRETCH: i32 = SCALE_STRETCH;
const SRGB: i32 = CS_SRGB;
const SRGB_16F: i32 = CS_SRGB_16F;
const EXTENDED: i32 = CS_709_EXTENDED;
const SCRGB: i32 = CS_709_SCRGB;

const MSEC: u64 = 1_000_000;
const TWO_63: u64 = 1 << 63;
const TWO_39: u64 = 1 << 39;
const TWO_53: u64 = 1 << 53;

/// The scale the cmocka test fills before a fit, to see that it is not
/// written.
const UNSET: u32 = 0x7fc0_beef;

#[track_caller]
fn check_bits(actual: f32, expected: f32, what: &str) {
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "{what} is {actual:e} ({:#010x}), expected {expected:e} ({:#010x})",
        actual.to_bits(),
        expected.to_bits()
    );
}

fn clock_time(mode: i32, manual_val: f32, start_time: u64, duration: u64, ts: u64) -> f32 {
    time(
        Clock {
            mode,
            manual_val,
            start_time,
            duration,
        },
        ts,
    )
}

/// `2^exp` as an `f32` (`ldexpf(1.0f, exp)` in the cmocka test).
fn pow2(exp: i32) -> f32 {
    2.0f32.powi(exp)
}

// ---- calc_time() ----

#[test]
fn steps_time_manual_value() {
    check_bits(
        clock_time(MANUAL, 0.25, 1000, 100, 0),
        0.25,
        "manual before the start",
    );
    check_bits(
        clock_time(MANUAL, 0.25, 1000, 100, 1050),
        0.25,
        "manual in the middle",
    );
    check_bits(
        clock_time(MANUAL, 0.25, 1000, 100, u64::MAX),
        0.25,
        "manual after the end",
    );
    check_bits(
        clock_time(MANUAL, 0.75, 0, 0, 5),
        0.75,
        "manual with no duration",
    );

    // characterized, not endorsed: not clamped, and -0 and NaN pass
    // through with their bits
    check_bits(clock_time(MANUAL, -0.0, 0, 0, 5), -0.0, "manual -0");
    check_bits(clock_time(MANUAL, 2.5, 0, 0, 5), 2.5, "manual above 1");
    check_bits(clock_time(MANUAL, -1.0, 0, 0, 5), -1.0, "manual below 0");
    let nan = f32::from_bits(0x7fc0_1234);
    check_bits(clock_time(MANUAL, nan, 0, 0, 5), nan, "manual NaN");
    let neg_inf = f32::from_bits(0xff80_0000);
    check_bits(clock_time(MANUAL, neg_inf, 0, 0, 5), neg_inf, "manual -inf");
}

#[test]
fn steps_time_around_the_start_and_end() {
    let start = 3_600_000 * MSEC;
    let duration = 100;

    check_bits(clock_time(AUTO, 0.5, start, duration, 0), 0.0, "at 0");
    check_bits(
        clock_time(AUTO, 0.5, start, duration, start - 1),
        0.0,
        "start - 1",
    );
    check_bits(clock_time(AUTO, 0.5, start, duration, start), 0.0, "start");
    check_bits(
        clock_time(AUTO, 0.5, start, duration, start + 1),
        0.01,
        "start + 1",
    );
    check_bits(
        clock_time(AUTO, 0.5, start, duration, start + 50),
        0.5,
        "half way",
    );
    check_bits(
        clock_time(AUTO, 0.5, start, duration, start + duration - 1),
        0.99,
        "end - 1",
    );
    check_bits(
        clock_time(AUTO, 0.5, start, duration, start + duration),
        1.0,
        "end",
    );
    check_bits(
        clock_time(AUTO, 0.5, start, duration, start + duration + 1),
        1.0,
        "end + 1",
    );
    check_bits(
        clock_time(AUTO, 0.5, start, duration, u64::MAX),
        1.0,
        "UINT64_MAX",
    );

    check_bits(
        clock_time(AUTO, 0.5, start, 300 * MSEC, start + 100 * MSEC),
        (1.0f64 / 3.0) as f32,
        "1/3",
    );

    check_bits(clock_time(AUTO, 0.5, 0, duration, 0), 0.0, "start 0, ts 0");
    check_bits(clock_time(AUTO, 0.5, 0, duration, 1), 0.01, "start 0, ts 1");

    check_bits(
        clock_time(AUTO, 0.5, u64::MAX - 10, duration, u64::MAX),
        0.1,
        "start UINT64_MAX - 10",
    );
}

#[test]
fn steps_time_zero_duration() {
    let start = 3_600_000 * MSEC;

    check_bits(
        clock_time(AUTO, 0.5, start, 0, start - 1),
        0.0,
        "zero duration, start - 1",
    );
    check_bits(
        clock_time(AUTO, 0.5, start, 0, start),
        0.0,
        "zero duration, start",
    );
    check_bits(
        clock_time(AUTO, 0.5, start, 0, start + 1),
        1.0,
        "zero duration, start + 1",
    );
    check_bits(
        clock_time(AUTO, 0.5, 0, 0, u64::MAX),
        1.0,
        "zero duration, UINT64_MAX",
    );
}

#[test]
fn steps_time_huge_durations() {
    check_bits(clock_time(AUTO, 0.5, 0, TWO_63, 1), pow2(-63), "2^63, ts 1");
    check_bits(
        clock_time(AUTO, 0.5, 0, TWO_63, 1 << 62),
        0.5,
        "2^63, ts 2^62",
    );
    check_bits(
        clock_time(AUTO, 0.5, 0, TWO_63, TWO_63),
        1.0,
        "2^63, at the end",
    );

    // characterized, not endorsed: any ratio of 1 - 2^-25 or more rounds to
    // 1.0f (the tie to even), so t >= 1 a little early
    check_bits(
        clock_time(AUTO, 0.5, 0, TWO_63, TWO_63 - 1),
        1.0,
        "2^63, end - 1",
    );
    check_bits(
        clock_time(AUTO, 0.5, 0, 1 << 26, (1 << 26) - 1),
        1.0,
        "2^26, end - 1",
    );
    check_bits(
        clock_time(AUTO, 0.5, 0, 1 << 26, (1 << 26) - 2),
        1.0,
        "2^26, end - 2 (a tie)",
    );
    check_bits(
        clock_time(AUTO, 0.5, 0, 1 << 26, (1 << 26) - 3),
        1.0 - pow2(-24),
        "2^26, end - 3",
    );
    check_bits(
        clock_time(AUTO, 0.5, 0, 1 << 26, (1 << 26) - 4),
        1.0 - pow2(-24),
        "2^26, end - 4",
    );

    check_bits(
        clock_time(AUTO, 0.5, 0, TWO_63 + 2, 1 << 62),
        0.5,
        "2^63 + 2, ts 2^62",
    );
    check_bits(
        clock_time(AUTO, 0.5, 0, u64::MAX, TWO_63),
        0.5,
        "UINT64_MAX, ts 2^63",
    );
    check_bits(
        clock_time(AUTO, 0.5, 0, u64::MAX, u64::MAX - 1),
        1.0,
        "UINT64_MAX, end - 1",
    );
    check_bits(
        clock_time(AUTO, 0.5, 0, u64::MAX, u64::MAX),
        1.0,
        "UINT64_MAX, at the end",
    );
    check_bits(
        clock_time(AUTO, 0.5, 1, u64::MAX, u64::MAX),
        1.0,
        "UINT64_MAX, start 1, end - 1",
    );

    let longest = u64::from(u32::MAX) * MSEC;
    check_bits(
        clock_time(AUTO, 0.5, 0, longest, longest / 2),
        0.5,
        "UINT32_MAX ms, half way",
    );
    check_bits(
        clock_time(AUTO, 0.5, 0, longest, longest - 1),
        1.0,
        "UINT32_MAX ms, end - 1",
    );
}

#[test]
fn steps_time_other_modes_are_automatic() {
    for mode in [AUTO, 2, -1, i32::MIN, i32::MAX] {
        check_bits(
            clock_time(mode, 0.25, 1000, 100, 1000),
            0.0,
            "other mode, start",
        );
        check_bits(
            clock_time(mode, 0.25, 1000, 100, 1050),
            0.5,
            "other mode, half way",
        );
        check_bits(
            clock_time(mode, 0.25, 1000, 100, 1100),
            1.0,
            "other mode, end",
        );
    }
}

// ---- the manual step of obs_transition_tick() ----

/// `transition_manual_val` after one tick, as `obs_transition_tick()` runs
/// it.
fn manual_step(mode: i32, val: f32, target: f32, torque: f32, clamp: f32, t: f32) -> f32 {
    match manual(mode, torque) {
        Step::Snap => target,
        Step::Step => calc_torquef(val, target, torque, clamp, t),
        Step::Hold => val,
    }
}

#[test]
fn steps_manual_snaps_at_torque_zero() {
    assert_eq!(manual(MANUAL, 0.0), Step::Snap);
    assert_eq!(manual(MANUAL, -0.0), Step::Snap);
    check_bits(manual_step(MANUAL, 0.0, 0.75, 0.0, 0.0, 0.5), 0.75, "snap");
    check_bits(
        manual_step(MANUAL, 0.75, 0.25, -0.0, 0.5, 0.5),
        0.25,
        "snap down at -0",
    );
    check_bits(
        manual_step(MANUAL, 0.0, 2.0, 0.0, 0.0, 0.5),
        2.0,
        "snap above 1",
    );
}

#[test]
fn steps_manual_steps_with_torque() {
    for torque in [1.0, 0.5, -1.0, 1e-30, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(manual(MANUAL, torque), Step::Step, "{torque}");
    }

    check_bits(
        manual_step(MANUAL, 0.0, 1.0, 0.5, 0.0, 0.5),
        0.25,
        "step up",
    );
    check_bits(
        manual_step(MANUAL, 1.0, 0.0, 0.5, 0.0, 0.5),
        0.75,
        "step down",
    );

    check_bits(
        manual_step(MANUAL, 0.0, 1.0, 8.0, 0.0, 1.0),
        1.0,
        "overshoot up",
    );
    check_bits(
        manual_step(MANUAL, 1.0, 0.0, 8.0, 0.0, 1.0),
        0.0,
        "overshoot down",
    );

    check_bits(
        manual_step(MANUAL, 0.5, 0.5 + pow2(-20), 0.5, 0.0, 0.5),
        0.5 + pow2(-20),
        "close",
    );
}

#[test]
fn steps_manual_steps_with_a_clamp() {
    check_bits(
        manual_step(MANUAL, 0.0, 1.0, 0.0625, 0.25, 0.5),
        0.125,
        "clamp up",
    );
    check_bits(
        manual_step(MANUAL, 1.0, 0.0, 0.0625, 0.25, 0.5),
        0.875,
        "clamp down",
    );
    check_bits(
        manual_step(MANUAL, 0.0, 1.0, 0.5, 0.25, 1.0),
        0.5,
        "above the clamp",
    );
    check_bits(
        manual_step(MANUAL, 0.0, 0.0625, 0.0625, 0.25, 1.0),
        0.0625,
        "clamp overshoot",
    );
}

#[test]
fn steps_manual_nan_targets() {
    let nan = f32::from_bits(0x7fc0_0000);

    // characterized, not endorsed: a NaN target snaps to NaN, and steps to
    // NaN with a torque; a NaN torque steps to NaN; a NaN value only
    // recovers by a snap
    check_bits(
        manual_step(MANUAL, 0.5, nan, 0.0, 0.25, 0.5),
        nan,
        "NaN target, snap",
    );
    assert!(manual_step(MANUAL, 0.5, nan, 1.0, 0.25, 0.5).is_nan());
    assert!(manual_step(MANUAL, 0.5, nan, 1.0, 0.0, 0.5).is_nan());
    assert!(manual_step(MANUAL, 0.5, nan, 1.0, 0.25, 0.0).is_nan());

    assert_eq!(manual(MANUAL, nan), Step::Step);
    assert!(manual_step(MANUAL, 0.0, 1.0, nan, 0.25, 0.5).is_nan());
    assert!(manual_step(MANUAL, 1.0, 0.0, nan, 0.25, 0.5).is_nan());

    assert!(manual_step(MANUAL, nan, 1.0, 1.0, 0.25, 0.5).is_nan());
    check_bits(
        manual_step(MANUAL, nan, 1.0, 0.0, 0.25, 0.5),
        1.0,
        "NaN value, snap",
    );
}

#[test]
fn steps_manual_only_in_manual_mode() {
    for mode in [AUTO, 2, -1, i32::MIN, i32::MAX] {
        for torque in [0.0, 1.0, f32::from_bits(0x7fc0_0000)] {
            assert_eq!(manual(mode, torque), Step::Hold, "{mode} {torque}");
            check_bits(manual_step(mode, 0.5, 1.0, torque, 0.25, 0.5), 0.5, "hold");
        }
    }
}

// ---- get_cx(), get_cy() and the size fold ----

#[test]
fn steps_size_set_or_actual() {
    assert_eq!(size(1920, 1280), 1920);
    assert_eq!(size(640, 1280), 640);
    assert_eq!(size(0, 1280), 1280);
    assert_eq!(size(0, 0), 0);
    assert_eq!(size(1, 0), 1);
    assert_eq!(size(u32::MAX, 5), u32::MAX);
    assert_eq!(size(0, u32::MAX), u32::MAX);
}

#[test]
fn steps_fold_larger_child() {
    assert_eq!(fold(fold(0, 1280), 1920), 1920);
    assert_eq!(fold(fold(0, 1920), 1280), 1920);
    assert_eq!(fold(0, 0), 0);
    assert_eq!(fold(1920, 1920), 1920);
    assert_eq!(fold(1920, 0), 1920);
    assert_eq!(fold(0, u32::MAX), u32::MAX);
    assert_eq!(fold(u32::MAX, 1), u32::MAX);
}

// ---- the fit ----

/// The scale as the cmocka test sees it: filled with [`UNSET`], then
/// written only where the fit writes it.
fn fit_of(
    tr_cx: f32,
    tr_cy: f32,
    source_cx: f32,
    source_cy: f32,
    scale_type: i32,
) -> (bool, f32, f32) {
    let unset = f32::from_bits(UNSET);
    match fit(tr_cx, tr_cy, source_cx, source_cy, scale_type) {
        Fit::NoMatrix => (false, unset, unset),
        Fit::Scale { x, y } => (true, x, y),
        Fit::Unset => (true, unset, unset),
    }
}

#[track_caller]
fn check_fit(got: (bool, f32, f32), x: f32, y: f32) {
    assert!(got.0, "no matrix, expected a scale of {x} by {y}");
    check_bits(got.1, x, "scale x");
    check_bits(got.2, y, "scale y");
}

#[track_caller]
fn check_no_matrix(got: (bool, f32, f32)) {
    assert!(!got.0, "a matrix, expected none");
    check_bits(got.1, f32::from_bits(UNSET), "scale x");
    check_bits(got.2, f32::from_bits(UNSET), "scale y");
}

#[test]
fn steps_fit_max_only() {
    check_fit(fit_of(1920.0, 1080.0, 1280.0, 720.0, MAX_ONLY), 1.0, 1.0);
    check_fit(fit_of(1920.0, 1080.0, 1920.0, 1080.0, MAX_ONLY), 1.0, 1.0);
    check_fit(fit_of(1920.0, 1080.0, 100.0, 1080.0, MAX_ONLY), 1.0, 1.0);

    check_fit(fit_of(1920.0, 1080.0, 3840.0, 2160.0, MAX_ONLY), 0.5, 0.5);
    check_fit(fit_of(1920.0, 1080.0, 2000.0, 500.0, MAX_ONLY), 0.96, 0.96);
    check_fit(fit_of(1920.0, 1080.0, 1000.0, 2160.0, MAX_ONLY), 0.5, 0.5);
    check_fit(
        fit_of(1920.0, 1080.0, 1921.0, 1080.0, MAX_ONLY),
        1920.0 / 1921.0,
        1920.0 / 1921.0,
    );
    check_fit(
        fit_of(1920.0, 1080.0, 1920.0, 1081.0, MAX_ONLY),
        1080.0 / 1081.0,
        1080.0 / 1081.0,
    );
}

#[test]
fn steps_fit_aspect() {
    check_fit(fit_of(1920.0, 1080.0, 960.0, 540.0, ASPECT), 2.0, 2.0);
    check_fit(fit_of(1920.0, 1080.0, 1920.0, 1080.0, ASPECT), 1.0, 1.0);
    check_fit(fit_of(1920.0, 1080.0, 3840.0, 2160.0, ASPECT), 0.5, 0.5);

    check_fit(fit_of(1920.0, 1080.0, 2400.0, 900.0, ASPECT), 0.8, 0.8);
    check_fit(
        fit_of(1920.0, 1080.0, 1080.0, 1920.0, ASPECT),
        0.5625,
        0.5625,
    );
    check_fit(fit_of(1920.0, 1080.0, 640.0, 480.0, ASPECT), 2.25, 2.25);

    check_fit(fit_of(1920.0, 1080.0, 1280.0, 720.0, ASPECT), 1.5, 1.5);
}

#[test]
fn steps_fit_stretch() {
    check_fit(fit_of(1920.0, 1080.0, 960.0, 720.0, STRETCH), 2.0, 1.5);
    check_fit(fit_of(1920.0, 1080.0, 1920.0, 1080.0, STRETCH), 1.0, 1.0);
    check_fit(fit_of(1920.0, 1080.0, 3840.0, 1080.0, STRETCH), 0.5, 1.0);
    check_fit(fit_of(1920.0, 1080.0, 3840.0, 4320.0, STRETCH), 0.5, 0.25);
}

#[test]
fn steps_fit_zero_transition_size() {
    // characterized, not endorsed: tr_cx / tr_cy is NaN or inf, so the
    // height decides, or 0, so the width decides
    for scale_type in [MAX_ONLY, ASPECT] {
        check_fit(fit_of(0.0, 0.0, 1280.0, 720.0, scale_type), 0.0, 0.0);
        check_fit(fit_of(0.0, 1080.0, 1280.0, 720.0, scale_type), 0.0, 0.0);
        check_fit(fit_of(1920.0, 0.0, 1280.0, 720.0, scale_type), 0.0, 0.0);
    }
    check_fit(fit_of(0.0, 0.0, 1280.0, 720.0, STRETCH), 0.0, 0.0);
    check_fit(fit_of(0.0, 1080.0, 1280.0, 720.0, STRETCH), 0.0, 1.5);
    check_fit(fit_of(1920.0, 0.0, 1280.0, 720.0, STRETCH), 1.5, 0.0);
}

#[test]
fn steps_fit_zero_child_size() {
    for scale_type in [MAX_ONLY, ASPECT, STRETCH, 3, -1] {
        check_no_matrix(fit_of(1920.0, 1080.0, 0.0, 1080.0, scale_type));
        check_no_matrix(fit_of(1920.0, 1080.0, 1920.0, 0.0, scale_type));
        check_no_matrix(fit_of(1920.0, 1080.0, 0.0, 0.0, scale_type));
        check_no_matrix(fit_of(0.0, 0.0, 0.0, 0.0, scale_type));
        check_no_matrix(fit_of(1920.0, 1080.0, -0.0, 1080.0, scale_type));
    }
}

#[test]
fn steps_fit_other_scale_types() {
    // characterized, not endorsed: a matrix, but no scale written
    let unset = f32::from_bits(UNSET);
    for scale_type in [3, -1, 100, i32::MIN, i32::MAX] {
        check_fit(
            fit_of(1920.0, 1080.0, 1280.0, 720.0, scale_type),
            unset,
            unset,
        );
        check_fit(
            fit_of(1920.0, 1080.0, 3840.0, 2160.0, scale_type),
            unset,
            unset,
        );
    }
}

// ---- mix_spaces() ----

#[test]
fn steps_mix_spaces_every_pair() {
    let spaces = [SRGB, SRGB_16F, EXTENDED, SCRGB];
    let expected = [
        [SRGB, SRGB_16F, EXTENDED, EXTENDED],
        [SRGB_16F, SRGB_16F, EXTENDED, EXTENDED],
        [EXTENDED, EXTENDED, EXTENDED, EXTENDED],
        [EXTENDED, EXTENDED, EXTENDED, EXTENDED],
    ];
    for (a, row) in spaces.iter().zip(expected) {
        for (b, want) in spaces.iter().zip(row) {
            assert_eq!(mix_spaces(*a, *b), want, "mix_spaces({a}, {b})");
        }
    }

    assert_eq!(mix_spaces(-1, 4), SRGB);
    assert_eq!(mix_spaces(4, SRGB_16F), SRGB_16F);
    assert_eq!(mix_spaces(i32::MIN, SCRGB), EXTENDED);
    assert_eq!(mix_spaces(i32::MAX, i32::MAX), SRGB);
}

// ---- transition_calc_min_ts() ----

#[test]
fn steps_min_ts_none_one_two() {
    assert_eq!(min_ts([0, 0]), 0);

    assert_eq!(min_ts([5000, 0]), 5000);
    assert_eq!(min_ts([0, 5000]), 5000);
    assert_eq!(min_ts([u64::MAX, 0]), u64::MAX);
    assert_eq!(min_ts([0, 1]), 1);

    assert_eq!(min_ts([3000, 5000]), 3000);
    assert_eq!(min_ts([5000, 3000]), 3000);
    assert_eq!(min_ts([5000, 5000]), 5000);
    assert_eq!(min_ts([u64::MAX, 1]), 1);
    assert_eq!(min_ts([1, u64::MAX]), 1);
    assert_eq!(min_ts([u64::MAX, u64::MAX - 1]), u64::MAX - 1);
}

// ---- edge cases ----

/// The `f32` nearest `p / q` (ties to even), from integers alone, for
/// `0 < p < q`: the 24-bit quotient, rounded once on its remainder. This is
/// what the C gives where `long double` has 113 bits (aarch64 Linux: the
/// two roundings cannot differ from one for a `u64` ratio).
pub fn nearest_ratio(p: u64, q: u64) -> f32 {
    assert!(0 < p && p < q);
    let (wide_p, wide_q) = (u128::from(p), u128::from(q));
    // k with 2^23 <= (p << k) / q < 2^24
    let mut k = 23 + p.leading_zeros() - q.leading_zeros();
    while (wide_p << k) / wide_q < 1 << 23 {
        k += 1;
    }
    while (wide_p << k) / wide_q >= 1 << 24 {
        k -= 1;
    }
    let mut m = (wide_p << k) / wide_q;
    let r = (wide_p << k) % wide_q;
    if 2 * r > wide_q || (2 * r == wide_q && m % 2 == 1) {
        m += 1;
    }
    if m == 1 << 24 {
        m = 1 << 23;
        k -= 1;
    }
    // m < 2^24 and 2^-k (24 <= k <= 87) are exact, and so is their product
    let mantissa = u32::try_from(m).unwrap();
    mantissa as f32 * pow2(-i32::try_from(k).unwrap())
}

/// `(float)((long double)p / (long double)q)` on x86_64 (x87), written by
/// long division one bit at a time: 64 significant bits of the quotient,
/// rounded to nearest even on the remainder, then that 64-bit value rounded
/// to 24 bits, to nearest even. `0 < p < q`.
pub fn x87_ratio(p: u64, q: u64) -> f32 {
    assert!(0 < p && p < q);
    let q = u128::from(q);
    let mut rem = u128::from(p);
    // the quotient is 1.xxx times 2^exp
    let mut exp = 0i32;
    while rem < q {
        rem <<= 1;
        exp -= 1;
    }
    let mut sig = 0u128;
    for _ in 0..64 {
        sig <<= 1;
        if rem >= q {
            sig |= 1;
            rem -= q;
        }
        rem <<= 1;
    }
    // rem is twice the remainder: above q is past half a unit
    if rem > q || (rem == q && sig & 1 == 1) {
        sig += 1;
    }
    if sig == 1 << 64 {
        sig = 1 << 63;
        exp += 1;
    }
    // the 64-bit value to 24 bits
    let (mut top, low) = (sig >> 40, sig & ((1 << 40) - 1));
    if low > 1 << 39 || (low == 1 << 39 && top & 1 == 1) {
        top += 1;
    }
    if top == 1 << 24 {
        top = 1 << 23;
        exp += 1;
    }
    let mantissa = u32::try_from(top - (1 << 23)).unwrap();
    let biased = u32::try_from(exp + 127).unwrap();
    f32::from_bits(biased << 23 | mantissa)
}

/// `(float)((double)p / (double)q)`: the C where `long double` is a
/// `double` (MSVC, Apple silicon).
pub fn f64_ratio(p: u64, q: u64) -> f32 {
    (p as f64 / q as f64) as f32
}

/// Ratios where the platforms' C give different floats: (p, q, x87, the
/// 64-bit `double`, the nearest). The first two have durations of about
/// 33.5 s: a `double` lands on a float rounding tie and rounds it to even,
/// the x87 and the nearest do not. The other four are above 2^39: the x87
/// lands on a tie, the nearest does not.
pub const PLATFORM_TIES: [(u64, u64, u32, u32, u32); 6] = [
    (
        33_554_426_999,
        33_554_431_999,
        0x3f7f_fffd,
        0x3f7f_fffe,
        0x3f7f_fffd,
    ),
    (
        33_554_370_999,
        33_554_431_999,
        0x3f7f_ffe1,
        0x3f7f_ffe2,
        0x3f7f_ffe1,
    ),
    (
        1_782_156_411_927_942_798,
        12_155_074_924_612_969_154,
        0x3e16_231c,
        0x3e16_231c,
        0x3e16_231b,
    ),
    (
        1_891_072_130_013_493_079,
        4_385_917_186_926_150_621,
        0x3edc_c234,
        0x3edc_c234,
        0x3edc_c233,
    ),
    (
        2_553_438_365_880_568_314,
        16_791_520_477_553_770_470,
        0x3e1b_b77c,
        0x3e1b_b77c,
        0x3e1b_b77d,
    ),
    (
        7_780_729_102_897_824_847,
        14_413_708_920_487_964_683,
        0x3f0a_3148,
        0x3f0a_3148,
        0x3f0a_3149,
    ),
];

/// A full-range `u64`, with values near 0, 2^39, 2^53, 2^63 and `u64::MAX`
/// drawn often.
fn any_u64() -> impl Strategy<Value = u64> {
    prop_oneof![
        3 => any::<u64>(),
        // log-uniform: any bit length
        2 => (any::<u64>(), 0u32..64).prop_map(|(v, shift)| v >> shift),
        1 => prop::sample::select(vec![
            0u64,
            1,
            2,
            TWO_39 - 1,
            TWO_39,
            TWO_53 - 1,
            TWO_53,
            TWO_53 + 1,
            TWO_63 - 1,
            TWO_63,
            TWO_63 + 1,
            u64::MAX - 1,
            u64::MAX,
        ]),
    ]
}

/// `0 < p < q` with `q` of any bit length from 2 to 64, half the time with
/// `p` near an `f32` rounding tie of `p / q`.
fn any_ratio() -> impl Strategy<Value = (u64, u64)> {
    (
        any::<u64>(),
        0u32..63,
        any::<u64>(),
        any::<bool>(),
        (1u64 << 23)..(1 << 24),
        0u32..40,
        -2i64..=2,
    )
        .prop_map(|(q, shift, p_seed, near_tie, m, e, d)| {
            let q = (q >> shift).max(2);
            let p = if near_tie {
                // (2m + 1) / 2^(25 + e) is a tie in the binade below 2^-e
                let tie = (u128::from(2 * m + 1) * u128::from(q)) >> (25 + e);
                u64::try_from(tie).unwrap().saturating_add_signed(d)
            } else {
                p_seed
            };
            (1 + p % (q - 1), q)
        })
}

/// Any `f32` bit pattern, with ±0, ±inf, NaNs and the slider's ends often.
fn any_f32() -> impl Strategy<Value = f32> {
    prop_oneof![
        3 => any::<u32>().prop_map(f32::from_bits),
        1 => prop::sample::select(vec![
            0.0f32,
            -0.0,
            1.0,
            -1.0,
            0.5,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
            -f32::NAN,
            f32::from_bits(0x7f80_0001),
            f32::from_bits(1),
            f32::MAX,
        ]),
        // in [0, 1], uniformly over the bits
        2 => (0u32..=0x3f80_0000).prop_map(f32::from_bits),
    ]
}

/// A size as libobs passes it (a `u32` as `f32`, often a video size), or
/// any `f32` bit pattern.
fn any_size() -> impl Strategy<Value = f32> {
    prop_oneof![
        2 => any::<u32>().prop_map(|v| v as f32),
        2 => prop::sample::select(vec![
            0.0f32, 1.0, 2.0, 640.0, 720.0, 1080.0, 1280.0, 1920.0, 2160.0, 3840.0,
        ]),
        2 => (0u32..=8192).prop_map(|v| v as f32),
        1 => any_f32(),
    ]
}

fn any_mode() -> impl Strategy<Value = i32> {
    prop_oneof![
        2 => Just(MANUAL),
        2 => Just(AUTO),
        1 => any::<i32>(),
    ]
}

fn any_scale_type() -> impl Strategy<Value = i32> {
    prop_oneof![
        4 => 0i32..3,
        1 => any::<i32>(),
    ]
}

#[test]
fn edge_ratio_models() {
    // the models themselves, on ratios whose floats are known
    for (p, q, want) in [
        (1, 2, 0.5f32),
        (1, 100, 0.01),
        (99, 100, 0.99),
        (1, 3, (1.0f64 / 3.0) as f32),
        (1, u64::MAX, pow2(-64)),
        (u64::MAX - 1, u64::MAX, 1.0),
        // ties to even: (2^24 + 1) / 2^25 is half way between 2^-1 and the
        // next f32 up; 2^24 + 3 is half way above an odd mantissa
        ((1 << 24) + 1, 1 << 25, 0.5),
        ((1 << 24) + 3, 1 << 25, f32::from_bits(0.5f32.to_bits() + 2)),
    ] {
        check_bits(nearest_ratio(p, q), want, "nearest");
        check_bits(x87_ratio(p, q), want, "x87");
        check_bits(f64_ratio(p, q), want, "f64");
    }
    // where the platforms differ, each model gives its platform's float
    for (p, q, x87, double, nearest) in PLATFORM_TIES {
        check_bits(x87_ratio(p, q), f32::from_bits(x87), "x87 tie");
        check_bits(f64_ratio(p, q), f32::from_bits(double), "f64 tie");
        check_bits(nearest_ratio(p, q), f32::from_bits(nearest), "nearest tie");
    }
}

/// The ratio is the x87's where the platforms differ: a 33.5 s transition
/// where a plain `f64` division would round the other way, and four ratios
/// above 2^39 where the x87 is not the nearest.
#[test]
fn edge_ratio_platform_ties() {
    for (p, q, x87, _, _) in PLATFORM_TIES {
        check_bits(ratio(p, q), f32::from_bits(x87), "platform tie");
    }
}

/// Near-ties of the rounding: ratios within a few units of 2^-60 of an
/// `f32` tie, where a double rounding through a too-narrow format goes the
/// wrong way. The `f64` division does for some of them, below 2^53.
#[test]
fn edge_ratio_near_ties() {
    let mut checked = 0usize;
    let mut f64_differs = 0usize;
    for c in [1u64, 3, 5, 7, 1000, 1_000_000, 123_456_789] {
        for m in [
            (1u64 << 23) + 1,
            (1 << 23) + 2,
            (1 << 24) - 3,
            12_345_677,
            16_000_001,
        ] {
            let (p, q) = ((2 * m + 1) * c, (1u64 << 25) * c);
            for (dp, dq) in [
                (0i64, 0i64),
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (-1, -1),
            ] {
                let p = p.checked_add_signed(dp).unwrap();
                let q = q.checked_add_signed(dq).unwrap();
                if p < q {
                    check_bits(ratio(p, q), x87_ratio(p, q), "near-tie");
                    if f64_ratio(p, q).to_bits() != ratio(p, q).to_bits() {
                        f64_differs += 1;
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 200, "{checked}");
    assert!(f64_differs > 0, "no near-tie where f64 differs");
}

/// Every ratio with a duration up to 2^10 ns.
#[test]
fn edge_ratio_every_small_duration() {
    for q in 2u64..=1 << 10 {
        for p in 1..q {
            check_bits(ratio(p, q), x87_ratio(p, q), "small");
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    /// The ratio is the x87's for any `u64` ratio.
    #[test]
    fn edge_ratio_x87_model((p, q) in any_ratio()) {
        check_bits(ratio(p, q), x87_ratio(p, q), "x87");
    }

    /// Below 2^39 the x87 cannot land on a tie, so the ratio is the
    /// nearest.
    #[test]
    fn edge_ratio_nearest_below_2_39(
        q in (2u64..TWO_39, 0u32..38).prop_map(|(q, shift)| (q >> shift).max(2)),
        p_seed in any::<u64>(),
    ) {
        let p = 1 + p_seed % (q - 1);
        check_bits(ratio(p, q), nearest_ratio(p, q), "below 2^39");
    }

    /// Where `long double` is a `double` the C is never more than one
    /// `f32` unit from the x87.
    #[test]
    fn edge_ratio_f64_within_one_unit((p, q) in any_ratio()) {
        let got = ratio(p, q).to_bits();
        let double = f64_ratio(p, q).to_bits();
        prop_assert!(got.abs_diff(double) <= 1, "{p}/{q}: {got:#x} vs {double:#x}");
    }

    /// The clock against the C's rule written with the x87 model.
    #[test]
    fn edge_time_model(
        mode in any_mode(),
        manual_val in any_f32(),
        start_time in any_u64(),
        duration in any_u64(),
        offset in any_u64(),
        before in any::<bool>(),
    ) {
        // half the time near the start, either side of it
        let ts = if before {
            start_time.saturating_sub(offset)
        } else {
            start_time.saturating_add(offset)
        };
        let got = clock_time(mode, manual_val, start_time, duration, ts);
        let want = if mode == MANUAL {
            manual_val
        } else if ts <= start_time {
            0.0
        } else {
            let elapsed = ts - start_time;
            if duration == 0 || elapsed >= duration {
                1.0
            } else {
                x87_ratio(elapsed, duration)
            }
        };
        prop_assert_eq!(got.to_bits(), want.to_bits());
        if mode != MANUAL {
            prop_assert!((0.0..=1.0).contains(&got));
        }
    }

    /// The automatic clock never goes back in time.
    #[test]
    fn edge_time_in_order(
        start_time in any_u64(),
        duration in any_u64(),
        a in any_u64(),
        b in any_u64(),
    ) {
        let (early, late) = (a.min(b), a.max(b));
        let t0 = clock_time(AUTO, 0.0, start_time, duration, early);
        let t1 = clock_time(AUTO, 0.0, start_time, duration, late);
        prop_assert!(t0 <= t1, "{t0} at {early} > {t1} at {late}");
    }

    /// Snap only in manual mode with a torque of ±0, step for any other
    /// torque (NaN included), hold in any other mode.
    #[test]
    fn edge_manual_any_torque(mode in any_mode(), torque_bits in any::<u32>()) {
        let torque = f32::from_bits(torque_bits);
        let want = if mode != MANUAL {
            Step::Hold
        } else if torque_bits & 0x7fff_ffff == 0 {
            Step::Snap
        } else {
            Step::Step
        };
        prop_assert_eq!(manual(mode, torque), want);
    }

    #[test]
    fn edge_size_and_fold(set in any::<u32>(), actual in any::<u32>()) {
        prop_assert_eq!(size(set, actual), if set == 0 { actual } else { set });
        prop_assert_eq!(fold(set, actual), set.max(actual));
    }

    /// The fit's invariants over any sizes: no matrix exactly for a child
    /// side of ±0; a type outside the enum writes nothing; stretch is each
    /// side's ratio; aspect is one of the two ratios on both sides;
    /// max-only is 1 for a child that is not larger (a NaN side is never
    /// larger), and aspect otherwise.
    #[test]
    fn edge_fit_invariants(
        tr_cx in any_size(),
        tr_cy in any_size(),
        source_cx in any_size(),
        source_cy in any_size(),
        scale_type in any_scale_type(),
    ) {
        let got = fit(tr_cx, tr_cy, source_cx, source_cy, scale_type);
        if source_cx == 0.0 || source_cy == 0.0 {
            prop_assert_eq!(got, Fit::NoMatrix);
            return Ok(());
        }
        let width = tr_cx / source_cx;
        let height = tr_cy / source_cy;
        let same = |a: f32, b: f32| a.to_bits() == b.to_bits();
        match (ScaleType::from_c(scale_type), got) {
            (None, Fit::Unset) => {}
            (Some(ScaleType::Stretch), Fit::Scale { x, y }) => {
                prop_assert!(same(x, width) && same(y, height));
            }
            (Some(ScaleType::Aspect), Fit::Scale { x, y }) => {
                prop_assert!(same(x, y));
                prop_assert!(same(x, width) || same(x, height));
            }
            (Some(ScaleType::MaxOnly), Fit::Scale { x, y }) => {
                if source_cx > tr_cx || source_cy > tr_cy {
                    let Fit::Scale { x: ax, y: ay } =
                        fit(tr_cx, tr_cy, source_cx, source_cy, ASPECT)
                    else {
                        panic!("aspect gave no scale");
                    };
                    prop_assert!(same(x, ax) && same(y, ay));
                } else {
                    prop_assert!(same(x, 1.0) && same(y, 1.0));
                }
            }
            (scale_type, got) => {
                prop_assert!(false, "{scale_type:?} gave {got:?}");
            }
        }
    }

    /// For whole-number sizes (exact in `f32`), aspect takes the smaller of
    /// the two side ratios, so the child fits inside the transition, unless
    /// the two aspects round equal (then the height decides).
    #[test]
    fn edge_fit_aspect_smaller_ratio(
        tr_cx in 1u32..=8192,
        tr_cy in 1u32..=8192,
        source_cx in 1u32..=16384,
        source_cy in 1u32..=16384,
    ) {
        let (tr_cx, tr_cy) = (tr_cx as f32, tr_cy as f32);
        let (source_cx, source_cy) = (source_cx as f32, source_cy as f32);
        let Fit::Scale { x, .. } = fit(tr_cx, tr_cy, source_cx, source_cy, ASPECT) else {
            panic!("no scale");
        };
        let width = tr_cx / source_cx;
        let height = tr_cy / source_cy;
        if tr_cx / tr_cy != source_cx / source_cy {
            prop_assert_eq!(x.to_bits(), width.min(height).to_bits());
        }
    }

    /// `mix_spaces` against a rank model: the higher of the two ranks
    /// (EXTENDED and SCRGB 2, SRGB_16F 1, anything else 0).
    #[test]
    fn edge_mix_spaces_rank_model(
        a in prop_oneof![-2i32..6, any::<i32>()],
        b in prop_oneof![-2i32..6, any::<i32>()],
    ) {
        let rank = |s: i32| match s {
            CS_709_EXTENDED | CS_709_SCRGB => 2,
            CS_SRGB_16F => 1,
            _ => 0,
        };
        let want = [SRGB, SRGB_16F, EXTENDED][rank(a).max(rank(b))];
        prop_assert_eq!(mix_spaces(a, b), want);
        prop_assert_eq!(mix_spaces(b, a), want);
    }

    /// The earliest timestamp that is not 0, written as a filter.
    #[test]
    fn edge_min_ts_filter_model(
        a in prop_oneof![Just(0u64), any_u64()],
        b in prop_oneof![Just(0u64), any_u64()],
    ) {
        let want = [a, b].into_iter().filter(|&ts| ts != 0).min().unwrap_or(0);
        prop_assert_eq!(min_ts([a, b]), want);
        prop_assert_eq!(min_ts([b, a]), want);
    }
}
