//! Tier 3: the `obs_source_transition_clock_*` shims against the original C
//! (`obs-c-oracle`, `oracle/source_transition_clock.c`: the unmodified
//! `libobs/obs-source-transition-clock.c.h` with its functions renamed).
//!
//! Every function takes plain values; the fit writes its scale through two
//! `float *`, which both sides get pre-filled with the same guard value, so
//! a run compares the return value and both floats bit for bit, written or
//! not.
//!
//! Runs:
//! - the constants, the step values and the C's `long double` width
//!   (`LDBL_MANT_DIG`) against the C header;
//! - the clock over every mode (both of the enum and four outside it) and
//!   every combination of special manual values, start times, durations
//!   and timestamps around the start and the end; the six ratios where
//!   the platforms' C differ; sweeps of ratios a few units from an `f32`
//!   rounding tie in 40 binades for durations from 0.5 s to past 2^63 ns,
//!   with the count of those a plain `f64` division would round the other
//!   way; every ratio with a duration up to 2^11 ns; and proptests over
//!   full-range `u64` clocks;
//! - the manual step over every mode and special torques, and the step as
//!   `obs_transition_tick()` runs it (the C's decision with the C's
//!   `calc_torquef()`, the shim's with the Rust one), with special values,
//!   targets and clamps; the size and the fold over special values;
//! - the fit over every scale type of the enum and five outside it with
//!   every combination of 12 special sizes for each of the four sides, and
//!   proptests over sizes drawn from `u32` sizes and `f32` bit patterns
//!   (never a wide `Range<f32>`, #1951);
//! - `mix_spaces` over every pair of 12 values (the enum, around it, the
//!   ends of `int`), and the timestamps over every pair of special values;
//!   proptests over any `i32` and `u64`.
//!
//! Bit parity holds for every input where the C's `long double` is the
//! x87's 80 bits (x86_64 Linux and macOS, where the oracle is checked).
//! Where the oracle's `long double` has another width, the clock's ratio
//! can differ from the shim's in the last bit for a quotient within about
//! 2^-54 of an `f32` rounding tie (a `double`, MSVC and Apple silicon) or
//! from 2^39 ns on (IEEE quad, aarch64 Linux), as those platforms' libobs
//! already differ from x86_64's; there the run checks the C against a model
//! of its width and the shim against the x87 model (both in Tier 1), and
//! everything else bit for bit.
//!
//! One exception, in the manual tick only: `calc_torquef()` is not part of
//! this port (each side calls its own, the oracle's C and `obs-graphics`'s
//! port), and where one of its operations has two NaN operands, which NaN
//! comes out depends on the operand order the compiler picks, as in #2021.
//! There both sides must give a NaN. The decision itself, and every other
//! result, is compared bit for bit.
//!
//! Intentional difference, checked on the shim only: a null `scale_x` or
//! `scale_y` is not written (the C writes through it).

use std::ffi::c_int;

use obs_c_oracle::graphics_math::oracle_calc_torquef;
use obs_c_oracle::source_transition_clock::{self as oracle, LAYOUT_NAMES, LDBL_MANT_DIG_INDEX};
use obs_graphics::math_extra::calc_torquef;
use obs_source::ffi::source_transition_clock::{
    self as shim, OBS_SOURCE_TRANSITION_CLOCK_HOLD, OBS_SOURCE_TRANSITION_CLOCK_SNAP,
    OBS_SOURCE_TRANSITION_CLOCK_STEP, obs_source_transition_clock_step,
};
use obs_source::transition_clock::{
    CS_709_EXTENDED, CS_709_SCRGB, CS_SRGB, CS_SRGB_16F, MODE_MANUAL, SCALE_ASPECT, SCALE_MAX_ONLY,
    SCALE_STRETCH,
};
use proptest::prelude::*;

use super::transition_clock::{PLATFORM_TIES, f64_ratio, nearest_ratio, x87_ratio};

