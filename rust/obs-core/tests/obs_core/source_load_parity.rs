//! Tier 2 layout and Tier 3 parity: the shims of `obs_core::ffi::source_load`
//! (and the safe core under them) against the original
//! `libobs/obs-source-load.c.h`, compiled as `oracle_obs_source_load_*`
//! (obs-c-oracle, oracle/source_load.c).
//!
//! Every output is compared exactly: the monitoring plan field by field, the
//! version, delay and mode as integers, and the volume and balance floats
//! bit for bit (so `-0.0` is not `0.0`). A NaN must give a NaN on every side;
//! its payload and sign are not compared, since Rust leaves them unspecified
//! for an `f64` to `f32` cast and the setters map every NaN alike.
//!
//! Runs: the constants and the layout of the struct; the migration
//! exhaustively over the cap (each of the 32 caps bits alone, none, all, and
//! all but the cap) x the types (-3 to 5, the `int` and `int64` ends and
//! around `2^32`) x versions at and around both bounds, every power of two
//! and the ends; every major.minor version (65,536) with five patches, with
//! and without the cap, for each monitoring type; the value rules over their
//! edges, every 4093rd `f32` bit pattern widened, every NaN class and every
//! `int64` within 64 of the `int` and `int64` ends; and proptests over `u32`
//! and `i64` versions, any caps and types, `f64` bit patterns and `f32` bit
//! patterns widened.
//!
//! No intentional differences. `test_source_load_migration_steps` runs the C
//! test against the C; the real-libobs source load tests run against the
//! hybrid libobs, where these shims replace the C.

use core::mem::{align_of, offset_of, size_of};

use obs_c_oracle::source_load::{
    CONSTANT_NAMES, LAYOUT_NAMES, OracleMonitoring, oracle_obs_source_load_balance,
    oracle_obs_source_load_deinterlace_mode, oracle_obs_source_load_delay,
    oracle_obs_source_load_monitoring_migration, oracle_obs_source_load_prev_ver,
    oracle_obs_source_load_volume, oracle_source_load_constant, oracle_source_load_layout,
};
use obs_core::ffi::source_load::{
    obs_source_load_balance, obs_source_load_deinterlace_mode, obs_source_load_delay,
    obs_source_load_monitoring, obs_source_load_monitoring_migration, obs_source_load_prev_ver,
    obs_source_load_volume,
};
use obs_core::source_load::{
    self as load, ALL_MIXERS, DEINTERLACE_DISABLE, MONITOR_BY_DEFAULT, MONITOR_ONLY_BEFORE,
    MONITORING_ENABLED_BEFORE, MONITORING_MONITOR_ONLY, MONITORING_NONE, semantic_version,
};
use proptest::prelude::*;

const CAP: u32 = MONITOR_BY_DEFAULT;
const TWO_32: i64 = 1 << 32;

fn oracle_plan(p: OracleMonitoring) -> obs_source_load_monitoring {
    obs_source_load_monitoring {
        set_mixers: p.set_mixers,
        mixers: p.mixers,
        store_enabled: p.store_enabled,
        enabled: p.enabled,
    }
}

/// The shim, the C and the core on one migration; returns the plan.
fn check_migration(prev: u32, caps: u32, ty: i64) -> obs_source_load_monitoring {
    let rust = obs_source_load_monitoring_migration(prev, caps, ty);
    // SAFETY: a plain value function.
    let c = oracle_plan(unsafe { oracle_obs_source_load_monitoring_migration(prev, caps, ty) });
    assert_eq!(
        rust, c,
        "migration(prev_ver {prev:#010x}, caps {caps:#010x}, type {ty})"
    );
    assert_eq!(
        obs_source_load_monitoring::from(load::monitoring_migration(prev, caps, ty)),
        c,
        "core migration(prev_ver {prev:#010x}, caps {caps:#010x}, type {ty})"
    );
    c
}

fn check_prev_ver(stored: i64) -> u32 {
    let rust = obs_source_load_prev_ver(stored);
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_source_load_prev_ver(stored) };
    assert_eq!(rust, c, "prev_ver({stored})");
    assert_eq!(load::prev_ver(stored), c, "core prev_ver({stored})");
    c
}

