//! Tier 3: the Rust C ABI shim and safe core behave like the original C.
//!
//! Intentional differences, excluded from the oracle comparison:
//! - `bounds_under_plane` and `bounds_plane_test` set the corner `w` to 0.
//!   C leaves it uninitialized and `vec3_dot` reads it. The proptest checks
//!   the shim against the safe core only. `plane_test_abi_byte` checks that
//!   the oracle stores 1: C converts the 1/2/3 codes to `bool`.
//! - `bounds_intersects_obb` uses a zero matrix when `|det| < 0.0005`. C
//!   leaves that inverse uninitialized. Those matrices are skipped.
//!
//! No ULP slack. `vec3_min` / `vec3_max` follow SSE (second operand unless
//! the comparison holds). A ray or line miss does not write `*t`.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use obs_c_oracle::graphics_math::{
    self as c, OracleBounds, OracleMatrix3, OracleMatrix4, OraclePlane, OracleVec3,
};
use obs_graphics::bounds::Bounds;
use obs_graphics::ffi::bounds::{self as rs, bounds};
use obs_graphics::ffi::matrix3::matrix3;
use obs_graphics::ffi::matrix4::matrix4;
use obs_graphics::ffi::plane::plane;
use obs_graphics::ffi::vec3::vec3;
use obs_graphics::matrix3::Matrix3;
use obs_graphics::matrix4::Matrix4;
use obs_graphics::plane::Plane;
use obs_graphics::vec3::Vec3;
use obs_graphics::vec4::Vec4;
use proptest::prelude::*;

/// Bit-exact equality, except that any NaN equals any NaN.
fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn same_vec(r: vec3, c: OracleVec3) -> bool {
    same(r.x, c.x) && same(r.y, c.y) && same(r.z, c.z) && same(r.w, c.w)
}

fn same_bounds(r: bounds, c: OracleBounds) -> bool {
    same_vec(r.min, c.min) && same_vec(r.max, c.max)
}

fn same_vec_rs(r: vec3, c: vec3) -> bool {
    same(r.x, c.x) && same(r.y, c.y) && same(r.z, c.z) && same(r.w, c.w)
}

fn same_vec_cs(c: OracleVec3, d: OracleVec3) -> bool {
    same(c.x, d.x) && same(c.y, d.y) && same(c.z, d.z) && same(c.w, d.w)
}

fn same_bounds_rs(r: bounds, c: bounds) -> bool {
    same_vec_rs(r.min, c.min) && same_vec_rs(r.max, c.max)
}

fn same_bounds_cs(c: OracleBounds, d: OracleBounds) -> bool {
    same_vec_cs(c.min, d.min) && same_vec_cs(c.max, d.max)
}

fn rv(v: [f32; 4]) -> vec3 {
    vec3 {
        x: v[0],
        y: v[1],
        z: v[2],
        w: v[3],
    }
}

fn cv(v: [f32; 4]) -> OracleVec3 {
    OracleVec3 {
        x: v[0],
        y: v[1],
        z: v[2],
        w: v[3],
    }
}

fn rb(min: [f32; 4], max: [f32; 4]) -> bounds {
    bounds {
        min: rv(min),
        max: rv(max),
    }
}

fn cb(min: [f32; 4], max: [f32; 4]) -> OracleBounds {
    OracleBounds {
        min: cv(min),
        max: cv(max),
    }
}

fn core_b(min: [f32; 4], max: [f32; 4]) -> Bounds {
    Bounds::new(Vec3::from(rv(min)), Vec3::from(rv(max)))
}

fn rm4(rows: [[f32; 4]; 4]) -> matrix4 {
    matrix4 {
        x: Vec4::new(rows[0][0], rows[0][1], rows[0][2], rows[0][3]).into(),
        y: Vec4::new(rows[1][0], rows[1][1], rows[1][2], rows[1][3]).into(),
        z: Vec4::new(rows[2][0], rows[2][1], rows[2][2], rows[2][3]).into(),
        t: Vec4::new(rows[3][0], rows[3][1], rows[3][2], rows[3][3]).into(),
    }
}

