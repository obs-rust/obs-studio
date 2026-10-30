//! Tier 3: the Rust C ABI shim and safe core behave like the original C,
//! compiled as an oracle.
//!
//! `polar_to_cart` and `cart_to_polar` set `w` to 0. C copies that lane
//! uninitialized (except the within-`EPSILON` zeroing path of
//! `cart_to_polar`), so those two compare xyz only.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use obs_c_oracle::graphics_math::{self as c, OracleVec3};
use obs_c_oracle::vec2::OracleVec2;
use obs_graphics::ffi::math_extra::{self as rs};
use obs_graphics::ffi::vec2::vec2;
use obs_graphics::ffi::vec3::vec3;
use obs_graphics::math_extra::{self, calc_torque};
use obs_graphics::vec2::Vec2;
use obs_graphics::vec3::Vec3;
use proptest::prelude::*;

/// Bit-exact equality, except that any NaN equals any NaN.
fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn o3(v: Vec3) -> OracleVec3 {
    OracleVec3 {
        x: v.x,
        y: v.y,
        z: v.z,
        w: v.w,
    }
}

fn r3(v: Vec3) -> vec3 {
    v.into()
}

fn xyz(r: vec3, c: OracleVec3) -> bool {
    same(r.x, c.x) && same(r.y, c.y) && same(r.z, c.z)
}

fn same3(r: vec3, c: OracleVec3) -> bool {
    xyz(r, c) && same(r.w, c.w)
}

fn check_polar(v: Vec3) -> Result<(), TestCaseError> {
    let mut rd = vec3::default();
    let mut cd = OracleVec3::default();
    let rv = r3(v);
    let cv = o3(v);
    // SAFETY: the pointers refer to live, aligned locals.
    unsafe {
        rs::polar_to_cart(&mut rd, &rv);
        c::oracle_polar_to_cart(&mut cd, &cv);
    }
    prop_assert!(xyz(rd, cd), "polar_to_cart shim {rd:?} != C xyz {cd:?}");
    prop_assert_eq!(rd.w, 0.0);
    let core: vec3 = math_extra::polar_to_cart(v).into();
    prop_assert!(xyz(core, cd), "polar_to_cart core {core:?} != C xyz {cd:?}");

    unsafe {
        rs::cart_to_polar(&mut rd, &rv);
        c::oracle_cart_to_polar(&mut cd, &cv);
    }
    prop_assert!(xyz(rd, cd), "cart_to_polar shim {rd:?} != C xyz {cd:?}");
    prop_assert_eq!(rd.w, 0.0);
    let core: vec3 = math_extra::cart_to_polar(v).into();
    prop_assert!(xyz(core, cd), "cart_to_polar core {core:?} != C xyz {cd:?}");
    Ok(())
}

fn check_norm(norm: Vec3, polar: Vec2) -> Result<(), TestCaseError> {
    let mut rd2 = vec2::default();
    let mut cd2 = OracleVec2::default();
    // SAFETY: the pointers refer to live, aligned locals.
    unsafe {
        rs::norm_to_polar(&mut rd2, &r3(norm));
        c::oracle_norm_to_polar(&mut cd2, &o3(norm));
    }
    prop_assert!(
        same(rd2.x, cd2.x) && same(rd2.y, cd2.y),
        "norm_to_polar shim {rd2:?} != C {cd2:?}"
    );
    let core = math_extra::norm_to_polar(norm);
    prop_assert!(
        same(core.x, cd2.x) && same(core.y, cd2.y),
        "norm_to_polar core {core:?} != C {cd2:?}"
    );

    let mut rd = vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 7.0,
    };
    let mut cd = OracleVec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 7.0,
    };
    let rp = vec2 {
        x: polar.x,
        y: polar.y,
    };
    let cp = OracleVec2 {
        x: polar.x,
        y: polar.y,
    };
    unsafe {
        rs::polar_to_norm(&mut rd, &rp);
        c::oracle_polar_to_norm(&mut cd, &cp);
    }
    prop_assert!(xyz(rd, cd), "polar_to_norm shim {rd:?} != C xyz {cd:?}");
    prop_assert!(
        same(rd.w, 7.0) && same(cd.w, 7.0),
        "w overwritten: {rd:?} {cd:?}"
    );
    let core: vec3 = math_extra::polar_to_norm(polar).into();
    prop_assert!(xyz(core, cd), "polar_to_norm core {core:?} != C xyz {cd:?}");
    Ok(())
}