fn same_float(a: f32, b: f32) -> bool {
    (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
}

fn check_volume(x: f64) -> f32 {
    let rust = obs_source_load_volume(x);
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_source_load_volume(x) };
    assert!(
        same_float(rust, c),
        "volume({x:e} = {:#018x}): {rust:e} vs C {c:e}",
        x.to_bits()
    );
    let core = load::volume(x);
    assert!(
        same_float(core, c),
        "core volume({x:e}): {core:e} vs C {c:e}"
    );
    assert_eq!(c.is_nan(), x.is_nan(), "volume({x:e}) NaN in, NaN out");
    c
}

fn check_balance(x: f64) -> f32 {
    let rust = obs_source_load_balance(x);
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_source_load_balance(x) };
    assert!(
        same_float(rust, c),
        "balance({x:e} = {:#018x}): {rust:e} vs C {c:e}",
        x.to_bits()
    );
    let core = load::balance(x);
    assert!(
        same_float(core, c),
        "core balance({x:e}): {core:e} vs C {c:e}"
    );
    assert_eq!(c.is_nan(), x.is_nan(), "balance({x:e}) NaN in, NaN out");
    c
}

fn check_delay(d: i64) -> u64 {
    let rust = obs_source_load_delay(d);
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_source_load_delay(d) };
    assert_eq!(rust, c, "delay({d})");
    assert_eq!(load::delay(d), c, "core delay({d})");
    c
}

fn check_mode(m: i64) -> i32 {
    let rust = obs_source_load_deinterlace_mode(m);
    // SAFETY: a plain value function.
    let c = unsafe { oracle_obs_source_load_deinterlace_mode(m) };
    assert_eq!(rust, c, "deinterlace_mode({m})");
    assert_eq!(load::deinterlace_mode(m), c, "core deinterlace_mode({m})");
    c
}

fn check_values_i64(v: i64) {
    check_prev_ver(v);
    check_delay(v);
    check_mode(v);
}

fn check_values_f64(x: f64) {
    check_volume(x);
    check_balance(x);
}

// ------------------------------------------------------------------------
// layout and constants

#[test]
fn parity_constants() {
    let ours: [i64; 7] = [
        i64::from(MONITOR_ONLY_BEFORE),
        i64::from(MONITORING_ENABLED_BEFORE),
        i64::from(MONITOR_BY_DEFAULT),
        i64::from(MONITORING_NONE),
        i64::from(MONITORING_MONITOR_ONLY),
        i64::from(ALL_MIXERS),
        i64::from(DEINTERLACE_DISABLE),
    ];
    for (which, (name, value)) in CONSTANT_NAMES.iter().zip(ours).enumerate() {
        // SAFETY: a plain value function.
        assert_eq!(
            unsafe { oracle_source_load_constant(which) },
            value,
            "{name}"
        );
    }
    // SAFETY: as above; out of range is -1.
    assert_eq!(
        unsafe { oracle_source_load_constant(CONSTANT_NAMES.len()) },
        -1
    );
    assert_eq!(MONITOR_ONLY_BEFORE, semantic_version(23, 2, 2));
    assert_eq!(MONITORING_ENABLED_BEFORE, semantic_version(33, 0, 0));
}

#[test]
fn parity_layout() {
    let ours: [usize; 6] = [
        size_of::<obs_source_load_monitoring>(),
        align_of::<obs_source_load_monitoring>(),
        offset_of!(obs_source_load_monitoring, set_mixers),
        offset_of!(obs_source_load_monitoring, mixers),
        offset_of!(obs_source_load_monitoring, store_enabled),
        offset_of!(obs_source_load_monitoring, enabled),
    ];
    for (which, (name, value)) in LAYOUT_NAMES.iter().zip(ours).enumerate() {
        // SAFETY: a plain value function.
        assert_eq!(unsafe { oracle_source_load_layout(which) }, value, "{name}");
    }
    // SAFETY: as above; out of range is usize::MAX.
    assert_eq!(
        unsafe { oracle_source_load_layout(LAYOUT_NAMES.len()) },
        usize::MAX
    );
    assert_eq!(size_of::<OracleMonitoring>(), ours[0]);
}