fn cm4(rows: [[f32; 4]; 4]) -> OracleMatrix4 {
    OracleMatrix4 {
        x: c::OracleVec4 {
            x: rows[0][0],
            y: rows[0][1],
            z: rows[0][2],
            w: rows[0][3],
        },
        y: c::OracleVec4 {
            x: rows[1][0],
            y: rows[1][1],
            z: rows[1][2],
            w: rows[1][3],
        },
        z: c::OracleVec4 {
            x: rows[2][0],
            y: rows[2][1],
            z: rows[2][2],
            w: rows[2][3],
        },
        t: c::OracleVec4 {
            x: rows[3][0],
            y: rows[3][1],
            z: rows[3][2],
            w: rows[3][3],
        },
    }
}

fn rm3(rows: [[f32; 4]; 4]) -> matrix3 {
    matrix3 {
        x: rv(rows[0]),
        y: rv(rows[1]),
        z: rv(rows[2]),
        t: rv(rows[3]),
    }
}

fn cm3(rows: [[f32; 4]; 4]) -> OracleMatrix3 {
    OracleMatrix3 {
        x: cv(rows[0]),
        y: cv(rows[1]),
        z: cv(rows[2]),
        t: cv(rows[3]),
    }
}

fn rplane(dir: [f32; 4], dist: f32) -> plane {
    plane { dir: rv(dir), dist }
}

fn cplane(dir: [f32; 4], dist: f32) -> OraclePlane {
    OraclePlane { dir: cv(dir), dist }
}

struct Case {
    min: [f32; 4],
    max: [f32; 4],
    other_min: [f32; 4],
    other_max: [f32; 4],
    v: [f32; 4],
    p1: [f32; 4],
    p2: [f32; 4],
    dir: [f32; 4],
    dist: f32,
    epsilon: f32,
    sentinel: f32,
    index: u32,
    m4: [[f32; 4]; 4],
    m3: [[f32; 4]; 4],
}

fn assert_box(
    name: &str,
    got: bounds,
    oracle: OracleBounds,
    core: Bounds,
) -> Result<(), TestCaseError> {
    prop_assert!(same_bounds(got, oracle), "{name} shim != C");
    let core_box: bounds = core.into();
    prop_assert!(
        same(got.min.x, core_box.min.x)
            && same(got.min.y, core_box.min.y)
            && same(got.min.z, core_box.min.z)
            && same(got.min.w, core_box.min.w)
            && same(got.max.x, core_box.max.x)
            && same(got.max.y, core_box.max.y)
            && same(got.max.z, core_box.max.z)
            && same(got.max.w, core_box.max.w),
        "{name} shim != core"
    );
    Ok(())
}

fn check_move(case: &Case) -> Result<(), TestCaseError> {
    let mut rs_dst = bounds::default();
    let mut c_dst = OracleBounds::default();
    let src = rb(case.min, case.max);
    let other = cb(case.min, case.max);
    let rv = rv(case.v);
    let cv = cv(case.v);
    unsafe {
        rs::bounds_move(&mut rs_dst, &src, &rv);
        c::oracle_bounds_move(&mut c_dst, &other, &cv);
    }
    assert_box(
        "move",
        rs_dst,
        c_dst,
        core_b(case.min, case.max).move_by(Vec3::from(rv)),
    )?;

    let mut rs_alias = src;
    let mut c_alias = other;
    unsafe {
        rs::bounds_move(&mut rs_alias, &rs_alias, &rv);
        c::oracle_bounds_move(&mut c_alias, &c_alias, &cv);
    }
    assert_box(
        "move alias",
        rs_alias,
        c_alias,
        core_b(case.min, case.max).move_by(Vec3::from(rv)),
    )?;
    Ok(())
}

