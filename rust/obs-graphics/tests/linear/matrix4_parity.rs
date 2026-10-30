//! Tier 3: the Rust C ABI shim and safe core behave exactly like the
//! original C, compiled as an oracle.
//!
//! No intentional differences from C. No ULP slack:
//! - `matrix4_transpose` is SIMD (`_mm_unpacklo_ps` / `_mm_unpackhi_ps`) in C
//!   and a scalar lane move in Rust. Both only permute bits, so equality is
//!   exact, including NaN payloads and signed zero.
//! - `matrix4_mul` and the helpers that call it use `vec4_dot`. [`Vec4::dot`]
//!   sums in the SSE order `(m3 + m1) + (m2 + m0)`, so those products match
//!   bit for bit.
//! - `matrix4_determinant` and `matrix4_inv` are scalar in C. The cutoff is
//!   `|det| < 0.0005`, and a rejected inverse leaves `dst` unchanged.
//!
//! `matrix4_mul_4x3_only` is not exported. `rotate_i`, `rotate_aa_i` and
//! `scale_i` cover it.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use obs_c_oracle::graphics_math::{
    self as c, OracleAxisAng, OracleMatrix3, OracleMatrix4, OracleQuat, OracleVec3, OracleVec4,
};
use obs_graphics::axisang::AxisAng;
use obs_graphics::ffi::axisang::axisang;
use obs_graphics::ffi::matrix3::matrix3;
use obs_graphics::ffi::matrix4::{self as rs, matrix4};
use obs_graphics::ffi::quat::quat;
use obs_graphics::ffi::vec3::vec3;
use obs_graphics::ffi::vec4::vec4;
use obs_graphics::matrix3::Matrix3;
use obs_graphics::matrix4::Matrix4;
use obs_graphics::quat::Quat;
use obs_graphics::vec3::Vec3;
use obs_graphics::vec4::Vec4;
use proptest::prelude::*;

/// Bit-exact equality, except that any NaN equals any NaN.
fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn same_row(r: vec4, c: OracleVec4) -> bool {
    same(r.x, c.x) && same(r.y, c.y) && same(r.z, c.z) && same(r.w, c.w)
}

fn same_m(r: matrix4, c: OracleMatrix4) -> bool {
    same_row(r.x, c.x) && same_row(r.y, c.y) && same_row(r.z, c.z) && same_row(r.t, c.t)
}

fn rs_row(v: [f32; 4]) -> vec4 {
    vec4 {
        x: v[0],
        y: v[1],
        z: v[2],
        w: v[3],
    }
}

fn c_row(v: [f32; 4]) -> OracleVec4 {
    OracleVec4 {
        x: v[0],
        y: v[1],
        z: v[2],
        w: v[3],
    }
}

fn rs_row3(v: [f32; 4]) -> vec3 {
    vec3 {
        x: v[0],
        y: v[1],
        z: v[2],
        w: v[3],
    }
}

fn c_row3(v: [f32; 4]) -> OracleVec3 {
    OracleVec3 {
        x: v[0],
        y: v[1],
        z: v[2],
        w: v[3],
    }
}

fn rs_m(rows: [[f32; 4]; 4]) -> matrix4 {
    matrix4 {
        x: rs_row(rows[0]),
        y: rs_row(rows[1]),
        z: rs_row(rows[2]),
        t: rs_row(rows[3]),
    }
}

fn c_m(rows: [[f32; 4]; 4]) -> OracleMatrix4 {
    OracleMatrix4 {
        x: c_row(rows[0]),
        y: c_row(rows[1]),
        z: c_row(rows[2]),
        t: c_row(rows[3]),
    }
}

