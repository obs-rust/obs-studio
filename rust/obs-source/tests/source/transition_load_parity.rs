//! Tier 3: the `obs_source_transition_load_*` shims and the safe core
//! against the original C (`obs-c-oracle`, `oracle/source_transition_load.c`:
//! the unmodified `libobs/obs-source-transition-load.c.h` with its functions
//! renamed).
//!
//! The six functions take and return `bool`, `int`, `int64_t` and the
//! source outcome enum by value, so a run compares each `bool` as the byte
//! the C returns (it must be 0 or 1, and the same on both sides), the
//! outcome as the C's enum value and each number exactly. Each input is
//! given to the oracle, to the shim (the `extern "C"` function the hybrid
//! links) and to the core.
//!
//! Runs:
//! - the header's values and the sizes of the outcome enum, `bool` and
//!   `int`;
//! - every function of flags over every combination of them;
//! - the type check over every `int` from -1,000,000 to 1,000,000, every
//!   65,521st `int`, 2 with each other bit set, and the ends;
//! - the scale type over every value from -1,000,000 to 1,000,000, every
//!   power of two with its neighbours and negations, the 65,536 values
//!   either side of 2^32, -2^32, 2^63 and -2^63 (each end), a stride over
//!   `int64_t`, and the ends;
//! - proptests (20,000 cases each) of every function over full-range
//!   values: any flags, any `int` type, any `int64_t` scale type.
//!
//! Nothing is excluded: the C has no undefined or host-dependent result
//! here, and the port has no intentional difference.

use std::os::raw::c_int;

use obs_c_oracle::source_transition_load::{self as oracle, VALUE_NAMES};
use obs_source::ffi::source_transition_load::{
    self as shim, OBS_SOURCE_TRANSITION_LOAD_KEEP, OBS_SOURCE_TRANSITION_LOAD_NOT_FOUND,
    OBS_SOURCE_TRANSITION_LOAD_REFUSE,
};
use obs_source::transition_load::{
    self as core, SCALE_ASPECT, SCALE_MAX_ONLY, SCALE_STRETCH, Source, TYPE_TRANSITION,
};
use proptest::prelude::*;

/// The C's `bool` byte as a `bool`: it must be 0 or 1.
fn c_bool(byte: u8, what: &str) -> bool {
    assert!(byte <= 1, "{what}: the C returned {byte}");
    byte == 1
}

fn bit(bits: u32, n: u32) -> bool {
    bits & (1 << n) != 0
}

fn source_value(s: Source) -> u32 {
    match s {
        Source::Keep => OBS_SOURCE_TRANSITION_LOAD_KEEP,
        Source::NotFound => OBS_SOURCE_TRANSITION_LOAD_NOT_FOUND,
        Source::Refuse => OBS_SOURCE_TRANSITION_LOAD_REFUSE,
    }
}

fn check_is_transition(t: c_int) {
    // SAFETY: plain values in and out
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_transition_load_is_transition(t) },
        "is_transition",
    );
    assert_eq!(
        shim::obs_source_transition_load_is_transition(t),
        c,
        "is_transition({t})"
    );
    assert_eq!(core::is_transition(t), c, "is_transition({t})");
}

fn check_scale_type(v: i64) {
    // SAFETY: plain values in and out
    let c = unsafe { oracle::oracle_obs_source_transition_load_scale_type(v) };
    assert_eq!(
        shim::obs_source_transition_load_scale_type(v),
        c,
        "scale_type({v})"
    );
    assert_eq!(core::scale_type(v), c, "scale_type({v})");
}