fn check_scale(case: &Case) -> Result<(), TestCaseError> {
    let mut rs_dst = bounds::default();
    let mut c_dst = OracleBounds::default();
    let src = rb(case.min, case.max);
    let other = cb(case.min, case.max);
    unsafe {
        rs::bounds_scale(&mut rs_dst, &src, &rv(case.v));
        c::oracle_bounds_scale(&mut c_dst, &other, &cv(case.v));
    }
    assert_box(
        "scale",
        rs_dst,
        c_dst,
        core_b(case.min, case.max).scale(Vec3::from(rv(case.v))),
    )
}

fn check_merge(case: &Case) -> Result<(), TestCaseError> {
    let mut rs_dst = bounds::default();
    let mut c_dst = OracleBounds::default();
    let a = rb(case.min, case.max);
    let b = rb(case.other_min, case.other_max);
    let ca = cb(case.min, case.max);
    let c_b = cb(case.other_min, case.other_max);
    unsafe {
        rs::bounds_merge(&mut rs_dst, &a, &b);
        c::oracle_bounds_merge(&mut c_dst, &ca, &c_b);
    }
    assert_box(
        "merge",
        rs_dst,
        c_dst,
        core_b(case.min, case.max).merge(core_b(case.other_min, case.other_max)),
    )?;

    let mut rs_alias = a;
    let mut c_alias = ca;
    unsafe {
        rs::bounds_merge(&mut rs_alias, &rs_alias, &b);
        c::oracle_bounds_merge(&mut c_alias, &c_alias, &c_b);
    }
    assert_box(
        "merge alias",
        rs_alias,
        c_alias,
        core_b(case.min, case.max).merge(core_b(case.other_min, case.other_max)),
    )?;
    Ok(())
}

fn check_merge_point(case: &Case) -> Result<(), TestCaseError> {
    let mut rs_dst = bounds::default();
    let mut c_dst = OracleBounds::default();
    unsafe {
        rs::bounds_merge_point(&mut rs_dst, &rb(case.min, case.max), &rv(case.v));
        c::oracle_bounds_merge_point(&mut c_dst, &cb(case.min, case.max), &cv(case.v));
    }
    assert_box(
        "merge_point",
        rs_dst,
        c_dst,
        core_b(case.min, case.max).merge_point(Vec3::from(rv(case.v))),
    )
}

fn check_point(case: &Case, index: u32) -> Result<(), TestCaseError> {
    let sent = case.sentinel;
    let mut rs_dst = vec3 {
        x: sent,
        y: sent,
        z: sent,
        w: sent,
    };
    let mut c_dst = OracleVec3 {
        x: sent,
        y: sent,
        z: sent,
        w: sent,
    };
    unsafe {
        rs::bounds_get_point(&mut rs_dst, &rb(case.min, case.max), index);
        c::oracle_bounds_get_point(&mut c_dst, &cb(case.min, case.max), index);
    }
    prop_assert!(same_vec(rs_dst, c_dst), "get_point {index}");
    let core = core_b(case.min, case.max).point(index);
    match core {
        None => {
            prop_assert!(
                same(rs_dst.x, sent) && same(rs_dst.w, sent),
                "get_point untouched"
            );
        }
        Some(p) => {
            prop_assert!(same(rs_dst.x, p.x) && same(rs_dst.y, p.y) && same(rs_dst.z, p.z));
            prop_assert!(same(rs_dst.w, sent), "get_point leaves w");
        }
    }
    Ok(())
}

fn check_center(case: &Case) -> Result<(), TestCaseError> {
    let mut rs_dst = vec3::default();
    let mut c_dst = OracleVec3::default();
    unsafe {
        rs::bounds_get_center(&mut rs_dst, &rb(case.min, case.max));
        c::oracle_bounds_get_center(&mut c_dst, &cb(case.min, case.max));
    }
    prop_assert!(same_vec(rs_dst, c_dst), "center");
    let core = core_b(case.min, case.max).center();
    prop_assert!(
        same(rs_dst.x, core.x)
            && same(rs_dst.y, core.y)
            && same(rs_dst.z, core.z)
            && same(rs_dst.w, core.w)
    );
    Ok(())
}

