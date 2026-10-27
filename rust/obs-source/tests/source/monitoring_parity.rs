//! Tier 3: the `obs_source_monitoring_*` shims and the safe core against the
//! original C (`obs-c-oracle`, `oracle/source_monitoring.c`: the unmodified
//! `libobs/obs-source-monitoring.c.h` with its functions renamed).
//!
//! The thirteen functions take and return `bool`, `int`, `uint32_t`,
//! `int64_t`, `float` and the monitor, route and active enums by value, so a
//! run compares each `bool` as the byte the C returns (it must be 0 or 1, and
//! the same on both sides), each outcome as the C's enum value, each integer
//! exactly and the balance by its bits (so NaN equals NaN, and -0.0 is not
//! 0.0). Each input is given to the oracle, to the shim (the `extern "C"`
//! function the hybrid links) and to the core.
//!
//! Runs:
//! - the header's values and the sizes of the three enums, `bool`, `int` and
//!   `float`;
//! - every function of flags over every combination of them, the push-to
//!   log rule with each flag bit;
//! - the same-value rule and the audio gate over every pair of 32-bit edges
//!   and every single bit;
//! - the mixers at every power of two of `int64_t` with its neighbours and
//!   negations, the 65,536 values either side of 2^32, -2^32 and each end,
//!   and a stride over `int64_t`;
//! - the route over every `int` from -1,000,000 to 1,000,000, every
//!   65,521st `int` and the ends;
//! - the balance over every bit pattern whose top byte is 0x00, 0x3f, 0x7f,
//!   0x80, 0xbf or 0xff (the zeros, the subnormals, 0.5 to 2.0, the
//!   infinities and every NaN, of both signs) and every 251st pattern;
//! - proptests (20,000 cases each) of every function over full-range
//!   values.
//!
//! Nothing is excluded: the C has no undefined or host-dependent result
//! here (`isnan()` and the compares are IEEE on every host, and the
//! `(uint32_t)` of an `int64_t` is modular), and the port has no intentional
//! difference.

use std::os::raw::c_int;

use obs_c_oracle::source_monitoring::{self as oracle, VALUE_NAMES};
use obs_source::ffi::source_monitoring::{
    self as shim, OBS_SOURCE_MONITORING_ACTIVE_ACTIVATE, OBS_SOURCE_MONITORING_ACTIVE_DEACTIVATE,
    OBS_SOURCE_MONITORING_ACTIVE_NONE, OBS_SOURCE_MONITORING_MONITOR_CREATE,
    OBS_SOURCE_MONITORING_MONITOR_DESTROY, OBS_SOURCE_MONITORING_MONITOR_KEEP,
    OBS_SOURCE_MONITORING_ROUTE_MONITOR_AND_OUTPUT, OBS_SOURCE_MONITORING_ROUTE_MONITOR_ONLY,
    OBS_SOURCE_MONITORING_ROUTE_OFF, OBS_SOURCE_MONITORING_ROUTE_ON,
};
use obs_source::monitoring::{
    self as core, AUDIO, Active, Monitor, Route, TYPE_MONITOR_AND_OUTPUT, TYPE_MONITOR_ONLY,
    TYPE_NONE,
};
use proptest::prelude::*;

/// The C's `bool` byte as a `bool`: it must be 0 or 1.
fn c_bool(byte: u8, what: &str) -> bool {
    assert!(byte <= 1, "{what}: the C returned {byte}");
    byte == 1
}

fn monitor_value(m: Monitor) -> u32 {
    match m {
        Monitor::Keep => OBS_SOURCE_MONITORING_MONITOR_KEEP,
        Monitor::Create => OBS_SOURCE_MONITORING_MONITOR_CREATE,
        Monitor::Destroy => OBS_SOURCE_MONITORING_MONITOR_DESTROY,
    }
}

fn route_value(r: Route) -> u32 {
    match r {
        Route::Off => OBS_SOURCE_MONITORING_ROUTE_OFF,
        Route::MonitorOnly => OBS_SOURCE_MONITORING_ROUTE_MONITOR_ONLY,
        Route::MonitorAndOutput => OBS_SOURCE_MONITORING_ROUTE_MONITOR_AND_OUTPUT,
        Route::On => OBS_SOURCE_MONITORING_ROUTE_ON,
    }
}

fn active_value(a: Active) -> u32 {
    match a {
        Active::None => OBS_SOURCE_MONITORING_ACTIVE_NONE,
        Active::Activate => OBS_SOURCE_MONITORING_ACTIVE_ACTIVATE,
        Active::Deactivate => OBS_SOURCE_MONITORING_ACTIVE_DEACTIVATE,
    }
}

fn check_changes(cur: u32, value: u32) {
    // SAFETY: plain values in and out
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_monitoring_changes(cur, value) },
        "changes",
    );
    assert_eq!(
        shim::obs_source_monitoring_changes(cur, value),
        c,
        "changes({cur}, {value})"
    );
    assert_eq!(core::changes(cur, value), c, "changes({cur}, {value})");
}

