//! Tier 3: the `obs_source_transition_start_*` shims and the safe core
//! against the original C (`obs-c-oracle`, `oracle/source_transition_start.c`:
//! the unmodified `libobs/obs-source-transition-start.c.h` with its
//! functions renamed).
//!
//! The fourteen functions take and return `bool`, `int`, `uint32_t`,
//! `uint64_t`, `float`, `size_t` and the admission enum by value, so a run
//! compares each `bool` as the byte the C returns (it must be 0 or 1, and
//! the same on both sides), the admission as the C's enum value and each
//! number exactly. Each input is given to the oracle, to the shim (the
//! `extern "C"` function the hybrid links) and to the core.
//!
//! Runs:
//! - the header's values and the sizes of the admission enum, `bool` and
//!   `size_t`;
//! - every function of flags over every combination of them;
//! - the second admission over all 16 flag combinations for every mode
//!   from -100,000 to 100,000, every 65,521st `int` and the ends;
//! - the duration and the size rule over every value to 100,000, every
//!   65,521st `uint32_t`, every power of two and its neighbours, and the
//!   ends;
//! - the manual reset at every mode from -1,000 to 1,000 and the ends over
//!   36 special floats, and in manual mode over every 257th `f32` bit
//!   pattern, the 65,536 patterns either side of both zeros, the first
//!   65,536 of each NaN range and both infinities;
//! - proptests (20,000 cases each) of every function over full-range
//!   values: any flags, any `int` mode, any `uint32_t`, any `f32` bit
//!   pattern.
//!
//! Nothing is excluded: the C has no undefined or host-dependent result
//! here, and the port has no intentional difference.

use std::os::raw::c_int;

use obs_c_oracle::source_transition_start::{self as oracle, VALUE_NAMES};
use obs_source::ffi::source_transition_start::{
    self as shim, OBS_SOURCE_TRANSITION_START_DONE, OBS_SOURCE_TRANSITION_START_GO,
    OBS_SOURCE_TRANSITION_START_REFUSE,
};
use obs_source::transition_start::{self as core, Admission, MODE_MANUAL};
use proptest::prelude::*;

/// The C's `bool` byte as a `bool`: it must be 0 or 1.
fn c_bool(byte: u8, what: &str) -> bool {
    assert!(byte <= 1, "{what}: the C returned {byte}");
    byte == 1
}

fn bit(bits: u32, n: u32) -> bool {
    bits & (1 << n) != 0
}

fn admission_value(a: Admission) -> u32 {
    match a {
        Admission::Go => OBS_SOURCE_TRANSITION_START_GO,
        Admission::Refuse => OBS_SOURCE_TRANSITION_START_REFUSE,
        Admission::Done => OBS_SOURCE_TRANSITION_START_DONE,
    }
}

fn check_admit(ss: bool, sd: bool, same_mode: bool, mode: c_int, active: bool) {
    // SAFETY: plain values in and out
    let c = unsafe {
        oracle::oracle_obs_source_transition_start_admit(ss, sd, same_mode, mode, active)
    };
    let s = shim::obs_source_transition_start_admit(ss, sd, same_mode, mode, active);
    let r = admission_value(core::admit(ss, sd, same_mode, mode, active));
    assert_eq!(
        (s, r),
        (c, c),
        "admit({ss}, {sd}, {same_mode}, {mode}, {active})"
    );
}

fn check_duration(ms: u32) {
    // SAFETY: plain values in and out
    let c = unsafe { oracle::oracle_obs_source_transition_start_duration_ns(ms) };
    assert_eq!(
        shim::obs_source_transition_start_duration_ns(ms),
        c,
        "duration_ns({ms})"
    );
    assert_eq!(core::duration_ns(ms), c, "duration_ns({ms})");
}