const TWO_39: u64 = 1 << 39;
const TWO_53: u64 = 1 << 53;
const TWO_63: u64 = 1 << 63;
const MSEC: u64 = 1_000_000;

/// The guard the fit's scale is pre-filled with on both sides.
const GUARD: u32 = 0x7fc0_beef;

#[test]
fn layout_matches_the_c_header() {
    let rust = [
        usize::try_from(MODE_MANUAL).unwrap(),
        usize::try_from(SCALE_MAX_ONLY).unwrap(),
        usize::try_from(SCALE_ASPECT).unwrap(),
        usize::try_from(SCALE_STRETCH).unwrap(),
        usize::try_from(CS_SRGB).unwrap(),
        usize::try_from(CS_SRGB_16F).unwrap(),
        usize::try_from(CS_709_EXTENDED).unwrap(),
        usize::try_from(CS_709_SCRGB).unwrap(),
        size_of::<obs_source_transition_clock_step>(),
        OBS_SOURCE_TRANSITION_CLOCK_HOLD as usize,
        OBS_SOURCE_TRANSITION_CLOCK_SNAP as usize,
        OBS_SOURCE_TRANSITION_CLOCK_STEP as usize,
        size_of::<c_int>(),
        size_of::<bool>(),
        size_of::<f32>(),
    ];
    for (which, (name, value)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_source_transition_clock_layout(which) };
        assert_eq!(value, c, "{name}");
    }
    // The C evaluates float as float (FLT_EVAL_METHOD 0), as Rust does.
    // SAFETY: a pure lookup.
    assert_eq!(
        unsafe { oracle::oracle_source_transition_clock_layout(16) },
        0,
        "FLT_EVAL_METHOD"
    );
    let width = ldbl_mant_dig();
    assert!([53, 64, 113].contains(&width), "LDBL_MANT_DIG {width}");
    #[cfg(all(target_arch = "x86_64", not(target_env = "msvc")))]
    assert_eq!(width, 64, "x86_64 long double is the x87's");
}

// --------------------------------------------------------------------------
// One call on each side

/// `LDBL_MANT_DIG` of the oracle's C.
fn ldbl_mant_dig() -> usize {
    // SAFETY: a pure lookup.
    unsafe { oracle::oracle_source_transition_clock_layout(LDBL_MANT_DIG_INDEX) }
}

/// The ratio as the oracle's C rounds it, from the model of its width.
fn c_width_ratio(p: u64, q: u64) -> f32 {
    match ldbl_mant_dig() {
        64 => x87_ratio(p, q),
        53 => f64_ratio(p, q),
        _ => nearest_ratio(p, q),
    }
}

#[derive(Debug, Clone, Copy)]
struct Clock {
    mode: i32,
    manual_val: f32,
    start_time: u64,
    duration: u64,
}

fn c_time(c: Clock, ts: u64) -> f32 {
    // SAFETY: plain values.
    unsafe {
        oracle::oracle_obs_source_transition_clock_time(
            c.mode,
            c.manual_val,
            c.start_time,
            c.duration,
            ts,
        )
    }
}

fn r_time(c: Clock, ts: u64) -> f32 {
    shim::obs_source_transition_clock_time(c.mode, c.manual_val, c.start_time, c.duration, ts)
}

/// The two sides' clocks, bit for bit. Where the oracle's `long double` is
/// not the x87's, a ratio that differs must be the C's width's and the
/// shim's x87's (see the module header).
#[track_caller]
fn check_time(clock: Clock, ts: u64) -> f32 {
    let c = c_time(clock, ts);
    let r = r_time(clock, ts);
    if c.to_bits() != r.to_bits() {
        let elapsed = ts.wrapping_sub(clock.start_time);
        let in_ratio =
            clock.mode != MODE_MANUAL && ts > clock.start_time && elapsed < clock.duration;
        assert!(
            ldbl_mant_dig() != 64 && in_ratio,
            "time of {clock:?} at {ts}: C {c:e} ({:#010x}), Rust {r:e} ({:#010x})",
            c.to_bits(),
            r.to_bits()
        );
        assert_eq!(
            c.to_bits(),
            c_width_ratio(elapsed, clock.duration).to_bits()
        );
        assert_eq!(r.to_bits(), x87_ratio(elapsed, clock.duration).to_bits());
    }
    r
}

