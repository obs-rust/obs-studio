//! Tier 1: `obs_core::source_load`, the source load migration and value rules
//! of `obs_load_source_type()` (`libobs/obs-source-load.c.h`).
//!
//! `steps_*` port `test/cmocka/test_source_load_migration_steps.c` 1:1: its
//! cases under the same names, in the same order, with the same values, the
//! "characterized, not endorsed" ones included. `edge_*` add proptests over
//! full-range `u32` versions and caps, `i64` saved values, and `f64` values
//! taken as bit patterns (any NaN payload, subnormals, both zeros) and as
//! `f32` bit patterns widened (every float exactly), each against the rule
//! written out independently.

use obs_core::source_load::{
    ALL_MIXERS, DEINTERLACE_DISABLE, MONITOR_BY_DEFAULT, MONITOR_ONLY_BEFORE,
    MONITORING_ENABLED_BEFORE, MONITORING_MONITOR_ONLY, MONITORING_NONE, MonitoringPlan, balance,
    deinterlace_mode, delay, monitoring_migration, prev_ver, semantic_version, volume,
};
use proptest::prelude::*;

const fn v(major: u8, minor: u8, patch: u16) -> u32 {
    semantic_version(major, minor, patch)
}

const V23_2_2: u32 = v(23, 2, 2);
const V33_0_0: u32 = v(33, 0, 0);
const CAP: u32 = MONITOR_BY_DEFAULT;
const TWO_32: i64 = 1 << 32;

/// `got` and `expected` the same float, bit for bit (so -0.0 is not 0.0);
/// any NaN matches any NaN.
fn check_float(what: &str, input: f64, got: f32, expected: f32) {
    if expected.is_nan() {
        assert!(got.is_nan(), "{what}({input:e}): got {got:e}, expected NaN");
    } else {
        assert_eq!(
            got.to_bits(),
            expected.to_bits(),
            "{what}({input:e}): got {got:e}, expected {expected:e}"
        );
    }
}

fn expect_plan(
    prev: u32,
    caps: u32,
    ty: i64,
    set_mixers: bool,
    store_enabled: bool,
    enabled: bool,
) {
    assert_eq!(
        monitoring_migration(prev, caps, ty),
        MonitoringPlan {
            set_mixers,
            mixers: if set_mixers { 0x3F } else { 0 },
            store_enabled,
            enabled,
        },
        "migration(prev_ver {prev:#010x}, caps {caps:#010x}, type {ty})"
    );
}

fn expect_volume(input: f64, expected: f32) {
    check_float("volume", input, volume(input), expected);
}

fn expect_balance(input: f64, expected: f32) {
    check_float("balance", input, balance(input), expected);
}

const NANS: [u64; 7] = [
    0x7ff8_0000_0000_0000,
    0xfff8_0000_0000_0000,
    0x7ff0_0000_0000_0001,
    0x7ff4_0000_0000_0000,
    0xfff0_0000_0000_0001,
    0x7fff_ffff_ffff_ffff,
    0xffff_ffff_ffff_ffff,
];

fn next_up(x: f64) -> f64 {
    x.next_up()
}

fn next_down(x: f64) -> f64 {
    x.next_down()
}

const FLT_MAX: f64 = f32::MAX as f64;

// ------------------------------------------------------------------------
// constants

#[test]
fn steps_constants() {
    assert_eq!(MONITOR_ONLY_BEFORE, 0x1702_0002);
    assert_eq!(MONITOR_ONLY_BEFORE, V23_2_2);
    assert_eq!(MONITORING_ENABLED_BEFORE, 0x2100_0000);
    assert_eq!(MONITORING_ENABLED_BEFORE, V33_0_0);
    assert_eq!(MONITOR_BY_DEFAULT, 0x800);
    assert_eq!(MONITORING_NONE, 0);
    assert_eq!(MONITORING_MONITOR_ONLY, 1);
    assert_eq!(ALL_MIXERS, 0x3F);
    assert_eq!(DEINTERLACE_DISABLE, 0);
}

