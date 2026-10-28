//! Tier 3: the Rust C ABI shim and safe core behave exactly like the
//! original C, compiled as an oracle.
//!
//! No intentional differences. `plane_from_tri` accepts `v1 == &dst->dir` and
//! uses the original vertex for the distance, as C does since the alias
//! repair. `plane_intersection_line` leaves `*t` unchanged on a miss.
//! `vec3_cross` / `vec3_norm` follow the header SSE, so parity is bit-exact
//! (0 ULP).
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ptr;

use obs_c_oracle::graphics_math::{
    self as c, OracleMatrix3, OracleMatrix4, OraclePlane, OracleVec3, OracleVec4,
};
use obs_graphics::ffi::matrix3::matrix3;
use obs_graphics::ffi::matrix4::matrix4;
use obs_graphics::ffi::plane::{self as rs, plane};
use obs_graphics::ffi::vec3::vec3;
use obs_graphics::ffi::vec4::vec4;
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

fn same_plane(r: plane, c: OraclePlane) -> bool {
    same_vec(r.dir, c.dir) && same(r.dist, c.dist)
}

fn same_plane_rs(r: plane, c: plane) -> bool {
    same(r.dir.x, c.dir.x)
        && same(r.dir.y, c.dir.y)
        && same(r.dir.z, c.dir.z)
        && same(r.dist, c.dist)
}

fn same_plane_cs(c: OraclePlane, d: OraclePlane) -> bool {
    same(c.dir.x, d.dir.x)
        && same(c.dir.y, d.dir.y)
        && same(c.dir.z, d.dir.z)
        && same(c.dist, d.dist)
}

fn rs_v(v: [f32; 4]) -> vec3 {
    vec3 {
        x: v[0],
        y: v[1],
        z: v[2],
        w: v[3],
    }
}

fn c_v(v: [f32; 4]) -> OracleVec3 {
    OracleVec3 {
        x: v[0],
        y: v[1],
        z: v[2],
        w: v[3],
    }
}

fn core_v(v: [f32; 4]) -> Vec3 {
    Vec3::with_w(v[0], v[1], v[2], v[3])
}

fn rs_v4(v: [f32; 4]) -> vec4 {
    vec4 {
        x: v[0],
        y: v[1],
        z: v[2],
        w: v[3],
    }
}

fn c_v4(v: [f32; 4]) -> OracleVec4 {
    OracleVec4 {
        x: v[0],
        y: v[1],
        z: v[2],
        w: v[3],
    }
}

fn rs_m3(rows: [[f32; 4]; 4]) -> matrix3 {
    matrix3 {
        x: rs_v(rows[0]),
        y: rs_v(rows[1]),
        z: rs_v(rows[2]),
        t: rs_v(rows[3]),
    }
}

fn c_m3(rows: [[f32; 4]; 4]) -> OracleMatrix3 {
    OracleMatrix3 {
        x: c_v(rows[0]),
        y: c_v(rows[1]),
        z: c_v(rows[2]),
        t: c_v(rows[3]),
    }
}

fn core_m3(rows: [[f32; 4]; 4]) -> Matrix3 {
    Matrix3::new(
        core_v(rows[0]),
        core_v(rows[1]),
        core_v(rows[2]),
        core_v(rows[3]),
    )
}

fn rs_m4(rows: [[f32; 4]; 4]) -> matrix4 {
    matrix4 {
        x: rs_v4(rows[0]),
        y: rs_v4(rows[1]),
        z: rs_v4(rows[2]),
        t: rs_v4(rows[3]),
    }
}

fn c_m4(rows: [[f32; 4]; 4]) -> OracleMatrix4 {
    OracleMatrix4 {
        x: c_v4(rows[0]),
        y: c_v4(rows[1]),
        z: c_v4(rows[2]),
        t: c_v4(rows[3]),
    }
}

fn core_m4(rows: [[f32; 4]; 4]) -> Matrix4 {
    Matrix4::new(
        Vec4::new(rows[0][0], rows[0][1], rows[0][2], rows[0][3]),
        Vec4::new(rows[1][0], rows[1][1], rows[1][2], rows[1][3]),
        Vec4::new(rows[2][0], rows[2][1], rows[2][2], rows[2][3]),
        Vec4::new(rows[3][0], rows[3][1], rows[3][2], rows[3][3]),
    )
}