#[track_caller]
fn check_ratio(p: u64, q: u64) -> f32 {
    check_time(
        Clock {
            mode: 0,
            manual_val: 0.0,
            start_time: 0,
            duration: q,
        },
        p,
    )
}

#[track_caller]
fn check_manual(mode: i32, torque: f32) -> u32 {
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_transition_clock_manual(mode, torque) };
    let r = shim::obs_source_transition_clock_manual(mode, torque);
    assert_eq!(
        r,
        c,
        "manual({mode}, {torque:e} {:#010x})",
        torque.to_bits()
    );
    r
}

/// One tick of `obs_transition_tick()`'s manual step on each side: the
/// decision, then the target or `calc_torquef()` (the oracle's C on one
/// side, `obs-graphics`'s port, which hybrid libobs calls, on the other).
#[track_caller]
fn check_tick(mode: i32, val: f32, target: f32, torque: f32, clamp: f32, t: f32) {
    let c = match check_manual(mode, torque) {
        OBS_SOURCE_TRANSITION_CLOCK_SNAP => target,
        // SAFETY: plain values.
        OBS_SOURCE_TRANSITION_CLOCK_STEP => unsafe {
            oracle_calc_torquef(val, target, torque, clamp, t)
        },
        _ => val,
    };
    let r = match shim::obs_source_transition_clock_manual(mode, torque) {
        OBS_SOURCE_TRANSITION_CLOCK_SNAP => target,
        OBS_SOURCE_TRANSITION_CLOCK_STEP => calc_torquef(val, target, torque, clamp, t),
        _ => val,
    };
    // Which NaN comes out of a calc_torquef() operation with two NaN
    // operands depends on the operand order the compiler picks (as in
    // #2021); that is calc_torquef()'s, not this port's, so two NaNs match.
    if r.is_nan() && c.is_nan() {
        return;
    }
    assert_eq!(
        r.to_bits(),
        c.to_bits(),
        "tick({mode}, {val:e}, {target:e}, {torque:e}, {clamp:e}, {t:e})"
    );
}

#[track_caller]
fn check_size(set: u32, actual: u32) {
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_transition_clock_size(set, actual) };
    assert_eq!(
        shim::obs_source_transition_clock_size(set, actual),
        c,
        "size({set}, {actual})"
    );
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_transition_clock_fold(set, actual) };
    assert_eq!(
        shim::obs_source_transition_clock_fold(set, actual),
        c,
        "fold({set}, {actual})"
    );
}

/// The fit on each side, with the scale pre-filled with [`GUARD`]: the
/// return value and both floats' bits after the call.
#[track_caller]
fn check_fit(
    tr_cx: f32,
    tr_cy: f32,
    source_cx: f32,
    source_cy: f32,
    scale_type: i32,
) -> (bool, u32, u32) {
    let (mut cx, mut cy) = (f32::from_bits(GUARD), f32::from_bits(GUARD));
    // SAFETY: two valid floats.
    let c = unsafe {
        oracle::oracle_obs_source_transition_clock_fit(
            tr_cx, tr_cy, source_cx, source_cy, scale_type, &mut cx, &mut cy,
        )
    };
    let (mut rx, mut ry) = (f32::from_bits(GUARD), f32::from_bits(GUARD));
    // SAFETY: as above.
    let r = unsafe {
        shim::obs_source_transition_clock_fit(
            tr_cx, tr_cy, source_cx, source_cy, scale_type, &mut rx, &mut ry,
        )
    };
    let c = (c, cx.to_bits(), cy.to_bits());
    let r = (r, rx.to_bits(), ry.to_bits());
    assert_eq!(
        r, c,
        "fit({tr_cx:e}, {tr_cy:e}, {source_cx:e}, {source_cy:e}, {scale_type})"
    );
    r
}