fn check_transform(case: &Case) -> Result<(), TestCaseError> {
    let mut rs_dst = bounds::default();
    let mut c_dst = OracleBounds::default();
    let mat = rm4(case.m4);
    let cmat = cm4(case.m4);
    unsafe {
        rs::bounds_transform(&mut rs_dst, &rb(case.min, case.max), &mat);
        c::oracle_bounds_transform(&mut c_dst, &cb(case.min, case.max), &cmat);
    }
    assert_box(
        "transform",
        rs_dst,
        c_dst,
        core_b(case.min, case.max).transform(&Matrix4::from(mat)),
    )
}

fn check_transform3x4(case: &Case) -> Result<(), TestCaseError> {
    let mut rs_dst = bounds::default();
    let mut c_dst = OracleBounds::default();
    let mat = rm3(case.m3);
    unsafe {
        rs::bounds_transform3x4(&mut rs_dst, &rb(case.min, case.max), &mat);
        c::oracle_bounds_transform3x4(&mut c_dst, &cb(case.min, case.max), &cm3(case.m3));
    }
    assert_box(
        "transform3x4",
        rs_dst,
        c_dst,
        core_b(case.min, case.max).transform3x4(&Matrix3::from(mat)),
    )
}

fn check_ray(case: &Case) -> Result<(), TestCaseError> {
    let mut rs_t = case.sentinel;
    let mut c_t = case.sentinel;
    let rs_hit = unsafe {
        rs::bounds_intersection_ray(
            &rb(case.min, case.max),
            &rv(case.p1),
            &rv(case.v),
            &mut rs_t,
        )
    };
    let c_hit = unsafe {
        c::oracle_bounds_intersection_ray(
            &cb(case.min, case.max),
            &cv(case.p1),
            &cv(case.v),
            &mut c_t,
        )
    };
    prop_assert_eq!(rs_hit, c_hit, "ray bool");
    prop_assert!(same(rs_t, c_t), "ray t");
    let core = core_b(case.min, case.max)
        .intersection_ray(Vec3::from(rv(case.p1)), Vec3::from(rv(case.v)));
    prop_assert_eq!(rs_hit, core.is_some());
    if let Some(t) = core {
        prop_assert!(same(rs_t, t));
    } else {
        prop_assert!(same(rs_t, case.sentinel));
    }
    Ok(())
}

fn check_line(case: &Case) -> Result<(), TestCaseError> {
    let mut rs_t = case.sentinel;
    let mut c_t = case.sentinel;
    let rs_hit = unsafe {
        rs::bounds_intersection_line(
            &rb(case.min, case.max),
            &rv(case.p1),
            &rv(case.p2),
            &mut rs_t,
        )
    };
    let c_hit = unsafe {
        c::oracle_bounds_intersection_line(
            &cb(case.min, case.max),
            &cv(case.p1),
            &cv(case.p2),
            &mut c_t,
        )
    };
    prop_assert_eq!(rs_hit, c_hit, "line bool");
    prop_assert!(same(rs_t, c_t), "line t");
    Ok(())
}

fn check_intersects(case: &Case) -> Result<(), TestCaseError> {
    let a = rb(case.min, case.max);
    let b = rb(case.other_min, case.other_max);
    let rs_hit = unsafe { rs::bounds_intersects(&a, &b, case.epsilon) };
    let c_hit = unsafe {
        c::oracle_bounds_intersects(
            &cb(case.min, case.max),
            &cb(case.other_min, case.other_max),
            case.epsilon,
        )
    };
    prop_assert_eq!(rs_hit, c_hit, "intersects");
    prop_assert_eq!(
        rs_hit,
        core_b(case.min, case.max)
            .intersects(&core_b(case.other_min, case.other_max), case.epsilon)
    );
    Ok(())
}