// ------------------------------------------------------------------------
// prev_ver

#[test]
fn steps_prev_ver() {
    assert_eq!(prev_ver(0), 0);
    assert_eq!(prev_ver(1), 1);
    assert_eq!(prev_ver(i64::from(V23_2_2) - 1), V23_2_2 - 1);
    assert_eq!(prev_ver(i64::from(V23_2_2)), V23_2_2);
    assert_eq!(prev_ver(i64::from(V33_0_0)), V33_0_0);
    assert_eq!(prev_ver(i64::from(i32::MAX)), 0x7fff_ffff);
    assert_eq!(prev_ver(i64::from(i32::MAX) + 1), 0x8000_0000);
    assert_eq!(prev_ver(i64::from(u32::MAX)), u32::MAX);
}

/// Characterized, not endorsed: an int64 past `UINT32_MAX` keeps its low 32
/// bits, and a negative one wraps.
#[test]
fn steps_prev_ver_truncates() {
    assert_eq!(prev_ver(i64::from(u32::MAX) + 1), 0);
    assert_eq!(prev_ver(i64::from(u32::MAX) + 2), 1);
    assert_eq!(prev_ver(TWO_32 | i64::from(v(23, 2, 1))), v(23, 2, 1));
    assert_eq!(prev_ver(TWO_32 | i64::from(V33_0_0)), V33_0_0);
    assert_eq!(prev_ver(-1), u32::MAX);
    assert_eq!(prev_ver(-TWO_32), 0);
    assert_eq!(prev_ver(i64::MIN), 0);
    assert_eq!(prev_ver(i64::MAX), u32::MAX);
}

// ------------------------------------------------------------------------
// monitoring migration

#[test]
fn steps_migration_23_2_2_edges() {
    let below = [
        0,
        1,
        v(22, 0, 3),
        v(23, 0, 0),
        v(23, 2, 0),
        v(23, 2, 1),
        V23_2_2 - 1,
    ];
    let at_or_above = [V23_2_2, V23_2_2 + 1, v(23, 2, 3), v(24, 0, 0), v(30, 2, 3)];
    for prev in below {
        expect_plan(prev, CAP, 0, true, true, true);
        expect_plan(prev, CAP, 2, true, true, true);
        expect_plan(prev, 0, 0, false, true, false);
        expect_plan(prev, 0, 1, false, true, true);
    }
    for prev in at_or_above {
        expect_plan(prev, CAP, 0, false, true, false);
        expect_plan(prev, CAP, 2, false, true, true);
        expect_plan(prev, 0, 0, false, true, false);
    }
}

#[test]
fn steps_migration_33_0_0_edges() {
    let below = [V23_2_2, v(32, 255, 65535), v(32, 0, 0), V33_0_0 - 1];
    let at_or_above = [
        V33_0_0,
        V33_0_0 + 1,
        v(33, 0, 1),
        v(33, 1, 0),
        v(34, 0, 0),
        0x7fff_ffff,
        0x8000_0000,
        u32::MAX - 1,
        u32::MAX,
    ];
    for prev in below {
        expect_plan(prev, 0, 0, false, true, false);
        expect_plan(prev, 0, 1, false, true, true);
        expect_plan(prev, 0, 2, false, true, true);
        expect_plan(prev, CAP, 0, false, true, false);
    }
    for prev in at_or_above {
        for ty in -1..=3 {
            expect_plan(prev, 0, ty, false, false, false);
            expect_plan(prev, CAP, ty, false, false, false);
            expect_plan(prev, u32::MAX, ty, false, false, false);
        }
    }
}

#[test]
fn steps_migration_prev_ver_ends() {
    expect_plan(0, CAP, 0, true, true, true);
    expect_plan(0, 0, 0, false, true, false);
    expect_plan(0, 0, 2, false, true, true);
    expect_plan(u32::MAX, CAP, 0, false, false, false);
    expect_plan(u32::MAX, 0, 2, false, false, false);
    // the truncated int64 above UINT32_MAX (characterized, not endorsed)
    expect_plan(
        prev_ver(TWO_32 | i64::from(v(23, 2, 1))),
        CAP,
        0,
        true,
        true,
        true,
    );
    expect_plan(prev_ver(i64::from(u32::MAX) + 1), 0, 2, false, true, true);
}