fn check_torque(
    v1: Vec3,
    v2: Vec3,
    torque: f32,
    min_adjust: f32,
    t: f32,
) -> Result<(), TestCaseError> {
    let mut rd = vec3::default();
    let mut cd = OracleVec3::default();
    let (a, b) = (r3(v1), r3(v2));
    let (ca, cb) = (o3(v1), o3(v2));
    // SAFETY: the pointers refer to live, aligned locals. dst is distinct.
    unsafe {
        rs::calc_torque(&mut rd, &a, &b, torque, min_adjust, t);
        c::oracle_calc_torque(&mut cd, &ca, &cb, torque, min_adjust, t);
    }
    prop_assert!(same3(rd, cd), "calc_torque shim {rd:?} != C {cd:?}");
    let core: vec3 = calc_torque(v1, v2, torque, min_adjust, t).into();
    prop_assert!(same3(core, cd), "calc_torque core {core:?} != C {cd:?}");

    let rs_f = rs::calc_torquef(v1.x, v2.x, torque, min_adjust, t);
    let c_f = unsafe { c::oracle_calc_torquef(v1.x, v2.x, torque, min_adjust, t) };
    prop_assert!(
        same(rs_f, c_f),
        "calc_torquef {rs_f:?} != C {c_f:?} for {} {}",
        v1.x,
        v2.x
    );
    Ok(())
}

fn any_bits() -> impl Strategy<Value = f32> + Clone {
    any::<u32>().prop_map(f32::from_bits)
}

fn finite() -> impl Strategy<Value = f32> + Clone {
    any::<f32>().prop_filter("finite", |v: &f32| v.is_finite())
}

fn arb_vec3(elem: impl Strategy<Value = f32> + Clone) -> impl Strategy<Value = Vec3> {
    (elem.clone(), elem.clone(), elem.clone(), elem)
        .prop_map(|(x, y, z, w)| Vec3::with_w(x, y, z, w))
}

/// `test_calc_torque_dst_aliases_v1` from the cmocka file. The aliased call
/// matches the separate-storage result.
#[test]
fn calc_torque_dst_aliases_v1_matches_c() {
    let v1 = Vec3::with_w(10.0, 0.0, 0.0, 9.0);
    let v2 = Vec3::with_w(0.0, 0.0, 0.0, 4.0);
    let mut rd = r3(v1);
    let mut cd = o3(v1);
    let b = r3(v2);
    let cb = o3(v2);
    // SAFETY: dst and v1 are the same local. v2 is a distinct local.
    unsafe {
        rs::calc_torque(&mut rd, &rd, &b, 0.5, 0.1, 1.0);
        c::oracle_calc_torque(&mut cd, &cd, &cb, 0.5, 0.1, 1.0);
    }
    assert!(same3(rd, cd), "aliased calc_torque {rd:?} != C {cd:?}");
    assert_eq!((rd.x, rd.y, rd.z, rd.w), (5.0, 0.0, 0.0, 0.0));
}

/// `rand_float` draws the C library `rand`. Seed the same sequence for each
/// side. One test, so no other thread in this process draws in between.
#[test]
fn rand_float_matches_c_oracle() {
    for seed in [0u32, 1, 42, 0xdead_beef] {
        for flag in [0, 1, 2, -1] {
            // SAFETY: srand and rand only touch the C library's state.
            let (got, expect) = unsafe {
                libc::srand(seed);
                let got = rs::rand_float(flag);
                libc::srand(seed);
                let expect = c::oracle_rand_float(flag);
                (got, expect)
            };
            assert!(
                same(got, expect),
                "seed {seed} flag {flag}: {got:?} != C {expect:?}"
            );
            if flag != 0 {
                assert!((0.0..=1.0).contains(&got), "{got}");
            } else {
                assert!((-1.0..=1.0).contains(&got), "{got}");
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(1024)))]

    #[test]
    fn polar_matches_c_oracle_any_bits(v in arb_vec3(any_bits())) {
        check_polar(v)?;
    }

    #[test]
    fn polar_matches_c_oracle_finite(v in arb_vec3(finite())) {
        check_polar(v)?;
    }

    #[test]
    fn norm_matches_c_oracle_any_bits(
        norm in arb_vec3(any_bits()),
        polar in (any_bits(), any_bits()),
    ) {
        check_norm(norm, Vec2::new(polar.0, polar.1))?;
    }

    #[test]
    fn norm_matches_c_oracle_finite(
        norm in arb_vec3(finite()),
        polar in (finite(), finite()),
    ) {
        check_norm(norm, Vec2::new(polar.0, polar.1))?;
    }

    #[test]
    fn torque_matches_c_oracle_finite(
        v1 in arb_vec3(finite()),
        v2 in arb_vec3(finite()),
        torque in finite(),
        min_adjust in finite(),
        t in finite(),
    ) {
        check_torque(v1, v2, torque, min_adjust, t)?;
    }

    #[test]
    fn torque_matches_c_oracle_any_bits(
        v1 in arb_vec3(any_bits()),
        v2 in arb_vec3(any_bits()),
        torque in any_bits(),
        min_adjust in any_bits(),
        t in any_bits(),
    ) {
        check_torque(v1, v2, torque, min_adjust, t)?;
    }
}

/// Fixed inputs captured from macOS CI, including large argument reduction.
#[test]
fn paired_trig_regressions_match_c() {
    check_polar(Vec3::new(0.0, -5886.8154, 1.2832579e19)).unwrap();
    check_polar(Vec3::new(0.0, 1.1273432e30, 3.4347034e-22)).unwrap();
    check_norm(Vec3::default(), Vec2::new(4.5483803e35, 0.0)).unwrap();
}