fn check_obb(case: &Case) -> Result<(), TestCaseError> {
    let mat = Matrix4::from(rm4(case.m4));
    let core = core_b(case.min, case.max);
    let other = core_b(case.other_min, case.other_max);
    let rs_hit = unsafe {
        rs::bounds_intersects_obb(
            &rb(case.min, case.max),
            &rb(case.other_min, case.other_max),
            &rm4(case.m4),
            case.epsilon,
        )
    };
    prop_assert_eq!(rs_hit, core.intersects_obb(&other, &mat, case.epsilon));
    if mat.try_inv().is_some() {
        let c_hit = unsafe {
            c::oracle_bounds_intersects_obb(
                &cb(case.min, case.max),
                &cb(case.other_min, case.other_max),
                &cm4(case.m4),
                case.epsilon,
            )
        };
        prop_assert_eq!(rs_hit, c_hit, "obb");
    }
    Ok(())
}

fn check_obb3x4(case: &Case) -> Result<(), TestCaseError> {
    let rs_hit = unsafe {
        rs::bounds_intersects_obb3x4(
            &rb(case.min, case.max),
            &rb(case.other_min, case.other_max),
            &rm3(case.m3),
            case.epsilon,
        )
    };
    let c_hit = unsafe {
        c::oracle_bounds_intersects_obb3x4(
            &cb(case.min, case.max),
            &cb(case.other_min, case.other_max),
            &cm3(case.m3),
            case.epsilon,
        )
    };
    prop_assert_eq!(rs_hit, c_hit, "obb3x4");
    Ok(())
}

fn check_min_dist(case: &Case) -> Result<(), TestCaseError> {
    let rs_d =
        unsafe { rs::bounds_min_dist(&rb(case.min, case.max), &rplane(case.dir, case.dist)) };
    let c_d =
        unsafe { c::oracle_bounds_min_dist(&cb(case.min, case.max), &cplane(case.dir, case.dist)) };
    prop_assert!(same(rs_d, c_d), "min_dist");
    Ok(())
}

fn check_defined_only(case: &Case) -> Result<(), TestCaseError> {
    let core = core_b(case.min, case.max);
    let pl = Plane::new(Vec3::from(rv(case.dir)), case.dist);
    let rs_under =
        unsafe { rs::bounds_under_plane(&rb(case.min, case.max), &rplane(case.dir, case.dist)) };
    prop_assert_eq!(rs_under, core.under_plane(&pl), "under_plane shim");
    let rs_abi =
        unsafe { rs::bounds_plane_test(&rb(case.min, case.max), &rplane(case.dir, case.dist)) };
    prop_assert!(rs_abi, "plane_test ABI is 1 for every code");
    Ok(())
}

fn check(case: &Case) -> Result<(), TestCaseError> {
    check_move(case)?;
    check_scale(case)?;
    check_merge(case)?;
    check_merge_point(case)?;
    check_point(case, case.index)?;
    check_point(case, case.index % 9)?;
    check_center(case)?;
    check_transform(case)?;
    check_transform3x4(case)?;
    check_ray(case)?;
    check_line(case)?;
    check_intersects(case)?;
    check_obb(case)?;
    check_obb3x4(case)?;
    check_min_dist(case)?;
    check_defined_only(case)?;
    Ok(())
}

fn any_bits() -> impl Strategy<Value = f32> {
    any::<u32>().prop_map(f32::from_bits)
}

fn row() -> impl Strategy<Value = [f32; 4]> {
    [any_bits(), any_bits(), any_bits(), any_bits()]
}