#[test]
fn steps_migration_each_type() {
    let types: [(i64, bool); 7] = [
        (0, false),
        (1, true),
        (2, true),
        (3, true),
        (-1, true),
        (i64::from(i32::MAX), true),
        (i64::from(i32::MIN), true),
    ];
    let caps_list = [0, CAP, !CAP, u32::MAX];
    for (ty, enabled) in types {
        for caps in caps_list {
            let cap = caps & CAP != 0;
            expect_plan(v(20, 0, 0), caps, ty, cap, true, cap || enabled);
            expect_plan(v(30, 0, 0), caps, ty, false, true, enabled);
        }
    }
}

/// Characterized, not endorsed: the saved int64 type is read as an int.
#[test]
fn steps_migration_type_truncates() {
    expect_plan(v(30, 0, 0), 0, TWO_32, false, true, false);
    expect_plan(v(30, 0, 0), 0, TWO_32 + 1, false, true, true);
    expect_plan(v(30, 0, 0), 0, i64::from(u32::MAX), false, true, true);
    expect_plan(v(30, 0, 0), 0, i64::MIN, false, true, false);
    expect_plan(v(30, 0, 0), 0, i64::MAX, false, true, true);
    expect_plan(v(30, 0, 0), 0, -TWO_32, false, true, false);
    expect_plan(v(20, 0, 0), CAP, TWO_32, true, true, true);
}

/// The rule written out: `ty` read as its low 32 bits.
fn rule(prev: u32, caps: u32, ty: i64) -> MonitoringPlan {
    let monitor_only = prev < V23_2_2 && caps & CAP != 0;
    let low = ty.to_le_bytes();
    let as_int = i32::from_le_bytes([low[0], low[1], low[2], low[3]]);
    let ty = if monitor_only { 1 } else { as_int };
    let store = prev < V33_0_0;
    MonitoringPlan {
        set_mixers: monitor_only,
        mixers: if monitor_only { 0x3F } else { 0 },
        store_enabled: store,
        enabled: store && ty != 0,
    }
}

#[test]
fn steps_migration_cross() {
    let versions = [
        0,
        1,
        v(23, 2, 1),
        V23_2_2,
        V23_2_2 + 1,
        v(32, 0, 0),
        V33_0_0 - 1,
        V33_0_0,
        V33_0_0 + 1,
        0x7fff_ffff,
        0x8000_0000,
        u32::MAX,
    ];
    let caps_list = [0, 1, CAP, CAP | 1, CAP >> 1, CAP << 1, !CAP, u32::MAX];
    let types = [
        0,
        1,
        2,
        3,
        -1,
        i64::from(i32::MAX),
        i64::from(i32::MIN),
        i64::MIN,
        i64::MAX,
        TWO_32,
        TWO_32 + 2,
    ];
    let mut n = 0;
    for prev in versions {
        for caps in caps_list {
            for ty in types {
                let want = rule(prev, caps, ty);
                expect_plan(
                    prev,
                    caps,
                    ty,
                    want.set_mixers,
                    want.store_enabled,
                    want.enabled,
                );
                n += 1;
            }
        }
    }
    assert_eq!(n, 12 * 8 * 11);
}

// ------------------------------------------------------------------------
// volume