/// Every function of flags only, at one combination of two flags.
fn check_flags(a: bool, b: bool) {
    // SAFETY: plain values in and out, for every call below
    unsafe {
        let two = |what: &str, o: u8, s: bool, r: bool| {
            let o = c_bool(o, what);
            assert_eq!((s, r), (o, o), "{what}({a}, {b})");
        };
        two(
            "looks_up",
            oracle::oracle_obs_source_transition_load_looks_up(a),
            shim::obs_source_transition_load_looks_up(a),
            core::looks_up(a),
        );
        two(
            "swaps",
            oracle::oracle_obs_source_transition_load_swaps(a),
            shim::obs_source_transition_load_swaps(a),
            core::swaps(a),
        );
        two(
            "copy_stores",
            oracle::oracle_obs_source_transition_load_copy_stores(a, b),
            shim::obs_source_transition_load_copy_stores(a, b),
            core::copy_stores(a, b),
        );
        let o = oracle::oracle_obs_source_transition_load_source(a, b);
        assert_eq!(
            (
                shim::obs_source_transition_load_source(a, b),
                source_value(core::source(a, b))
            ),
            (o, o),
            "source({a}, {b})"
        );
    }
}

#[test]
fn parity_values() {
    // SAFETY: plain values in and out
    let v = |i: usize| unsafe { oracle::oracle_source_transition_load_value(i) };
    let want = [
        i64::from(TYPE_TRANSITION),
        i64::from(SCALE_MAX_ONLY),
        i64::from(SCALE_ASPECT),
        i64::from(SCALE_STRETCH),
        i64::from(OBS_SOURCE_TRANSITION_LOAD_KEEP),
        i64::from(OBS_SOURCE_TRANSITION_LOAD_NOT_FOUND),
        i64::from(OBS_SOURCE_TRANSITION_LOAD_REFUSE),
        i64::try_from(size_of::<u32>()).unwrap(),
        i64::try_from(size_of::<bool>()).unwrap(),
        i64::try_from(size_of::<c_int>()).unwrap(),
    ];
    for (i, (name, want)) in VALUE_NAMES.iter().zip(want).enumerate() {
        assert_eq!(v(i), want, "{name}");
    }
    assert_eq!(v(VALUE_NAMES.len()), i64::MIN);
    assert_eq!(Source::Keep as u32, OBS_SOURCE_TRANSITION_LOAD_KEEP);
    assert_eq!(
        Source::NotFound as u32,
        OBS_SOURCE_TRANSITION_LOAD_NOT_FOUND
    );
    assert_eq!(Source::Refuse as u32, OBS_SOURCE_TRANSITION_LOAD_REFUSE);
}

#[test]
fn parity_flags_every_combination() {
    for bits in 0..4 {
        check_flags(bit(bits, 0), bit(bits, 1));
    }
}

#[test]
fn parity_is_transition() {
    for t in -1_000_000..=1_000_000 {
        check_is_transition(t);
    }
    let mut t = i32::MIN;
    while let Some(next) = t.checked_add(65_521) {
        check_is_transition(t);
        t = next;
    }
    for shift in 0..32 {
        check_is_transition(2 | (1_i32 << shift));
    }
    for t in [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX] {
        check_is_transition(t);
    }
}

#[test]
fn parity_scale_type() {
    for v in -1_000_000_i64..=1_000_000 {
        check_scale_type(v);
    }
    for shift in 0..63 {
        let p = 1_i64 << shift;
        for v in [p - 1, p, p + 1, p + 2, -p, -p + 1, -p + 2, -p - 1] {
            check_scale_type(v);
        }
    }
    for centre in [1_i64 << 32, -(1_i64 << 32)] {
        for d in -65_536..=65_536 {
            check_scale_type(centre + d);
        }
    }
    for d in 0..65_536 {
        check_scale_type(i64::MAX - d);
        check_scale_type(i64::MIN + d);
    }
    let mut v = i64::MIN;
    while let Some(next) = v.checked_add(i64::MAX / 65_521) {
        check_scale_type(v);
        v = next;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn parity_full_range(t in any::<c_int>(), v in any::<i64>(), a in any::<bool>(), b in any::<bool>()) {
        check_is_transition(t);
        check_scale_type(v);
        check_flags(a, b);
    }
}
