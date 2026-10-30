//! Tier 3: the Rust C ABI shim and safe core behave exactly like the
//! original C, compiled as an oracle.
//!
//! No intentional differences from C. `matrix3_inv` follows `matrix4_inv`,
//! including leaving `dst` unchanged except `t.w = 0` when `|det| < 0.0005`.
//! `matrix3_transpose` keeps the SSE shuffle's `w` lanes (`z.w` on every axis).
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use obs_c_oracle::graphics_math::{
    self as c, OracleAxisAng, OracleMatrix3, OracleMatrix4, OraclePlane, OracleQuat, OracleVec3,
    OracleVec4,
};
use obs_graphics::axisang::AxisAng;
use obs_graphics::ffi::axisang::axisang;
use obs_graphics::ffi::matrix3::{self as rs, matrix3};
use obs_graphics::ffi::matrix4::matrix4;
use obs_graphics::ffi::plane::plane;
use obs_graphics::ffi::quat::quat;
use obs_graphics::ffi::vec3::vec3;
use obs_graphics::ffi::vec4::vec4;
use obs_graphics::matrix3::Matrix3;
use obs_graphics::matrix4::Matrix4;
use obs_graphics::plane::Plane;
use obs_graphics::quat::Quat;
use obs_graphics::vec3::Vec3;
use obs_graphics::vec4::Vec4;
use proptest::prelude::*;

/// Bit-exact equality, except that any NaN equals any NaN.
fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn same_row(r: vec3, c: OracleVec3) -> bool {
    same(r.x, c.x) && same(r.y, c.y) && same(r.z, c.z) && same(r.w, c.w)
}

fn same_m(r: matrix3, c: OracleMatrix3) -> bool {
    same_row(r.x, c.x) && same_row(r.y, c.y) && same_row(r.z, c.z) && same_row(r.t, c.t)
}

fn rs_row(v: [f32; 4]) -> vec3 {
    vec3 {
        x: v[0],
        y: v[1],
        z: v[2],
        w: v[3],
    }
}

fn c_row(v: [f32; 4]) -> OracleVec3 {
    OracleVec3 {
        x: v[0],
        y: v[1],
        z: v[2],
        w: v[3],
    }
}

fn rs_row4(v: [f32; 4]) -> vec4 {
    vec4 {
        x: v[0],
        y: v[1],
        z: v[2],
        w: v[3],
    }
}

fn c_row4(v: [f32; 4]) -> OracleVec4 {
    OracleVec4 {
        x: v[0],
        y: v[1],
        z: v[2],
        w: v[3],
    }
}

fn rs_m(rows: [[f32; 4]; 4]) -> matrix3 {
    matrix3 {
        x: rs_row(rows[0]),
        y: rs_row(rows[1]),
        z: rs_row(rows[2]),
        t: rs_row(rows[3]),
    }
}

fn c_m(rows: [[f32; 4]; 4]) -> OracleMatrix3 {
    OracleMatrix3 {
        x: c_row(rows[0]),
        y: c_row(rows[1]),
        z: c_row(rows[2]),
        t: c_row(rows[3]),
    }
}

fn core_m(rows: [[f32; 4]; 4]) -> Matrix3 {
    let row = |r: [f32; 4]| Vec3::with_w(r[0], r[1], r[2], r[3]);
    Matrix3::new(row(rows[0]), row(rows[1]), row(rows[2]), row(rows[3]))
}

fn rs_q(q: [f32; 4]) -> quat {
    quat {
        x: q[0],
        y: q[1],
        z: q[2],
        w: q[3],
    }
}

fn c_q(q: [f32; 4]) -> OracleQuat {
    OracleQuat {
        x: q[0],
        y: q[1],
        z: q[2],
        w: q[3],
    }
}

fn rs_aa(a: [f32; 4]) -> axisang {
    axisang {
        x: a[0],
        y: a[1],
        z: a[2],
        w: a[3],
    }
}