#[test]
fn steps_volume_in_range() {
    expect_volume(0.0, 0.0);
    expect_volume(0.5, 0.5);
    expect_volume(1.0, 1.0);
    expect_volume(2.0, 2.0);
    expect_volume(20.0, 20.0);
    expect_volume(0.1, 0.1);
    // rounds to nearest, ties to even
    expect_volume(1.0 + f64::powi(2.0, -24), 1.0);
    expect_volume(
        1.0 + f64::powi(2.0, -24) + f64::powi(2.0, -40),
        1.0 + f32::powi(2.0, -23),
    );
    expect_volume(1.0 + 3.0 * f64::powi(2.0, -24), 1.0 + f32::powi(2.0, -22));
    // the smallest float subnormal; below half of it is 0
    expect_volume(f64::powi(2.0, -149), f32::from_bits(1));
    expect_volume(f64::powi(2.0, -151), 0.0);
    expect_volume(f64::from(f32::MIN_POSITIVE), f32::MIN_POSITIVE);
    expect_volume(f64::MIN_POSITIVE, 0.0);
    expect_volume(next_up(0.0), 0.0);
}

#[test]
fn steps_volume_negative() {
    expect_volume(-1.0, 0.0);
    expect_volume(-0.5, 0.0);
    expect_volume(next_down(0.0), 0.0);
    expect_volume(-f64::MIN_POSITIVE, 0.0);
    expect_volume(-FLT_MAX, 0.0);
    expect_volume(f64::MIN, 0.0);
    expect_volume(f64::NEG_INFINITY, 0.0);
    // characterized, not endorsed: -0.0 is not below 0
    expect_volume(-0.0, -0.0);
}

#[test]
fn steps_volume_past_float() {
    expect_volume(FLT_MAX, f32::MAX);
    expect_volume(next_down(FLT_MAX), f32::MAX);
    expect_volume(next_up(FLT_MAX), f32::INFINITY);
    // characterized, not endorsed: float rounding would take it back to
    // FLT_MAX
    expect_volume(FLT_MAX + f64::powi(2.0, 103), f32::INFINITY);
    expect_volume(3.5e38, f32::INFINITY);
    expect_volume(1e300, f32::INFINITY);
    expect_volume(f64::MAX, f32::INFINITY);
    expect_volume(f64::INFINITY, f32::INFINITY);
}

#[test]
fn steps_volume_nan() {
    for bits in NANS {
        assert!(
            volume(f64::from_bits(bits)).is_nan(),
            "volume(NaN {bits:#018x})"
        );
    }
}

// ------------------------------------------------------------------------
// balance

#[test]
fn steps_balance_in_range() {
    expect_balance(0.0, 0.0);
    expect_balance(0.25, 0.25);
    expect_balance(0.5, 0.5);
    expect_balance(1.0, 1.0);
    expect_balance(0.1, 0.1);
    expect_balance(next_down(1.0), 1.0);
    expect_balance(1.0 - f64::powi(2.0, -24), 1.0 - f32::powi(2.0, -24));
    expect_balance(f64::powi(2.0, -149), f32::from_bits(1));
    expect_balance(next_up(0.0), 0.0);
}

#[test]
fn steps_balance_out_of_range() {
    expect_balance(next_up(1.0), 1.0);
    expect_balance(1.5, 1.0);
    expect_balance(2.0, 1.0);
    expect_balance(FLT_MAX, 1.0);
    expect_balance(f64::MAX, 1.0);
    expect_balance(f64::INFINITY, 1.0);
    expect_balance(next_down(0.0), 0.0);
    expect_balance(-0.5, 0.0);
    expect_balance(f64::MIN, 0.0);
    expect_balance(f64::NEG_INFINITY, 0.0);
    // characterized, not endorsed
    expect_balance(-0.0, -0.0);
}

#[test]
fn steps_balance_nan() {
    for bits in [NANS[0], NANS[1], NANS[2], NANS[6]] {
        assert!(
            balance(f64::from_bits(bits)).is_nan(),
            "balance(NaN {bits:#018x})"
        );
    }
}

// ------------------------------------------------------------------------
// delays