// ------------------------------------------------------------------------
// migration

/// Versions at and around both bounds, every power of two (and one below),
/// and the ends.
fn versions() -> Vec<u32> {
    let mut v = Vec::new();
    for bound in [MONITOR_ONLY_BEFORE, MONITORING_ENABLED_BEFORE] {
        for d in 0..=3 {
            v.extend([bound - d, bound + d]);
        }
    }
    for k in 0..32 {
        v.extend([1u32 << k, (1u32 << k) - 1]);
    }
    v.extend([
        0,
        1,
        2,
        3,
        u32::MAX,
        u32::MAX - 1,
        semantic_version(22, 0, 3),
        semantic_version(23, 2, 1),
        semantic_version(32, 255, 65535),
        semantic_version(33, 0, 1),
    ]);
    v.sort_unstable();
    v.dedup();
    v
}

/// Each caps bit alone, none, all, and all but the cap.
fn caps_sets() -> Vec<u32> {
    let mut c: Vec<u32> = (0..32).map(|k| 1u32 << k).collect();
    c.extend([0, u32::MAX, !CAP]);
    c
}

/// The monitoring types and values outside the enum: -3 to 5, the `int`
/// and `int64` ends, and around `2^32` either way.
fn types() -> Vec<i64> {
    let mut t: Vec<i64> = (-3..=5).collect();
    t.extend([
        i64::from(i32::MAX),
        i64::from(i32::MAX) + 1,
        i64::from(i32::MIN),
        i64::from(i32::MIN) - 1,
        i64::from(u32::MAX),
        i64::MIN,
        i64::MIN + 1,
        i64::MAX,
        i64::MAX - 1,
    ]);
    for d in -2..=3 {
        t.extend([TWO_32 + d, -TWO_32 + d]);
    }
    t
}

#[test]
fn parity_migration_exhaustive_caps_and_types() {
    let (vs, cs, ts) = (versions(), caps_sets(), types());
    let mut n = 0usize;
    let (mut mixers, mut stored, mut enabled) = (0usize, 0usize, 0usize);
    for &prev in &vs {
        for &caps in &cs {
            for &ty in &ts {
                let plan = check_migration(prev, caps, ty);
                mixers += usize::from(plan.set_mixers);
                stored += usize::from(plan.store_enabled);
                enabled += usize::from(plan.enabled);
                n += 1;
            }
        }
    }
    assert_eq!(n, vs.len() * cs.len() * ts.len());
    // every branch is reached
    assert!(mixers > 0 && stored > mixers && enabled > 0 && stored < n);
}

/// Every major.minor with patches 0 to 3 and 0xffff, with and without the
/// cap, for each type and one outside the enum.
#[test]
fn parity_migration_every_major_minor() {
    for major in 0..=255u8 {
        for minor in 0..=255u8 {
            for patch in [0u16, 1, 2, 3, 0xffff] {
                let prev = semantic_version(major, minor, patch);
                for caps in [0, CAP] {
                    for ty in [0i64, 1, 2, 3] {
                        check_migration(prev, caps, ty);
                    }
                }
            }
        }
    }
}

/// Saved int64 versions through `prev_ver()` into the migration, as
/// `obs_load_source_type()` chains them.
#[test]
fn parity_migration_from_saved_versions() {
    let mut saved: Vec<i64> = versions().into_iter().map(i64::from).collect();
    let more: Vec<i64> = saved
        .iter()
        .flat_map(|&v| [v + TWO_32, v - TWO_32, -v])
        .collect();
    saved.extend(more);
    saved.extend([i64::MIN, i64::MAX, -1]);
    for stored in saved {
        let prev = check_prev_ver(stored);
        for caps in [0, CAP] {
            for ty in [0i64, 1, 2] {
                check_migration(prev, caps, ty);
            }
        }
    }
}

// ------------------------------------------------------------------------
// value rules