#[track_caller]
fn check_mix(a: i32, b: i32) -> i32 {
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_transition_clock_mix_spaces(a, b) };
    let r = shim::obs_source_transition_clock_mix_spaces(a, b);
    assert_eq!(r, c, "mix_spaces({a}, {b})");
    r
}

#[track_caller]
fn check_min_ts(a: u64, b: u64) -> u64 {
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_transition_clock_min_ts(a, b) };
    let r = shim::obs_source_transition_clock_min_ts(a, b);
    assert_eq!(r, c, "min_ts({a}, {b})");
    r
}

// --------------------------------------------------------------------------
// The clock

const MODES: [i32; 6] = [0, 1, 2, -1, i32::MIN, i32::MAX];

fn special_f32() -> Vec<f32> {
    vec![
        0.0,
        -0.0,
        0.25,
        0.5,
        1.0,
        -1.0,
        2.5,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::from_bits(0x7fc0_0000),
        f32::from_bits(0xffc0_1234),
        f32::from_bits(0x7f80_0001),
        f32::from_bits(1),
        f32::MAX,
    ]
}

const STARTS: [u64; 6] = [0, 1, 1000, 3_600_000 * MSEC, TWO_63, u64::MAX - 10];
const DURATIONS: [u64; 12] = [
    0,
    1,
    2,
    3,
    100,
    300 * MSEC,
    33_554_431_999,
    u32::MAX as u64 * MSEC,
    TWO_53,
    TWO_63,
    TWO_63 + 2,
    u64::MAX,
];

/// Timestamps around the start and the end of a clock.
fn timestamps(start: u64, duration: u64) -> Vec<u64> {
    let end = start.saturating_add(duration);
    let mut ts = vec![0, 1, u64::MAX, u64::MAX - 1];
    for base in [
        start,
        end,
        start.saturating_add(duration / 2),
        start.saturating_add(duration / 3),
    ] {
        for d in [-2i64, -1, 0, 1, 2] {
            ts.push(base.saturating_add_signed(d));
        }
    }
    ts
}

#[test]
fn time_every_mode_and_special_clock() {
    let mut calls = 0usize;
    for mode in MODES {
        for manual_val in special_f32() {
            for start_time in STARTS {
                for duration in DURATIONS {
                    let clock = Clock {
                        mode,
                        manual_val,
                        start_time,
                        duration,
                    };
                    for ts in timestamps(start_time, duration) {
                        check_time(clock, ts);
                        calls += 1;
                    }
                }
            }
        }
    }
    assert!(calls > 100_000, "{calls}");
}

#[test]
fn time_platform_ties() {
    for (p, q, x87, _, _) in PLATFORM_TIES {
        let r = check_ratio(p, q);
        assert_eq!(r.to_bits(), x87, "{p}/{q}");
    }
}

/// Ratios a few units from an `f32` rounding tie, in 40 binades below 1,
/// for durations from 0.5 s to past 2^63 ns, whole milliseconds among
/// them. Returns how many ratios it checked, and how many of those with a
/// duration below 2^53 ns a plain `f64` division rounds the other way.
fn near_tie_sweep(durations: &[u64], ties_per_binade: u64) -> (usize, usize) {
    let (mut calls, mut f64_differs) = (0usize, 0usize);
    for &q in durations {
        for e in 0..40u32 {
            for i in 0..ties_per_binade {
                // spread over the binade's 2^23 ties
                let m = (1u64 << 23) + (i * ((1 << 23) / ties_per_binade) + q % 977) % (1 << 23);
                let tie = (u128::from(2 * m + 1) * u128::from(q)) >> (25 + e);
                let tie = u64::try_from(tie).unwrap();
                for d in [-1i64, 0, 1] {
                    let p = tie.saturating_add_signed(d);
                    if p == 0 || p >= q {
                        continue;
                    }
                    let r = check_ratio(p, q);
                    if q < TWO_53 && r.to_bits() != f64_ratio(p, q).to_bits() {
                        f64_differs += 1;
                    }
                    calls += 1;
                }
            }
        }
    }
    (calls, f64_differs)
}