fn check_has_audio(flags: u32) {
    // SAFETY: plain values in and out
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_monitoring_has_audio(flags) },
        "has_audio",
    );
    assert_eq!(
        shim::obs_source_monitoring_has_audio(flags),
        c,
        "has_audio({flags:#x})"
    );
    assert_eq!(core::has_audio(flags), c, "has_audio({flags:#x})");
}

fn check_mixers_store(v: i64) {
    // SAFETY: plain values in and out
    let c = unsafe { oracle::oracle_obs_source_monitoring_mixers_store(v) };
    assert_eq!(
        shim::obs_source_monitoring_mixers_store(v),
        c,
        "mixers_store({v})"
    );
    assert_eq!(core::mixers_store(v), c, "mixers_store({v})");
}

fn check_push_to_logs(flags: u32, changed: bool) {
    // SAFETY: plain values in and out
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_monitoring_push_to_logs(flags, changed) },
        "push_to_logs",
    );
    assert_eq!(
        shim::obs_source_monitoring_push_to_logs(flags, changed),
        c,
        "push_to_logs({flags:#x}, {changed})"
    );
    assert_eq!(
        core::push_to_logs(flags, changed),
        c,
        "push_to_logs({flags:#x}, {changed})"
    );
}

fn check_route(t: c_int) {
    // SAFETY: plain values in and out
    let c = unsafe { oracle::oracle_obs_source_monitoring_route(t) };
    assert_eq!(shim::obs_source_monitoring_route(t), c, "route({t})");
    assert_eq!(route_value(core::route(t)), c, "route({t})");
}

fn check_balance(b: u32) {
    let v = f32::from_bits(b);
    // SAFETY: plain values in and out
    let c = unsafe { oracle::oracle_obs_source_monitoring_balance(v) }.to_bits();
    assert_eq!(
        shim::obs_source_monitoring_balance(v).to_bits(),
        c,
        "balance({b:#x})"
    );
    assert_eq!(core::balance(v).to_bits(), c, "balance({b:#x})");
}

fn check_flags(a: bool, b: bool) {
    // SAFETY (each block): plain values in and out
    let c = unsafe { oracle::oracle_obs_source_monitoring_monitor(a, b) };
    assert_eq!(
        shim::obs_source_monitoring_monitor(a, b),
        c,
        "monitor({a}, {b})"
    );
    assert_eq!(monitor_value(core::monitor(a, b)), c, "monitor({a}, {b})");

    // SAFETY: plain values in and out
    let c = unsafe { oracle::oracle_obs_source_monitoring_audio_active(a, b) };
    assert_eq!(
        shim::obs_source_monitoring_audio_active(a, b),
        c,
        "audio_active({a}, {b})"
    );
    assert_eq!(
        active_value(core::audio_active(a, b)),
        c,
        "audio_active({a}, {b})"
    );

    // SAFETY: plain values in and out
    let c = unsafe { oracle::oracle_obs_source_monitoring_on_type(a) };
    assert_eq!(shim::obs_source_monitoring_on_type(a), c, "on_type({a})");
    assert_eq!(core::on_type(a), c, "on_type({a})");

    // SAFETY: plain values in and out
    let c = unsafe { oracle::oracle_obs_source_monitoring_stored_type(a) };
    assert_eq!(
        shim::obs_source_monitoring_stored_type(a),
        c,
        "stored_type({a})"
    );
    assert_eq!(core::stored_type(a), c, "stored_type({a})");

    // SAFETY: plain values in and out
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_monitoring_off_reads_mute(a) },
        "off_reads_mute",
    );
    assert_eq!(
        shim::obs_source_monitoring_off_reads_mute(a),
        c,
        "off_reads_mute({a})"
    );
    assert_eq!(core::off_reads_mute(a), c, "off_reads_mute({a})");

    // SAFETY: plain values in and out
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_monitoring_off_unmutes(a) },
        "off_unmutes",
    );
    assert_eq!(
        shim::obs_source_monitoring_off_unmutes(a),
        c,
        "off_unmutes({a})"
    );
    assert_eq!(core::off_unmutes(a), c, "off_unmutes({a})");

    // SAFETY: plain values in and out
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_monitoring_only_mutes(a) },
        "only_mutes",
    );
    assert_eq!(
        shim::obs_source_monitoring_only_mutes(a),
        c,
        "only_mutes({a})"
    );
    assert_eq!(core::only_mutes(a), c, "only_mutes({a})");

    check_changes(u32::from(a), u32::from(b));
    check_push_to_logs(AUDIO, b);
}

const EDGES: [u32; 14] = [
    0,
    1,
    2,
    3,
    4,
    0x7f,
    0x80,
    0xff,
    0x100,
    0x7fff_ffff,
    0x8000_0000,
    0x8000_0002,
    0xffff_fffe,
    0xffff_ffff,
];