fn double_edges() -> Vec<f64> {
    let flt_max = f64::from(f32::MAX);
    let mut v = vec![
        0.0,
        -0.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        0.5,
        0.25,
        0.1,
        2.0,
        -1.0,
        -0.5,
        f64::MAX,
        f64::MIN,
        f64::MIN_POSITIVE,
        -f64::MIN_POSITIVE,
        f64::from(f32::MIN_POSITIVE),
        flt_max + f64::powi(2.0, 103),
        flt_max + f64::powi(2.0, 104),
        f64::powi(2.0, -149),
        f64::powi(2.0, -150),
        f64::powi(2.0, -151),
        1.0 + f64::powi(2.0, -24),
        1.0 + 3.0 * f64::powi(2.0, -24),
    ];
    for centre in [0.0, 1.0, flt_max, -flt_max, -1.0] {
        let (mut up, mut down) = (centre, centre);
        for _ in 0..64 {
            up = up.next_up();
            down = down.next_down();
            v.extend([up, down]);
        }
        v.push(centre);
    }
    v
}

const NANS: [u64; 10] = [
    0x7ff8_0000_0000_0000,
    0xfff8_0000_0000_0000,
    0x7ff0_0000_0000_0001,
    0xfff0_0000_0000_0001,
    0x7ff4_0000_0000_0000,
    0x7ff0_0000_2000_0000,
    0x7ff0_0000_1fff_ffff,
    0x7fff_ffff_ffff_ffff,
    0xffff_ffff_ffff_ffff,
    0x7ff8_0000_0000_0001,
];

#[test]
fn parity_values_edges() {
    for x in double_edges() {
        check_values_f64(x);
    }
    for bits in NANS {
        check_values_f64(f64::from_bits(bits));
    }
    let mut ints: Vec<i64> = Vec::new();
    for centre in [
        0,
        i64::from(i32::MAX),
        i64::from(i32::MIN),
        i64::from(u32::MAX),
        TWO_32,
        -TWO_32,
    ] {
        for d in -64..=64 {
            ints.push(centre + d);
        }
    }
    for d in 0..=64 {
        ints.extend([i64::MIN + d, i64::MAX - d]);
    }
    for v in ints {
        check_values_i64(v);
    }
}

/// Every 4093rd `f32` bit pattern, widened (each exactly a float, both
/// signs, subnormals, infinities and NaNs), and its neighbours as doubles.
#[test]
fn parity_values_f32_sweep() {
    let mut bits = 0u32;
    loop {
        let x = f64::from(f32::from_bits(bits));
        check_values_f64(x);
        check_values_f64(x.next_up());
        check_values_f64(x.next_down());
        match bits.checked_add(4093) {
            Some(next) => bits = next,
            None => break,
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 8192, ..ProptestConfig::default() })]

    #[test]
    fn parity_migration_any(prev in any::<u32>(), caps in any::<u32>(), with_cap in any::<bool>(), ty in any::<i64>()) {
        let caps = if with_cap { caps | CAP } else { caps & !CAP };
        check_migration(prev, caps, ty);
        check_migration(prev, caps, ty & 3);
    }

    #[test]
    fn parity_migration_near_bounds(
        bound in prop_oneof![Just(MONITOR_ONLY_BEFORE), Just(MONITORING_ENABLED_BEFORE)],
        d in -0x0100_0000i64..=0x0100_0000,
        caps in any::<u32>(),
        ty in -4i64..=4,
    ) {
        let prev = u32::try_from(i64::from(bound) + d).unwrap();
        check_migration(prev, caps, ty);
        check_migration(prev, caps | CAP, ty);
    }

    #[test]
    fn parity_saved_version_any(stored in any::<i64>(), caps in any::<u32>(), ty in any::<i64>()) {
        let prev = check_prev_ver(stored);
        check_migration(prev, caps, ty);
    }

    #[test]
    fn parity_values_f64_bits(bits in any::<u64>()) {
        check_values_f64(f64::from_bits(bits));
    }

    #[test]
    fn parity_values_f32_bits(bits in any::<u32>()) {
        let x = f64::from(f32::from_bits(bits));
        check_values_f64(x);
        check_values_f64(x.next_up());
        check_values_f64(x.next_down());
    }

    #[test]
    fn parity_values_i64(v in any::<i64>(), small in -70i64..=70) {
        check_values_i64(v);
        check_values_i64(small);
        check_values_i64(v >> 31);
        check_values_i64(v >> 32);
    }
}