#[test]
fn time_near_ties() {
    let durations = [
        500 * MSEC,
        TWO_39 / 3,
        33_554_431_999,
        35_001 * MSEC,
        123_457 * MSEC,
        u32::MAX as u64 * MSEC,
        TWO_53 - 1,
        TWO_53 + 1,
        TWO_63 + 12_345,
        u64::MAX,
    ];
    let (calls, f64_differs) = near_tie_sweep(&durations, 512);
    assert!(calls > 500_000, "{calls}");
    // below 2^53 too: the 33.5 s and whole-millisecond durations
    assert!(f64_differs > 0, "no ratio below 2^53 where f64 differs");
    eprintln!(
        "near ties: {calls} ratios; below 2^53 a plain f64 division differs on {f64_differs}"
    );
}

/// The near-tie sweep over every 64th tie of every binade, for more
/// durations. Run with `--release --ignored`.
#[test]
#[ignore = "slow: about a billion ratios, run with --release --ignored"]
fn time_near_ties_wide() {
    let mut durations = vec![33_554_431_999, u64::MAX, TWO_63 + 1, TWO_53 - 1];
    // whole milliseconds from 30 s to 30 min, odd ones (the even ones have
    // more factors of 2, which keep a double off the ties)
    durations.extend((30_001u64..1_800_000).step_by(39_998).map(|ms| ms * MSEC));
    let (calls, f64_differs) = near_tie_sweep(&durations, 1 << 17);
    eprintln!(
        "near ties (wide): {calls} ratios; below 2^53 a plain f64 division differs on {f64_differs}"
    );
}

#[test]
fn time_every_small_duration() {
    for q in 2u64..=1 << 11 {
        for p in 1..q {
            check_ratio(p, q);
        }
    }
}

/// Every ratio with a duration up to 2^15 ns. Run with `--release
/// --ignored`.
#[test]
#[ignore = "slow: 2^29 ratios, run with --release --ignored"]
fn time_every_duration_to_2_15() {
    for q in 2u64..=1 << 15 {
        for p in 1..q {
            check_ratio(p, q);
        }
    }
}

// --------------------------------------------------------------------------
// The manual step, the size and the fold

const TORQUES: [f32; 10] = [
    0.0,
    -0.0,
    1.0,
    0.5,
    -1.0,
    1e-30,
    f32::INFINITY,
    f32::NEG_INFINITY,
    f32::NAN,
    8.0,
];

#[test]
fn manual_every_mode_and_torque() {
    for mode in MODES {
        for torque in TORQUES.into_iter().chain(special_f32()) {
            check_manual(mode, torque);
        }
    }
}

#[test]
fn manual_tick_special_values() {
    let values = [
        0.0f32,
        0.5,
        1.0,
        0.75,
        -0.0,
        0.5 + 2.0f32.powi(-20),
        f32::NAN,
        2.0,
    ];
    let clamps = [0.0f32, 0.25, 1e-6, f32::NAN];
    let times = [0.0f32, 0.5, 1.0, 1.0 / 60.0];
    for mode in [0, MODE_MANUAL, 2] {
        for val in values {
            for target in values {
                for torque in TORQUES {
                    for clamp in clamps {
                        for t in times {
                            check_tick(mode, val, target, torque, clamp, t);
                        }
                    }
                }
            }
        }
    }
}

const SIZES_U32: [u32; 9] = [0, 1, 2, 720, 1080, 1280, 1920, u32::MAX - 1, u32::MAX];

#[test]
fn size_and_fold_special_values() {
    for set in SIZES_U32 {
        for actual in SIZES_U32 {
            check_size(set, actual);
        }
    }
}

// --------------------------------------------------------------------------
// The fit

const SCALE_TYPES: [i32; 8] = [0, 1, 2, 3, -1, 100, i32::MIN, i32::MAX];