fn mat() -> impl Strategy<Value = [[f32; 4]; 4]> {
    [row(), row(), row(), row()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn bounds_matches_c_oracle(
        min in row(),
        max in row(),
        other_min in row(),
        other_max in row(),
        v in row(),
        p1 in row(),
        p2 in row(),
        dir in row(),
        dist in any_bits(),
        epsilon in any_bits(),
        sentinel in any_bits(),
        index in any::<u32>(),
        m4 in mat(),
        m3 in mat(),
    ) {
        check(&Case {
            min,
            max,
            other_min,
            other_max,
            v,
            p1,
            p2,
            dir,
            dist,
            epsilon,
            sentinel,
            index,
            m4,
            m3,
        })?;
    }
}

/// Three finite classifications. The oracle reads an uninitialized `w`; with
/// `dir.w == 0` a finite garbage lane contributes 0. The byte is whatever
/// this compiler stored for the `bool` return.
#[test]
fn plane_test_abi_byte() {
    let src = core_b([0.0, 0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 0.0]);
    let box_rs = rb([0.0, 0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 0.0]);
    let box_c = cb([0.0, 0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 0.0]);
    let planes = [
        Plane::new(Vec3::new(0.0, 0.0, 1.0), 2.0),
        Plane::new(Vec3::new(0.0, 0.0, 1.0), -1.0),
        Plane::new(Vec3::new(0.0, 0.0, 1.0), 0.5),
    ];
    for pl in planes {
        let code = src.plane_test(&pl);
        let dir = [pl.dir.x, pl.dir.y, pl.dir.z, pl.dir.w];
        let rs_byte = unsafe { rs::bounds_plane_test(&box_rs, &rplane(dir, pl.dist)) };
        let c_byte = unsafe { c::oracle_bounds_plane_test(&box_c, &cplane(dir, pl.dist)) };
        assert!(rs_byte, "safe code {code}");
        assert_eq!(c_byte, 1, "safe code {code}");
    }
}

/// The vector operand overlaps `dst` (it is `dst.min`), the arrangement the
/// alias audit measured as broken for all four helpers. `b` stays separate
/// here; the C regression in `test_graphics_alias_ordinary.c` also covers
/// `dst == b` with the operand in `dst.min`. Both implementations read their
/// inputs in full before writing, so the aliased call equals a separate one.
#[test]
fn move_vec_aliases_dst_min_matches_c() {
    let rb_src = rb([1.0, 2.0, 3.0, 0.0], [10.0, 20.0, 30.0, 0.0]);
    let cb_src = cb([1.0, 2.0, 3.0, 0.0], [10.0, 20.0, 30.0, 0.0]);
    let v = [10.0, -1.0, 0.5, 9.0];
    let mut rd = bounds::default();
    let mut cd = OracleBounds::default();
    // SAFETY: all pointers refer to live locals.
    unsafe {
        rs::bounds_move(&mut rd, &rb_src, &rv(v));
        c::oracle_bounds_move(&mut cd, &cb_src, &cv(v));
    }

    let mut r_alias = rb_src;
    let mut c_alias = cb_src;
    r_alias.min = rv(v);
    c_alias.min = cv(v);
    // SAFETY: the operand points into `dst`, which both implementations allow.
    unsafe {
        rs::bounds_move(&mut r_alias, &rb_src, &r_alias.min);
        c::oracle_bounds_move(&mut c_alias, &cb_src, &c_alias.min);
    }
    assert!(same_bounds(r_alias, c_alias), "alias shim != C");
    assert!(
        same_bounds_rs(r_alias, rd),
        "alias shim {r_alias:?} != non-aliased {rd:?}"
    );
    assert!(
        same_bounds_cs(c_alias, cd),
        "C alias {c_alias:?} != non-aliased {cd:?}"
    );
}

/// `bounds_scale` with the vector operand inside the output.
#[test]
fn scale_vec_aliases_dst_min_matches_c() {
    let rb_src = rb([2.0, 3.0, 4.0, 0.0], [10.0, 20.0, 30.0, 0.0]);
    let cb_src = cb([2.0, 3.0, 4.0, 0.0], [10.0, 20.0, 30.0, 0.0]);
    let v = [2.0, -1.0, 0.5, 9.0];
    let mut rd = bounds::default();
    let mut cd = OracleBounds::default();
    // SAFETY: all pointers refer to live locals.
    unsafe {
        rs::bounds_scale(&mut rd, &rb_src, &rv(v));
        c::oracle_bounds_scale(&mut cd, &cb_src, &cv(v));
    }

    let mut r_alias = rb_src;
    let mut c_alias = cb_src;
    r_alias.min = rv(v);
    c_alias.min = cv(v);
    // SAFETY: the operand points into `dst`, which both implementations allow.
    unsafe {
        rs::bounds_scale(&mut r_alias, &rb_src, &r_alias.min);
        c::oracle_bounds_scale(&mut c_alias, &cb_src, &c_alias.min);
    }
    assert!(same_bounds(r_alias, c_alias), "scale alias shim != C");
    assert!(
        same_bounds_rs(r_alias, rd),
        "scale alias shim {r_alias:?} != non-aliased {rd:?}"
    );
    assert!(
        same_bounds_cs(c_alias, cd),
        "scale C alias {c_alias:?} != non-aliased {cd:?}"
    );
}

/// `bounds_merge_point` with the point inside the output.
#[test]
fn merge_point_vec_aliases_dst_min_matches_c() {
    let rb_src = rb([5.0, 5.0, 5.0, 0.0], [10.0, 10.0, 10.0, 0.0]);
    let cb_src = cb([5.0, 5.0, 5.0, 0.0], [10.0, 10.0, 10.0, 0.0]);
    let v = [20.0, 20.0, 20.0, 7.0];
    let mut rd = rb([20.0, 20.0, 20.0, 0.0], [30.0, 30.0, 30.0, 0.0]);
    let mut cd = cb([20.0, 20.0, 20.0, 0.0], [30.0, 30.0, 30.0, 0.0]);
    // SAFETY: all pointers refer to live locals.
    unsafe {
        rs::bounds_merge_point(&mut rd, &rb_src, &rv(v));
        c::oracle_bounds_merge_point(&mut cd, &cb_src, &cv(v));
    }

    let mut r_alias = rb([20.0, 20.0, 20.0, 0.0], [30.0, 30.0, 30.0, 0.0]);
    let mut c_alias = cb([20.0, 20.0, 20.0, 0.0], [30.0, 30.0, 30.0, 0.0]);
    r_alias.min = rv(v);
    c_alias.min = cv(v);
    // SAFETY: the point points into `dst`, which both implementations allow.
    unsafe {
        rs::bounds_merge_point(&mut r_alias, &rb_src, &r_alias.min);
        c::oracle_bounds_merge_point(&mut c_alias, &cb_src, &c_alias.min);
    }
    assert!(same_bounds(r_alias, c_alias), "merge_point alias shim != C");
    assert!(
        same_bounds_rs(r_alias, rd),
        "merge_point alias shim {r_alias:?} != non-aliased {rd:?}"
    );
    assert!(
        same_bounds_cs(c_alias, cd),
        "merge_point C alias {c_alias:?} != non-aliased {cd:?}"
    );
}

/// `bounds_get_center` with `dst` sharing storage with `b.min`.
#[test]
fn get_center_dst_aliases_min_matches_c() {
    let rb_src = rb([0.0, 0.0, 0.0, 0.0], [10.0, 20.0, 30.0, 0.0]);
    let cb_src = cb([0.0, 0.0, 0.0, 0.0], [10.0, 20.0, 30.0, 0.0]);
    let mut rd = vec3::default();
    let mut cd = OracleVec3::default();
    // SAFETY: all pointers refer to live locals.
    unsafe {
        rs::bounds_get_center(&mut rd, &rb_src);
        c::oracle_bounds_get_center(&mut cd, &cb_src);
    }

    let mut r_alias = rb_src;
    let mut c_alias = cb_src;
    // SAFETY: `dst` is `b.min`, which the C audit found broken.
    unsafe {
        rs::bounds_get_center(&mut r_alias.min, &r_alias);
        c::oracle_bounds_get_center(&mut c_alias.min, &c_alias);
    }
    assert!(same_vec(r_alias.min, c_alias.min), "center alias shim != C");
    assert!(
        same_vec_rs(r_alias.min, rd),
        "center alias shim != non-aliased {rd:?}"
    );
    assert!(
        same_vec_cs(c_alias.min, cd),
        "center C alias != non-aliased {cd:?}"
    );
}
