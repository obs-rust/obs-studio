//! Tier 3: the Rust C ABI shim and safe core behave exactly like the
//! original C, compiled as an oracle.
//!
//! No intentional differences from C.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use obs_c_oracle::graphics_math::{self as c, OracleAxisAng, OracleQuat};
use obs_graphics::axisang::AxisAng;
use obs_graphics::ffi::axisang::{self as rs, axisang};
use obs_graphics::ffi::quat::quat;
use obs_graphics::quat::Quat;
use proptest::prelude::*;

/// Bit-exact equality, except that any NaN equals any NaN.
fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn same_aa(r: axisang, c: OracleAxisAng) -> bool {
    same(r.x, c.x) && same(r.y, c.y) && same(r.z, c.z) && same(r.w, c.w)
}

fn check(q: [f32; 4]) -> Result<(), TestCaseError> {
    let rq = quat {
        x: q[0],
        y: q[1],
        z: q[2],
        w: q[3],
    };
    let cq = OracleQuat {
        x: q[0],
        y: q[1],
        z: q[2],
        w: q[3],
    };
    let mut rd = axisang::default();
    let mut cd = OracleAxisAng::default();
    // SAFETY: all pointers refer to live, properly aligned locals.
    unsafe {
        rs::axisang_from_quat(&mut rd, &rq);
        c::oracle_axisang_from_quat(&mut cd, &cq);
    }
    prop_assert!(same_aa(rd, cd), "shim {rd:?} != C {cd:?} for {q:?}");
    let core: axisang = AxisAng::from_quat(Quat::new(q[0], q[1], q[2], q[3])).into();
    prop_assert!(same_aa(core, cd), "core {core:?} != C {cd:?} for {q:?}");
    Ok(())
}

/// Any bit pattern, so NaNs, infinities, zeros and subnormals all appear.
fn any_bits() -> impl Strategy<Value = f32> {
    any::<u32>().prop_map(f32::from_bits)
}

/// Unit quaternions, the inputs libobs actually passes.
fn unit_quat() -> impl Strategy<Value = [f32; 4]> {
    [-1.0f32..1.0, -1.0f32..1.0, -1.0f32..1.0, -1.0f32..1.0].prop_filter_map("nonzero", |q| {
        let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
        (n > 1e-3).then(|| q.map(|v| v / n))
    })
}

/// Vector parts whose squared length lands near `EPSILON` (1e-4), where the
/// zero branch switches on. A component of 0.01 alone squares to 1e-4.
fn near_epsilon() -> impl Strategy<Value = [f32; 4]> {
    let c = prop_oneof![
        Just(0.0f32),
        -0.0101f32..0.0101,
        (-3i32..=3).prop_map(|k| 0.01 + k as f32 * f32::EPSILON * 0.01),
    ];
    (c.clone(), c.clone(), c, -1.0f32..=1.0).prop_map(|(x, y, z, w)| [x, y, z, w])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn from_quat_matches_c_oracle_any_bits(q in [any_bits(), any_bits(), any_bits(), any_bits()]) {
        check(q)?;
    }

    #[test]
    fn from_quat_matches_c_oracle_unit(q in unit_quat()) {
        check(q)?;
    }

    #[test]
    fn from_quat_matches_c_oracle_near_epsilon(q in near_epsilon()) {
        check(q)?;
    }
}

/// The epsilon boundary itself: a squared length of exactly `EPSILON` is
/// still "close to zero" in C (`<=`), and the next float up is not.
#[test]
fn epsilon_boundary_matches_c_oracle() {
    let eps = 1e-4f32;
    for len in [
        eps,
        f32::from_bits(eps.to_bits() + 1),
        f32::from_bits(eps.to_bits() - 1),
    ] {
        // A quaternion whose x*x is exactly `len`, when sqrt rounds back.
        let x = len.sqrt();
        check([x, 0.0, 0.0, 0.5]).unwrap();
    }
}

/// The cases from `test/cmocka/test_axisang.c`, run against the oracle so
/// the C test's expectations are checked on every platform.
#[test]
fn cmocka_cases_match_c_oracle() {
    let half = core::f32::consts::FRAC_1_SQRT_2;
    let cases = [
        // 90 degrees about z: (0, 0, sin 45, cos 45)
        ([0.0, 0.0, half, half], Some([0.0, 0.0, 1.0])),
        // identity: no rotation, all zeros
        ([0.0, 0.0, 0.0, 1.0], None),
        // 180 degrees about x
        ([1.0, 0.0, 0.0, 0.0], Some([1.0, 0.0, 0.0])),
    ];
    for (q, axis) in cases {
        let mut cd = OracleAxisAng::default();
        // SAFETY: all pointers refer to live, properly aligned locals.
        unsafe {
            c::oracle_axisang_from_quat(
                &mut cd,
                &OracleQuat {
                    x: q[0],
                    y: q[1],
                    z: q[2],
                    w: q[3],
                },
            );
        }
        match axis {
            Some(a) => {
                for (got, want) in [cd.x, cd.y, cd.z].into_iter().zip(a) {
                    assert!((got - want).abs() < 1e-6, "axis {cd:?} for {q:?}");
                }
                assert!((cd.w - q[3].acos() * 2.0).abs() < 1e-6, "angle for {q:?}");
            }
            None => assert_eq!([cd.x, cd.y, cd.z, cd.w], [0.0; 4]),
        }
        check(q).unwrap();
    }
}