fn special_sizes() -> [f32; 12] {
    [
        0.0,
        -0.0,
        1.0,
        720.0,
        1080.0,
        1920.0,
        3840.0,
        u32::MAX as f32,
        f32::NAN,
        f32::INFINITY,
        f32::from_bits(1),
        -1.0,
    ]
}

#[test]
fn fit_every_type_and_special_size() {
    let sizes = special_sizes();
    let mut seen = [0usize; 3];
    for scale_type in SCALE_TYPES {
        for tr_cx in sizes {
            for tr_cy in sizes {
                for source_cx in sizes {
                    for source_cy in sizes {
                        let (matrix, x, _) =
                            check_fit(tr_cx, tr_cy, source_cx, source_cy, scale_type);
                        let kind = if !matrix {
                            0
                        } else if x == GUARD {
                            1
                        } else {
                            2
                        };
                        seen[kind] += 1;
                    }
                }
            }
        }
    }
    // no matrix, a matrix with no scale, a scale: each many times
    assert!(seen.iter().all(|&n| n > 10_000), "{seen:?}");
}

#[test]
fn fit_video_sizes() {
    let sizes = [
        1u32, 2, 480, 640, 720, 1080, 1280, 1440, 1920, 2160, 2560, 3840, 7680,
    ];
    for scale_type in [SCALE_MAX_ONLY, SCALE_ASPECT, SCALE_STRETCH] {
        for &tr_cx in &sizes {
            for &tr_cy in &sizes {
                for &source_cx in &sizes {
                    for &source_cy in &sizes {
                        check_fit(
                            tr_cx as f32,
                            tr_cy as f32,
                            source_cx as f32,
                            source_cy as f32,
                            scale_type,
                        );
                    }
                }
            }
        }
    }
}