fn c_aa(a: [f32; 4]) -> OracleAxisAng {
    OracleAxisAng {
        x: a[0],
        y: a[1],
        z: a[2],
        w: a[3],
    }
}

fn assert_both(
    label: &str,
    shim: matrix3,
    oracle: OracleMatrix3,
    core: matrix3,
) -> Result<(), TestCaseError> {
    prop_assert!(
        same_m(shim, oracle),
        "{label} shim {shim:?} != C {oracle:?}"
    );
    prop_assert!(
        same_m(core, oracle),
        "{label} core {core:?} != C {oracle:?}"
    );
    Ok(())
}

#[expect(
    clippy::too_many_lines,
    reason = "parity table: one block per ported C case"
)]
fn check(
    rows: [[f32; 4]; 4],
    other: [[f32; 4]; 4],
    q: [f32; 4],
    dist: f32,
) -> Result<(), TestCaseError> {
    let rm = rs_m(rows);
    let cm = c_m(rows);
    let ro = rs_m(other);
    let co = c_m(other);
    let rq = rs_q(q);
    let cq = c_q(q);
    let ra = rs_aa(q);
    let ca = c_aa(q);
    let rv = rs_row(q);
    let cv = c_row(q);
    let rp = plane {
        dir: rs_row([q[0], q[1], q[2], q[3]]),
        dist,
    };
    let cp = OraclePlane {
        dir: c_row([q[0], q[1], q[2], q[3]]),
        dist,
    };
    let core = core_m(rows);
    let core_o = core_m(other);

    let mut rs_dst = matrix3::default();
    let mut c_dst = OracleMatrix3::default();
    // SAFETY: every pointer refers to a live local.
    unsafe {
        rs::matrix3_from_quat(&mut rs_dst, &rq);
        c::oracle_matrix3_from_quat(&mut c_dst, &cq);
    }
    assert_both(
        "from_quat",
        rs_dst,
        c_dst,
        Matrix3::from_quat(Quat::new(q[0], q[1], q[2], q[3])).into(),
    )?;

    unsafe {
        rs::matrix3_from_axisang(&mut rs_dst, &ra);
        c::oracle_matrix3_from_axisang(&mut c_dst, &ca);
    }
    assert_both(
        "from_axisang",
        rs_dst,
        c_dst,
        Matrix3::from_axisang(AxisAng::new(q[0], q[1], q[2], q[3])).into(),
    )?;

    let r4 = matrix4 {
        x: rs_row4(rows[0]),
        y: rs_row4(rows[1]),
        z: rs_row4(rows[2]),
        t: rs_row4(rows[3]),
    };
    let c4 = OracleMatrix4 {
        x: c_row4(rows[0]),
        y: c_row4(rows[1]),
        z: c_row4(rows[2]),
        t: c_row4(rows[3]),
    };
    unsafe {
        rs::matrix3_from_matrix4(&mut rs_dst, &r4);
        c::oracle_matrix3_from_matrix4(&mut c_dst, &c4);
    }
    assert_both(
        "from_matrix4",
        rs_dst,
        c_dst,
        Matrix3::from_matrix4(Matrix4::new(
            Vec4::new(rows[0][0], rows[0][1], rows[0][2], rows[0][3]),
            Vec4::new(rows[1][0], rows[1][1], rows[1][2], rows[1][3]),
            Vec4::new(rows[2][0], rows[2][1], rows[2][2], rows[2][3]),
            Vec4::new(rows[3][0], rows[3][1], rows[3][2], rows[3][3]),
        ))
        .into(),
    )?;

    unsafe {
        rs::matrix3_mul(&mut rs_dst, &rm, &ro);
        c::oracle_matrix3_mul(&mut c_dst, &cm, &co);
    }
    assert_both("mul", rs_dst, c_dst, (core * &core_o).into())?;

    unsafe {
        rs::matrix3_rotate(&mut rs_dst, &rm, &rq);
        c::oracle_matrix3_rotate(&mut c_dst, &cm, &cq);
    }
    assert_both(
        "rotate",
        rs_dst,
        c_dst,
        core.rotate(Quat::new(q[0], q[1], q[2], q[3])).into(),
    )?;

    unsafe {
        rs::matrix3_rotate_aa(&mut rs_dst, &rm, &ra);
        c::oracle_matrix3_rotate_aa(&mut c_dst, &cm, &ca);
    }
    assert_both(
        "rotate_aa",
        rs_dst,
        c_dst,
        core.rotate_aa(AxisAng::new(q[0], q[1], q[2], q[3])).into(),
    )?;

    unsafe {
        rs::matrix3_scale(&mut rs_dst, &rm, &rv);
        c::oracle_matrix3_scale(&mut c_dst, &cm, &cv);
    }
    assert_both(
        "scale",
        rs_dst,
        c_dst,
        core.scale(Vec3::with_w(q[0], q[1], q[2], q[3])).into(),
    )?;

    unsafe {
        rs::matrix3_transpose(&mut rs_dst, &rm);
        c::oracle_matrix3_transpose(&mut c_dst, &cm);
    }
    assert_both("transpose", rs_dst, c_dst, core.transpose().into())?;

    rs_dst = ro;
    c_dst = co;
    unsafe {
        rs::matrix3_inv(&mut rs_dst, &rm);
        c::oracle_matrix3_inv(&mut c_dst, &cm);
    }
    assert_both("inv", rs_dst, c_dst, core_o.inv(core).into())?;

    unsafe {
        rs::matrix3_mirror(&mut rs_dst, &rm, &rp);
        c::oracle_matrix3_mirror(&mut c_dst, &cm, &cp);
    }
    assert_both(
        "mirror",
        rs_dst,
        c_dst,
        core.mirror(&Plane::new(Vec3::with_w(q[0], q[1], q[2], q[3]), dist))
            .into(),
    )?;

    unsafe {
        rs::matrix3_mirrorv(&mut rs_dst, &rm, &rv);
        c::oracle_matrix3_mirrorv(&mut c_dst, &cm, &cv);
    }
    assert_both(
        "mirrorv",
        rs_dst,
        c_dst,
        core.mirrorv(Vec3::with_w(q[0], q[1], q[2], q[3])).into(),
    )?;

    // dst aliases m2. The shim copies first; C uses a temporary. Same result.
    let mut rs_alias = ro;
    let mut c_alias = co;
    let rs_ptr = &mut rs_alias as *mut matrix3;
    let c_ptr = &mut c_alias as *mut OracleMatrix3;
    unsafe {
        rs::matrix3_mul(rs_ptr, &rm, rs_ptr);
        c::oracle_matrix3_mul(c_ptr, &cm, c_ptr);
    }
    assert_both("mul alias m2", rs_alias, c_alias, (core * &core_o).into())?;

    let mut rs_alias = rm;
    let mut c_alias = cm;
    let rs_ptr = &mut rs_alias as *mut matrix3;
    let c_ptr = &mut c_alias as *mut OracleMatrix3;
    unsafe {
        rs::matrix3_inv(rs_ptr, rs_ptr);
        c::oracle_matrix3_inv(c_ptr, c_ptr);
    }
    assert_both("inv alias", rs_alias, c_alias, core.inv(core).into())?;

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
    fn matrix3_matches_c_oracle(
        rows in mat(),
        other in mat(),
        q in row(),
        dist in any_bits(),
    ) {
        check(rows, other, q, dist)?;
    }
}

/// Fixed input from macOS CI: matrix conversion must use scalar C trig too.
#[test]
fn from_axisang_trig_regression_matches_c() {
    check(
        [[0.0; 4]; 4],
        [[0.0; 4]; 4],
        [1.1e-44, -1_625_024.0, 127_771.28, 42_494.33],
        5.739125e22,
    )
    .unwrap();
}