#[test]
fn values_match_the_header() {
    let want: [i64; 20] = [
        i64::from(AUDIO),
        i64::from(TYPE_NONE),
        i64::from(TYPE_MONITOR_ONLY),
        i64::from(TYPE_MONITOR_AND_OUTPUT),
        i64::from(OBS_SOURCE_MONITORING_MONITOR_KEEP),
        i64::from(OBS_SOURCE_MONITORING_MONITOR_CREATE),
        i64::from(OBS_SOURCE_MONITORING_MONITOR_DESTROY),
        i64::from(OBS_SOURCE_MONITORING_ROUTE_OFF),
        i64::from(OBS_SOURCE_MONITORING_ROUTE_MONITOR_ONLY),
        i64::from(OBS_SOURCE_MONITORING_ROUTE_MONITOR_AND_OUTPUT),
        i64::from(OBS_SOURCE_MONITORING_ROUTE_ON),
        i64::from(OBS_SOURCE_MONITORING_ACTIVE_NONE),
        i64::from(OBS_SOURCE_MONITORING_ACTIVE_ACTIVATE),
        i64::from(OBS_SOURCE_MONITORING_ACTIVE_DEACTIVATE),
        i64::try_from(size_of::<u32>()).unwrap(),
        i64::try_from(size_of::<u32>()).unwrap(),
        i64::try_from(size_of::<u32>()).unwrap(),
        i64::try_from(size_of::<bool>()).unwrap(),
        i64::try_from(size_of::<c_int>()).unwrap(),
        i64::try_from(size_of::<f32>()).unwrap(),
    ];
    for (i, (name, w)) in VALUE_NAMES.iter().zip(want).enumerate() {
        // SAFETY: plain values in and out
        assert_eq!(
            unsafe { oracle::oracle_source_monitoring_value(i) },
            w,
            "{name}"
        );
    }
    // SAFETY: plain values in and out
    assert_eq!(
        unsafe { oracle::oracle_source_monitoring_value(VALUE_NAMES.len()) },
        i64::MIN
    );
}

#[test]
fn flags_every_combination() {
    for a in [false, true] {
        for b in [false, true] {
            check_flags(a, b);
        }
    }
    for &flags in &EDGES {
        for changed in [false, true] {
            check_push_to_logs(flags, changed);
        }
    }
    for bit in 0..32 {
        for changed in [false, true] {
            check_push_to_logs(1 << bit, changed);
            check_push_to_logs(!(1u32 << bit), changed);
        }
    }
}

#[test]
fn changes_and_audio_gate_edges() {
    for &a in &EDGES {
        check_has_audio(a);
        for &b in &EDGES {
            check_changes(a, b);
        }
    }
    for bit in 0..32 {
        let v = 1u32 << bit;
        check_has_audio(v);
        check_has_audio(!v);
        check_changes(v, v);
        check_changes(v, !v);
        check_changes(v, 0);
    }
}

#[test]
fn mixers_store_edges() {
    for bit in 0..63 {
        let v = 1i64 << bit;
        for x in [v - 1, v, v + 1, -v, -v - 1, -v + 1] {
            check_mixers_store(x);
        }
    }
    for base in [1i64 << 32, -(1i64 << 32)] {
        for d in -65_536..65_536 {
            check_mixers_store(base + d);
        }
    }
    for d in 0..65_536 {
        check_mixers_store(i64::MIN + d);
        check_mixers_store(i64::MAX - d);
    }
    let mut x = i64::MIN;
    while let Some(next) = x.checked_add(0x0000_7fff_ffff_fffb) {
        check_mixers_store(x);
        x = next;
    }
}

#[test]
fn route_range() {
    for t in -1_000_000..=1_000_000 {
        check_route(t);
    }
    let mut t = i64::from(i32::MIN);
    while t <= i64::from(i32::MAX) {
        check_route(c_int::try_from(t).unwrap());
        t += 65_521;
    }
    for t in [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX] {
        check_route(t);
    }
}

#[test]
fn balance_bit_patterns() {
    // every pattern of the zeros and subnormals up to 2^-125, the values
    // from 0.5 up to 2.0 and the infinities and NaNs, of both signs
    for top in [0x00u32, 0x3f, 0x7f, 0x80, 0xbf, 0xff] {
        for low in 0..1u32 << 24 {
            check_balance(top << 24 | low);
        }
    }
    // and every 251st pattern of the rest
    for b in (0..=u32::MAX).step_by(251) {
        check_balance(b);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn full_range(
        a in any::<u32>(),
        b in any::<u32>(),
        m in any::<i64>(),
        t in any::<i32>(),
        f in any::<u32>(),
        x in any::<bool>(),
        y in any::<bool>(),
    ) {
        check_changes(a, b);
        check_changes(a, a);
        check_has_audio(a);
        check_mixers_store(m);
        check_push_to_logs(a, x);
        check_route(t);
        check_balance(f);
        check_flags(x, y);
    }
}