fn check_unsized(size: u32) {
    // SAFETY: plain values in and out
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_transition_start_unsized(size) },
        "unsized",
    );
    assert_eq!(
        shim::obs_source_transition_start_unsized(size),
        c,
        "unsized({size})"
    );
    assert_eq!(core::is_unsized(size), c, "unsized({size})");
}

fn check_manual_reset(mode: c_int, t: f32) {
    // SAFETY: plain values in and out
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_transition_start_manual_reset(mode, t) },
        "manual_reset",
    );
    let what = || format!("manual_reset({mode}, {:#010x})", t.to_bits());
    assert_eq!(
        shim::obs_source_transition_start_manual_reset(mode, t),
        c,
        "{}",
        what()
    );
    assert_eq!(core::manual_reset(mode, t), c, "{}", what());
}

/// Every function of flags only, at one combination of up to three flags.
fn check_flags(a: bool, b: bool, c: bool) {
    // SAFETY: plain values in and out, for every call below
    unsafe {
        let two = |what: &str, o: u8, s: bool, r: bool| {
            let o = c_bool(o, what);
            assert_eq!((s, r), (o, o), "{what}({a}, {b}, {c})");
        };
        two(
            "refused",
            oracle::oracle_obs_source_transition_start_refused(a, b),
            shim::obs_source_transition_start_refused(a, b),
            core::refused(a, b),
        );
        two(
            "walks",
            oracle::oracle_obs_source_transition_start_walks(a, b),
            shim::obs_source_transition_start_walks(a, b),
            core::walks(a, b),
        );
        two(
            "restart",
            oracle::oracle_obs_source_transition_start_restart(a, b, c),
            shim::obs_source_transition_start_restart(a, b, c),
            core::restart(a, b, c),
        );
        two(
            "adds",
            oracle::oracle_obs_source_transition_start_adds(a, b),
            shim::obs_source_transition_start_adds(a, b),
            core::adds(a, b),
        );
        two(
            "removes",
            oracle::oracle_obs_source_transition_start_removes(a, b, c),
            shim::obs_source_transition_start_removes(a, b, c),
            core::removes(a, b, c),
        );
        two(
            "calls_back",
            oracle::oracle_obs_source_transition_start_calls_back(a, b),
            shim::obs_source_transition_start_calls_back(a, b),
            core::calls_back(a, b),
        );
        two(
            "null_dest",
            oracle::oracle_obs_source_transition_start_null_dest(a, b, c),
            shim::obs_source_transition_start_null_dest(a, b, c),
            core::null_dest(a, b, c),
        );
        two(
            "activates",
            oracle::oracle_obs_source_transition_start_activates(a),
            shim::obs_source_transition_start_activates(a),
            core::activates(a),
        );
        two(
            "child_active",
            oracle::oracle_obs_source_transition_start_child_active(a, b),
            shim::obs_source_transition_start_child_active(a, b),
            core::child_active(a, b),
        );
        let o = oracle::oracle_obs_source_transition_start_active_index(a);
        assert_eq!(
            shim::obs_source_transition_start_active_index(a),
            o,
            "active_index({a})"
        );
        assert_eq!(core::active_index(a), o, "active_index({a})");
    }
}

#[test]
fn parity_values() {
    // SAFETY: plain values in and out
    let v = |i: usize| unsafe { oracle::oracle_source_transition_start_value(i) };
    let want = [
        i64::from(MODE_MANUAL),
        i64::from(OBS_SOURCE_TRANSITION_START_GO),
        i64::from(OBS_SOURCE_TRANSITION_START_REFUSE),
        i64::from(OBS_SOURCE_TRANSITION_START_DONE),
        i64::try_from(size_of::<u32>()).unwrap(),
        i64::try_from(size_of::<bool>()).unwrap(),
        i64::try_from(size_of::<usize>()).unwrap(),
    ];
    for (i, (name, want)) in VALUE_NAMES.iter().zip(want).enumerate() {
        assert_eq!(v(i), want, "{name}");
    }
    assert_eq!(v(VALUE_NAMES.len()), i64::MIN);
    assert_eq!(Admission::Go as u32, OBS_SOURCE_TRANSITION_START_GO);
    assert_eq!(Admission::Refuse as u32, OBS_SOURCE_TRANSITION_START_REFUSE);
    assert_eq!(Admission::Done as u32, OBS_SOURCE_TRANSITION_START_DONE);
}