#[test]
fn steps_delay() {
    assert_eq!(delay(0), 0);
    assert_eq!(delay(1), 1);
    assert_eq!(delay(200), 200);
    assert_eq!(delay(i64::from(i32::MAX)), 0x7fff_ffff);
    assert_eq!(delay(i64::from(u32::MAX)), u64::from(u32::MAX));
    assert_eq!(delay(TWO_32), 1 << 32);
    assert_eq!(delay(i64::MAX), 0x7fff_ffff_ffff_ffff);
    assert_eq!(delay(-1), 0);
    assert_eq!(delay(-200), 0);
    assert_eq!(delay(i64::from(i32::MIN)), 0);
    assert_eq!(delay(i64::MIN + 1), 0);
    assert_eq!(delay(i64::MIN), 0);
}

// ------------------------------------------------------------------------
// deinterlace mode

#[test]
fn steps_deinterlace_mode_in_range() {
    for mode in 0..=8 {
        assert_eq!(deinterlace_mode(mode), i32::try_from(mode).unwrap());
    }
    assert_eq!(deinterlace_mode(9), 9);
    assert_eq!(deinterlace_mode(-1), -1);
    assert_eq!(deinterlace_mode(i64::from(i32::MAX)), i32::MAX);
    assert_eq!(deinterlace_mode(i64::from(i32::MIN)), i32::MIN);
}

#[test]
fn steps_deinterlace_mode_out_of_range() {
    assert_eq!(deinterlace_mode(i64::from(i32::MAX) + 1), 0);
    assert_eq!(deinterlace_mode(i64::from(i32::MIN) - 1), 0);
    assert_eq!(deinterlace_mode(i64::from(u32::MAX)), 0);
    assert_eq!(deinterlace_mode(TWO_32 + 1), 0);
    assert_eq!(deinterlace_mode(TWO_32 + 5), 0);
    assert_eq!(deinterlace_mode(-TWO_32 + 5), 0);
    assert_eq!(deinterlace_mode(i64::MAX), 0);
    assert_eq!(deinterlace_mode(i64::MIN), 0);
}

// ------------------------------------------------------------------------
// edges

/// Versions weighted to the two bounds and the ends, else any `u32`.
fn any_version() -> impl Strategy<Value = u32> {
    prop_oneof![
        any::<u32>(),
        (V23_2_2 - 2..=V23_2_2 + 2),
        (V33_0_0 - 2..=V33_0_0 + 2),
        Just(0u32),
        Just(u32::MAX),
    ]
}

/// Saved int64s weighted to the int and the int64 ends and small values,
/// else any.
fn any_saved() -> impl Strategy<Value = i64> {
    prop_oneof![
        any::<i64>(),
        -4i64..=4,
        Just(i64::from(i32::MAX)),
        Just(i64::from(i32::MAX) + 1),
        Just(i64::from(i32::MIN)),
        Just(i64::from(i32::MIN) - 1),
        Just(i64::from(u32::MAX)),
        Just(TWO_32),
        Just(i64::MIN),
        Just(i64::MAX),
    ]
}

/// Doubles as bit patterns: any `f64` (every NaN payload, subnormals, both
/// zeros), any `f32` widened (each exactly a float), and around the bounds.
fn any_double() -> impl Strategy<Value = f64> {
    prop_oneof![
        any::<u64>().prop_map(f64::from_bits),
        any::<u32>().prop_map(|bits| f64::from(f32::from_bits(bits))),
        (-64i64..=64).prop_map(|steps| {
            let mut x = FLT_MAX;
            for _ in 0..steps.unsigned_abs() {
                x = if steps < 0 { next_down(x) } else { next_up(x) };
            }
            x
        }),
        (-64i64..=64).prop_map(|steps| {
            let mut x = 1.0f64;
            for _ in 0..steps.unsigned_abs() {
                x = if steps < 0 { next_down(x) } else { next_up(x) };
            }
            x
        }),
        (-64i64..=64).prop_map(|steps| {
            let mut x = 0.0f64;
            for _ in 0..steps.unsigned_abs() {
                x = if steps < 0 { next_down(x) } else { next_up(x) };
            }
            x
        }),
    ]
}