fn core_m(rows: [[f32; 4]; 4]) -> Matrix4 {
    let row = |r: [f32; 4]| Vec4::new(r[0], r[1], r[2], r[3]);
    Matrix4::new(row(rows[0]), row(rows[1]), row(rows[2]), row(rows[3]))
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
    shim: matrix4,
    oracle: OracleMatrix4,
    core: matrix4,
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
fn check(rows: [[f32; 4]; 4], other: [[f32; 4]; 4], q: [f32; 4]) -> Result<(), TestCaseError> {
    let rm = rs_m(rows);
    let cm = c_m(rows);
    let ro = rs_m(other);
    let co = c_m(other);
    let rq = rs_q(q);
    let cq = c_q(q);
    let ra = rs_aa(q);
    let ca = c_aa(q);
    let rv = rs_row3(q);
    let cv = c_row3(q);
    let r4 = rs_row(q);
    let c4 = c_row(q);
    let core = core_m(rows);
    let core_o = core_m(other);
    let quat = Quat::new(q[0], q[1], q[2], q[3]);
    let axis = AxisAng::new(q[0], q[1], q[2], q[3]);
    let vec3 = Vec3::with_w(q[0], q[1], q[2], q[3]);
    let vec4 = Vec4::new(q[0], q[1], q[2], q[3]);

    let mut rs_dst = matrix4::default();
    let mut c_dst = OracleMatrix4::default();

    let r3 = matrix3 {
        x: rs_row3(rows[0]),
        y: rs_row3(rows[1]),
        z: rs_row3(rows[2]),
        t: rs_row3(rows[3]),
    };
    let c3 = OracleMatrix3 {
        x: c_row3(rows[0]),
        y: c_row3(rows[1]),
        z: c_row3(rows[2]),
        t: c_row3(rows[3]),
    };
    // SAFETY: every pointer refers to a live local.
    unsafe {
        rs::matrix4_from_matrix3(&mut rs_dst, &r3);
        c::oracle_matrix4_from_matrix3(&mut c_dst, &c3);
    }
    assert_both(
        "from_matrix3",
        rs_dst,
        c_dst,
        Matrix4::from_matrix3(Matrix3::new(
            Vec3::with_w(rows[0][0], rows[0][1], rows[0][2], rows[0][3]),
            Vec3::with_w(rows[1][0], rows[1][1], rows[1][2], rows[1][3]),
            Vec3::with_w(rows[2][0], rows[2][1], rows[2][2], rows[2][3]),
            Vec3::with_w(rows[3][0], rows[3][1], rows[3][2], rows[3][3]),
        ))
        .into(),
    )?;

    unsafe {
        rs::matrix4_from_quat(&mut rs_dst, &rq);
        c::oracle_matrix4_from_quat(&mut c_dst, &cq);
    }
    assert_both("from_quat", rs_dst, c_dst, Matrix4::from_quat(quat).into())?;

    unsafe {
        rs::matrix4_from_axisang(&mut rs_dst, &ra);
        c::oracle_matrix4_from_axisang(&mut c_dst, &ca);
    }
    assert_both(
        "from_axisang",
        rs_dst,
        c_dst,
        Matrix4::from_axisang(axis).into(),
    )?;

    unsafe {
        rs::matrix4_mul(&mut rs_dst, &rm, &ro);
        c::oracle_matrix4_mul(&mut c_dst, &cm, &co);
    }
    assert_both("mul", rs_dst, c_dst, (core * &core_o).into())?;

    let rs_det = unsafe { rs::matrix4_determinant(&rm) };
    let c_det = unsafe { c::oracle_matrix4_determinant(&cm) };
    prop_assert!(same(rs_det, c_det), "det shim {rs_det} != C {c_det}");
    prop_assert!(
        same(core.determinant(), c_det),
        "det core {} != C {c_det}",
        core.determinant()
    );

    unsafe {
        rs::matrix4_translate3v(&mut rs_dst, &rm, &rv);
        c::oracle_matrix4_translate3v(&mut c_dst, &cm, &cv);
    }
    assert_both("translate3v", rs_dst, c_dst, core.translate3v(vec3).into())?;

    unsafe {
        rs::matrix4_translate4v(&mut rs_dst, &rm, &r4);
        c::oracle_matrix4_translate4v(&mut c_dst, &cm, &c4);
    }
    assert_both("translate4v", rs_dst, c_dst, core.translate4v(vec4).into())?;

    unsafe {
        rs::matrix4_rotate(&mut rs_dst, &rm, &rq);
        c::oracle_matrix4_rotate(&mut c_dst, &cm, &cq);
    }
    assert_both("rotate", rs_dst, c_dst, core.rotate(quat).into())?;

    unsafe {
        rs::matrix4_rotate_aa(&mut rs_dst, &rm, &ra);
        c::oracle_matrix4_rotate_aa(&mut c_dst, &cm, &ca);
    }
    assert_both("rotate_aa", rs_dst, c_dst, core.rotate_aa(axis).into())?;

    unsafe {
        rs::matrix4_scale(&mut rs_dst, &rm, &rv);
        c::oracle_matrix4_scale(&mut c_dst, &cm, &cv);
    }
    assert_both("scale", rs_dst, c_dst, core.scale(vec3).into())?;

    unsafe {
        rs::matrix4_translate3v_i(&mut rs_dst, &rv, &rm);
        c::oracle_matrix4_translate3v_i(&mut c_dst, &cv, &cm);
    }
    assert_both(
        "translate3v_i",
        rs_dst,
        c_dst,
        core.translate3v_i(vec3).into(),
    )?;

    unsafe {
        rs::matrix4_translate4v_i(&mut rs_dst, &r4, &rm);
        c::oracle_matrix4_translate4v_i(&mut c_dst, &c4, &cm);
    }
    assert_both(
        "translate4v_i",
        rs_dst,
        c_dst,
        core.translate4v_i(vec4).into(),
    )?;

    unsafe {
        rs::matrix4_rotate_i(&mut rs_dst, &rq, &rm);
        c::oracle_matrix4_rotate_i(&mut c_dst, &cq, &cm);
    }
    assert_both("rotate_i", rs_dst, c_dst, core.rotate_i(quat).into())?;

    unsafe {
        rs::matrix4_rotate_aa_i(&mut rs_dst, &ra, &rm);
        c::oracle_matrix4_rotate_aa_i(&mut c_dst, &ca, &cm);
    }
    assert_both("rotate_aa_i", rs_dst, c_dst, core.rotate_aa_i(axis).into())?;

    unsafe {
        rs::matrix4_scale_i(&mut rs_dst, &rv, &rm);
        c::oracle_matrix4_scale_i(&mut c_dst, &cv, &cm);
    }
    assert_both("scale_i", rs_dst, c_dst, core.scale_i(vec3).into())?;

    unsafe {
        rs::matrix4_transpose(&mut rs_dst, &rm);
        c::oracle_matrix4_transpose(&mut c_dst, &cm);
    }
    assert_both("transpose", rs_dst, c_dst, core.transpose().into())?;

    // Previous dst is `other`. A rejected inverse must leave it unchanged.
    rs_dst = ro;
    c_dst = co;
    let rs_ok = unsafe { rs::matrix4_inv(&mut rs_dst, &rm) };
    let c_ok = unsafe { c::oracle_matrix4_inv(&mut c_dst, &cm) };
    prop_assert_eq!(rs_ok, c_ok, "inv bool");
    let core_inv = core.try_inv().unwrap_or(core_o);
    assert_both("inv", rs_dst, c_dst, core_inv.into())?;

    // dst aliases the input. C copies first; the shim reads first.
    let mut rs_alias = rm;
    let mut c_alias = cm;
    let rs_ptr = &mut rs_alias as *mut matrix4;
    let c_ptr = &mut c_alias as *mut OracleMatrix4;
    unsafe {
        rs::matrix4_mul(rs_ptr, rs_ptr, &ro);
        c::oracle_matrix4_mul(c_ptr, c_ptr, &co);
    }
    assert_both("mul alias m1", rs_alias, c_alias, (core * &core_o).into())?;

    let mut rs_alias = rm;
    let mut c_alias = cm;
    let rs_ptr = &mut rs_alias as *mut matrix4;
    let c_ptr = &mut c_alias as *mut OracleMatrix4;
    let rs_ok = unsafe { rs::matrix4_inv(rs_ptr, rs_ptr) };
    let c_ok = unsafe { c::oracle_matrix4_inv(c_ptr, c_ptr) };
    prop_assert_eq!(rs_ok, c_ok, "inv alias bool");
    let core_inv = core.try_inv().unwrap_or(core);
    assert_both("inv alias", rs_alias, c_alias, core_inv.into())?;

    let mut rs_alias = rm;
    let mut c_alias = cm;
    let rs_ptr = &mut rs_alias as *mut matrix4;
    let c_ptr = &mut c_alias as *mut OracleMatrix4;
    unsafe {
        rs::matrix4_transpose(rs_ptr, rs_ptr);
        c::oracle_matrix4_transpose(c_ptr, c_ptr);
    }
    assert_both(
        "transpose alias",
        rs_alias,
        c_alias,
        core.transpose().into(),
    )?;

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
    fn matrix4_matches_c_oracle(
        rows in mat(),
        other in mat(),
        q in row(),
    ) {
        check(rows, other, q)?;
    }
}