fn rs_p(dir: [f32; 4], dist: f32) -> plane {
    plane {
        dir: rs_v(dir),
        dist,
    }
}

fn c_p(dir: [f32; 4], dist: f32) -> OraclePlane {
    OraclePlane {
        dir: c_v(dir),
        dist,
    }
}

struct Case {
    dir: [f32; 4],
    dist: f32,
    v1: [f32; 4],
    v2: [f32; 4],
    v3: [f32; 4],
    m3: [[f32; 4]; 4],
    m4: [[f32; 4]; 4],
    precision: f32,
}

#[expect(
    clippy::too_many_lines,
    reason = "parity table: one block per ported C case"
)]
fn check(case: Case) -> Result<(), TestCaseError> {
    let Case {
        dir,
        dist,
        v1,
        v2,
        v3,
        m3,
        m4,
        precision,
    } = case;
    let mut rs_dst = rs_p(dir, dist);
    let mut c_dst = c_p(dir, dist);
    let (ra, rb, rc) = (rs_v(v1), rs_v(v2), rs_v(v3));
    let (ca, cb, cc) = (c_v(v1), c_v(v2), c_v(v3));
    // SAFETY: every pointer addresses a live local of the matching C type.
    unsafe {
        rs::plane_from_tri(&mut rs_dst, &ra, &rb, &rc);
        c::oracle_plane_from_tri(&mut c_dst, &ca, &cb, &cc);
    }
    let core = Plane::from_tri(core_v(v1), core_v(v2), core_v(v3));
    prop_assert!(
        same_plane(rs_dst, c_dst),
        "from_tri shim {rs_dst:?} != C {c_dst:?}"
    );
    prop_assert!(
        same_plane(core.into(), c_dst),
        "from_tri core {core:?} != C {c_dst:?}"
    );

    let src = rs_p(dir, dist);
    let csrc = c_p(dir, dist);
    let rm4 = rs_m4(m4);
    let cm4 = c_m4(m4);
    // SAFETY: as above.
    unsafe {
        rs::plane_transform(&mut rs_dst, &src, &rm4);
        c::oracle_plane_transform(&mut c_dst, &csrc, &cm4);
    }
    let core = Plane::new(core_v(dir), dist).transform(&core_m4(m4));
    prop_assert!(
        same_plane(rs_dst, c_dst),
        "transform shim {rs_dst:?} != C {c_dst:?}"
    );
    prop_assert!(
        same_plane(core.into(), c_dst),
        "transform core {core:?} != C {c_dst:?}"
    );

    let rm3 = rs_m3(m3);
    let cm3 = c_m3(m3);
    // SAFETY: as above.
    unsafe {
        rs::plane_transform3x4(&mut rs_dst, &src, &rm3);
        c::oracle_plane_transform3x4(&mut c_dst, &csrc, &cm3);
    }
    let core = Plane::new(core_v(dir), dist).transform3x4(&core_m3(m3));
    prop_assert!(
        same_plane(rs_dst, c_dst),
        "transform3x4 shim {rs_dst:?} != C {c_dst:?}"
    );
    prop_assert!(
        same_plane(core.into(), c_dst),
        "transform3x4 core {core:?} != C {c_dst:?}"
    );

    let mut rt = 42.0f32;
    let mut ct = 42.0f32;
    // SAFETY: as above. Both sides always write `t`.
    let (rh, ch) = unsafe {
        (
            rs::plane_intersection_ray(&src, &ra, &rb, &mut rt),
            c::oracle_plane_intersection_ray(&csrc, &ca, &cb, &mut ct),
        )
    };
    prop_assert_eq!(rh, ch, "ray hit");
    prop_assert!(same(rt, ct), "ray t {rt} != {ct}");
    let (core_hit, core_t) = Plane::new(core_v(dir), dist).intersection_ray(core_v(v1), core_v(v2));
    prop_assert_eq!(core_hit, ch, "ray core hit");
    prop_assert!(same(core_t, ct), "ray core t {core_t} != {ct}");

    rt = 42.0;
    ct = 42.0;
    // SAFETY: a miss does not dereference `t`; a hit writes the local.
    let (rh, ch) = unsafe {
        (
            rs::plane_intersection_line(&src, &ra, &rb, &mut rt),
            c::oracle_plane_intersection_line(&csrc, &ca, &cb, &mut ct),
        )
    };
    prop_assert_eq!(rh, ch, "line hit");
    prop_assert!(same(rt, ct), "line t {rt} != {ct}");
    let core_line = Plane::new(core_v(dir), dist).intersection_line(core_v(v1), core_v(v2));
    prop_assert_eq!(core_line.is_some(), ch, "line core hit");
    if let Some(value) = core_line {
        prop_assert!(same(value, ct), "line core t {value} != {ct}");
    } else {
        prop_assert!(same(rt, 42.0), "line miss wrote {rt}");
    }

    // SAFETY: as above.
    let (rh, ch) = unsafe {
        (
            rs::plane_tri_inside(&src, &ra, &rb, &rc, precision),
            c::oracle_plane_tri_inside(&csrc, &ca, &cb, &cc, precision),
        )
    };
    prop_assert_eq!(rh, ch, "tri_inside");
    prop_assert_eq!(
        Plane::new(core_v(dir), dist).tri_inside(core_v(v1), core_v(v2), core_v(v3), precision),
        ch,
        "tri_inside core"
    );

    // SAFETY: as above.
    let (rh, ch) = unsafe {
        (
            rs::plane_line_inside(&src, &ra, &rb, precision),
            c::oracle_plane_line_inside(&csrc, &ca, &cb, precision),
        )
    };
    prop_assert_eq!(rh, ch, "line_inside");
    prop_assert_eq!(
        Plane::new(core_v(dir), dist).line_inside(core_v(v1), core_v(v2), precision),
        ch,
        "line_inside core"
    );
    Ok(())
}