/// The nearest `f32` of a finite `f64` in float's range (round to nearest,
/// ties to even), worked out from the bits instead of a cast.
fn nearest_f32(x: f64) -> f32 {
    let target = x.abs();
    // the largest finite float not above |x| (a binary search over the bits
    // of the non-negative floats, which order as their bits do)
    let (mut lo, mut hi) = (0u32, 0x7f7f_ffffu32);
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if f64::from(f32::from_bits(mid)) <= target {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let down = f32::from_bits(lo);
    let up = f32::from_bits(lo + 1); // infinity past FLT_MAX
    let below = target - f64::from(down);
    let above = f64::from(up) - target;
    let pick = if below < above || (below == above && lo % 2 == 0) {
        down
    } else {
        up
    };
    if x.is_sign_negative() { -pick } else { pick }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 4096, ..ProptestConfig::default() })]

    /// The C's `(uint32_t)` of an int64: its low 32 bits.
    #[test]
    fn edge_prev_ver_low_bits(stored in any_saved()) {
        let low = stored.to_le_bytes();
        prop_assert_eq!(prev_ver(stored), u32::from_le_bytes([low[0], low[1], low[2], low[3]]));
    }

    /// The migration over any version, caps and type, against the rule.
    #[test]
    fn edge_migration_rule(prev in any_version(), caps in any::<u32>(), with_cap in any::<bool>(), ty in any_saved()) {
        let caps = if with_cap { caps | CAP } else { caps & !CAP };
        prop_assert_eq!(monitoring_migration(prev, caps, ty), rule(prev, caps, ty));
    }

    /// The volume: NaN stays NaN; negative is 0; past FLT_MAX is infinity;
    /// else the nearest float.
    #[test]
    fn edge_volume(x in any_double()) {
        let got = volume(x);
        if x.is_nan() {
            prop_assert!(got.is_nan());
        } else if x < 0.0 {
            prop_assert_eq!(got.to_bits(), 0);
        } else if x > FLT_MAX {
            prop_assert_eq!(got, f32::INFINITY);
        } else {
            prop_assert_eq!(got.to_bits(), nearest_f32(x).to_bits(), "{:e}", x);
            prop_assert!(got.is_finite());
        }
    }

    /// The balance: NaN stays NaN; else in 0 to 1 (or -0.0) as the nearest
    /// float.
    #[test]
    fn edge_balance(x in any_double()) {
        let got = balance(x);
        if x.is_nan() {
            prop_assert!(got.is_nan());
        } else if x < 0.0 {
            prop_assert_eq!(got.to_bits(), 0);
        } else if x > 1.0 {
            prop_assert_eq!(got.to_bits(), 1.0f32.to_bits());
        } else {
            prop_assert_eq!(got.to_bits(), nearest_f32(x).to_bits(), "{:e}", x);
            prop_assert!((-0.0..=1.0).contains(&got));
        }
    }

    /// A delay: itself when not negative, else 0.
    #[test]
    fn edge_delay(d in any_saved()) {
        let want = if d < 0 { 0 } else { d.unsigned_abs() };
        prop_assert_eq!(delay(d), want);
    }

    /// The deinterlace mode: itself in the int range, else disabled, never
    /// the low 32 bits of an int64 past it.
    #[test]
    fn edge_deinterlace_mode(m in any_saved()) {
        let got = deinterlace_mode(m);
        if m < i64::from(i32::MIN) || m > i64::from(i32::MAX) {
            prop_assert_eq!(got, DEINTERLACE_DISABLE);
        } else {
            prop_assert_eq!(i64::from(got), m);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 4096, ..ProptestConfig::default() })]

    /// `MAKE_SEMANTIC_VERSION()` written out, over every major, minor and
    /// patch.
    #[test]
    fn edge_semantic_version(major in any::<u8>(), minor in any::<u8>(), patch in any::<u16>()) {
        prop_assert_eq!(
            semantic_version(major, minor, patch),
            u32::from(major) * 0x0100_0000 + u32::from(minor) * 0x1_0000 + u32::from(patch)
        );
    }
}