/// Intentional difference: the C writes through a null scale pointer; the
/// shim skips it and gives the same answer.
#[test]
fn fit_null_scale_in_the_shim() {
    let mut y = 0.0f32;
    // SAFETY: the shim checks for null before it writes; y is valid.
    let matrix = unsafe {
        shim::obs_source_transition_clock_fit(
            1920.0,
            1080.0,
            960.0,
            720.0,
            SCALE_STRETCH,
            std::ptr::null_mut(),
            &mut y,
        )
    };
    assert!(matrix);
    assert_eq!(y, 1.5);
    let mut x = 0.0f32;
    // SAFETY: as above.
    let matrix = unsafe {
        shim::obs_source_transition_clock_fit(
            1920.0,
            1080.0,
            960.0,
            720.0,
            SCALE_STRETCH,
            &mut x,
            std::ptr::null_mut(),
        )
    };
    assert!(matrix);
    assert_eq!(x, 2.0);
    for scale_type in SCALE_TYPES {
        // SAFETY: as above.
        let matrix = unsafe {
            shim::obs_source_transition_clock_fit(
                1920.0,
                1080.0,
                3840.0,
                2160.0,
                scale_type,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(matrix);
    }
}

// --------------------------------------------------------------------------
// The colour spaces and the timestamps

#[test]
fn mix_spaces_every_pair() {
    let spaces = [
        -2,
        -1,
        0,
        1,
        2,
        3,
        4,
        5,
        100,
        i32::MIN,
        i32::MAX,
        i32::MIN + 1,
    ];
    let mut seen = [0usize; 3];
    for a in spaces {
        for b in spaces {
            seen[usize::try_from(check_mix(a, b)).unwrap()] += 1;
        }
    }
    assert!(seen.iter().all(|&n| n > 0), "{seen:?}");
}

#[test]
fn min_ts_special_pairs() {
    let values = [
        0u64,
        1,
        2,
        5000,
        1 << 32,
        TWO_53,
        TWO_63,
        u64::MAX - 1,
        u64::MAX,
    ];
    for a in values {
        for b in values {
            check_min_ts(a, b);
        }
    }
}

// --------------------------------------------------------------------------
// Proptests

fn any_u64() -> impl Strategy<Value = u64> {
    prop_oneof![
        3 => any::<u64>(),
        2 => (any::<u64>(), 0u32..64).prop_map(|(v, shift)| v >> shift),
        1 => prop::sample::select(vec![0u64, 1, 2, TWO_39, TWO_53 - 1, TWO_53, TWO_63, u64::MAX]),
    ]
}

fn any_f32() -> impl Strategy<Value = f32> {
    prop_oneof![
        3 => any::<u32>().prop_map(f32::from_bits),
        1 => prop::sample::select(special_f32()),
        // in [0, 1], uniformly over the bits
        2 => (0u32..=0x3f80_0000).prop_map(f32::from_bits),
    ]
}

fn any_size() -> impl Strategy<Value = f32> {
    prop_oneof![
        2 => any::<u32>().prop_map(|v| v as f32),
        2 => (0u32..=8192).prop_map(|v| v as f32),
        1 => prop::sample::select(special_sizes().to_vec()),
        1 => any::<u32>().prop_map(f32::from_bits),
    ]
}

fn any_mode() -> impl Strategy<Value = i32> {
    prop_oneof![2 => Just(MODE_MANUAL), 2 => Just(0), 1 => any::<i32>()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    /// Any clock: half the time a timestamp near the start, either side,
    /// and half the time near the end.
    #[test]
    fn time_any_clock(
        mode in any_mode(),
        manual_val in any_f32(),
        start_time in any_u64(),
        duration in any_u64(),
        offset in any_u64(),
        near in 0u8..3,
    ) {
        let ts = match near {
            0 => start_time.saturating_sub(offset),
            1 => start_time.saturating_add(offset),
            _ => start_time.saturating_add(duration).saturating_add_signed(
                i64::try_from(offset % 4096).unwrap() - 2048,
            ),
        };
        check_time(
            Clock {
                mode,
                manual_val,
                start_time,
                duration,
            },
            ts,
        );
    }

    /// Any ratio, half of them near a rounding tie.
    #[test]
    fn time_any_ratio(
        q in (any::<u64>(), 0u32..63).prop_map(|(q, shift)| (q >> shift).max(2)),
        p_seed in any::<u64>(),
        near_tie in any::<bool>(),
        m in (1u64 << 23)..(1 << 24),
        e in 0u32..40,
        d in -2i64..=2,
    ) {
        let p = if near_tie {
            let tie = (u128::from(2 * m + 1) * u128::from(q)) >> (25 + e);
            u64::try_from(tie).unwrap().saturating_add_signed(d)
        } else {
            p_seed
        };
        check_ratio(1 + p % (q - 1), q);
    }

    #[test]
    fn manual_any_torque(mode in any_mode(), torque in any_f32()) {
        check_manual(mode, torque);
    }

    #[test]
    fn tick_any_values(
        mode in any_mode(),
        val in any_f32(),
        target in any_f32(),
        torque in any_f32(),
        clamp in any_f32(),
        t in any_f32(),
    ) {
        check_tick(mode, val, target, torque, clamp, t);
    }

    #[test]
    fn size_and_fold_any(set in any::<u32>(), actual in any::<u32>()) {
        check_size(set, actual);
    }

    #[test]
    fn fit_any_sizes(
        tr_cx in any_size(),
        tr_cy in any_size(),
        source_cx in any_size(),
        source_cy in any_size(),
        scale_type in prop_oneof![4 => 0i32..3, 1 => any::<i32>()],
    ) {
        check_fit(tr_cx, tr_cy, source_cx, source_cy, scale_type);
    }

    #[test]
    fn mix_spaces_any(a in prop_oneof![-2i32..6, any::<i32>()], b in prop_oneof![-2i32..6, any::<i32>()]) {
        check_mix(a, b);
    }

    #[test]
    fn min_ts_any(a in prop_oneof![Just(0u64), any_u64()], b in prop_oneof![Just(0u64), any_u64()]) {
        check_min_ts(a, b);
    }
}