#[test]
fn plane_from_tri_v1_alias_matches_c() {
    let mut rs_dst = rs_p([0.0, 0.0, 1.0, 0.0], 0.0);
    let mut c_dst = c_p([0.0, 0.0, 1.0, 0.0], 0.0);
    let rb = rs_v([1.0, 0.0, 0.0, 0.0]);
    let rc = rs_v([0.0, 1.0, 0.0, 0.0]);
    let cb = c_v([1.0, 0.0, 0.0, 0.0]);
    let cc = c_v([0.0, 1.0, 0.0, 0.0]);
    // SAFETY: `v1` points at `dst->dir`, which is the alias the C function allows.
    unsafe {
        let dst = &mut rs_dst as *mut plane;
        rs::plane_from_tri(dst, ptr::addr_of!((*dst).dir), &rb, &rc);
        let dst = &mut c_dst as *mut OraclePlane;
        c::oracle_plane_from_tri(dst, ptr::addr_of!((*dst).dir), &cb, &cc);
    }
    assert!(
        same_plane(rs_dst, c_dst),
        "alias shim {rs_dst:?} != C {c_dst:?}"
    );
    /* the distance is the dot with the original vertex now, not dir·dir */
    assert!(same(rs_dst.dist, 0.57735026), "alias dist {}", rs_dst.dist);

    let mut rs_ref = plane::default();
    let mut c_ref = OraclePlane::default();
    let ra = rs_v([0.0, 0.0, 1.0, 0.0]);
    let ca = c_v([0.0, 0.0, 1.0, 0.0]);
    // SAFETY: all pointers refer to live locals, with distinct destinations.
    unsafe {
        rs::plane_from_tri(&mut rs_ref, &ra, &rb, &rc);
        c::oracle_plane_from_tri(&mut c_ref, &ca, &cb, &cc);
    }
    assert!(
        same_plane_rs(rs_dst, rs_ref),
        "alias {rs_dst:?} != separate {rs_ref:?}"
    );
    assert!(
        same_plane_cs(c_dst, c_ref),
        "C alias {c_dst:?} != separate {c_ref:?}"
    );
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
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn plane_matches_c_oracle(
        dir in row(),
        dist in any_bits(),
        v1 in row(),
        v2 in row(),
        v3 in row(),
        m3 in mat(),
        m4 in mat(),
        precision in any_bits(),
    ) {
        check(Case { dir, dist, v1, v2, v3, m3, m4, precision })?;
    }
}