#[test]
fn parity_every_flag_combination() {
    for bits in 0..8 {
        check_flags(bit(bits, 0), bit(bits, 1), bit(bits, 2));
    }
}

fn modes() -> Vec<c_int> {
    let mut m: Vec<c_int> = (-100_000..=100_000).collect();
    m.extend((c_int::MIN..=c_int::MAX).step_by(65_521));
    m.extend([c_int::MIN, c_int::MIN + 1, c_int::MAX - 1, c_int::MAX]);
    m
}

#[test]
fn parity_admit() {
    for mode in modes() {
        for bits in 0..16 {
            check_admit(bit(bits, 0), bit(bits, 1), bit(bits, 2), mode, bit(bits, 3));
        }
    }
}

#[test]
fn parity_durations_and_sizes() {
    let mut v: Vec<u32> = (0..=100_000).collect();
    v.extend((0..=u32::MAX).step_by(65_521));
    for p in 0..32 {
        let x = 1u32 << p;
        v.extend([x - 1, x, x.wrapping_add(1)]);
    }
    v.extend([u32::MAX - 1, u32::MAX]);
    for x in v {
        check_duration(x);
        check_unsized(x);
    }
}

#[test]
fn parity_manual_reset() {
    let special = [
        0.0f32,
        -0.0,
        f32::NAN,
        -f32::NAN,
        f32::from_bits(0x7f80_0001),
        f32::from_bits(0x7fbf_ffff),
        f32::from_bits(0x7fc0_0001),
        f32::from_bits(0x7fff_ffff),
        f32::from_bits(0xff80_0001),
        f32::from_bits(0xffff_ffff),
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::from_bits(0x8000_0001),
        f32::from_bits(0x007f_ffff),
        f32::from_bits(0x807f_ffff),
        f32::MAX,
        f32::MIN,
        f32::EPSILON,
        -f32::EPSILON,
        1.0,
        -1.0,
        0.5,
        -0.5,
        0.25,
        0.75,
        0.999_999_94,
        1.000_000_1,
        1e-30,
        -1e-30,
        1e30,
        -1e30,
        2.0,
        100.0,
    ];
    let mut ms: Vec<c_int> = (-1000..=1000).collect();
    ms.extend([c_int::MIN, c_int::MIN + 1, c_int::MAX - 1, c_int::MAX]);
    for mode in ms {
        for t in special {
            check_manual_reset(mode, t);
        }
    }

    let mut bits: Vec<u32> = (0..=u32::MAX).step_by(257).collect();
    for edge in [
        0u32,
        0x8000_0000,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0000,
        0xffc0_0000,
    ] {
        bits.extend((0..65_536).map(|d| edge.wrapping_add(d)));
        bits.extend((1..65_536).map(|d| edge.wrapping_sub(d)));
    }
    for b in bits {
        check_manual_reset(MODE_MANUAL, f32::from_bits(b));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn parity_proptest_values(
        flags in any::<u8>(),
        mode in any::<c_int>(),
        x in any::<u32>(),
        t in any::<u32>(),
    ) {
        let f = |n: u32| flags & (1 << n) != 0;
        check_flags(f(0), f(1), f(2));
        check_admit(f(0), f(1), f(2), mode, f(3));
        check_admit(f(0), f(1), f(2), MODE_MANUAL, f(3));
        check_duration(x);
        check_unsized(x);
        check_manual_reset(mode, f32::from_bits(t));
        check_manual_reset(MODE_MANUAL, f32::from_bits(t));
    }
}
