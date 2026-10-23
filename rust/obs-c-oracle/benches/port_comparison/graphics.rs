use super::{measure_pair, pair};

unsafe extern "C" {
    fn srand(seed: std::ffi::c_uint);
}
use criterion::Criterion;
use obs_c_oracle::{graphics_math as c, vec2 as c2};
use obs_graphics::ffi::{
    matrix3::matrix3,
    matrix4::matrix4,
    plane::plane,
    vec2::{self as r2, vec2},
    vec3::{self as r3, vec3},
    vec4::{self as r4, vec4},
};
use std::hint::black_box;

macro_rules! output_pair {
    ($criterion:expr, $name:expr, $rust:path, $original:path, $rt:ty, $ct:ty, ($($rarg:expr),*), ($($carg:expr),*), [$($field:ident),+]) => {
        pair($criterion, $name, || {
            let mut out = <$rt>::default();
            // SAFETY: all inputs and the distinct output are live, aligned
            // structs with the ABI types required by this function.
            unsafe { $rust(&mut out, $(black_box($rarg)),*) };
            [$(out.$field),+]
        }, || {
            let mut out = <$ct>::default();
            // SAFETY: same validity guarantees for independent C ABI structs.
            unsafe { $original(&mut out, $(black_box($carg)),*) };
            [$(out.$field),+]
        });
    }
}

pub fn benchmarks(criterion: &mut Criterion) {
    let rv2 = vec2 { x: 3.0, y: -4.0 };
    let cv2 = c2::OracleVec2 { x: 3.0, y: -4.0 };
    output_pair!(
        criterion,
        "vec2/abs",
        r2::vec2_abs,
        c2::oracle_vec2_abs,
        vec2,
        c2::OracleVec2,
        (&rv2),
        (&cv2),
        [x, y]
    );
    output_pair!(
        criterion,
        "vec2/floor",
        r2::vec2_floor,
        c2::oracle_vec2_floor,
        vec2,
        c2::OracleVec2,
        (&rv2),
        (&cv2),
        [x, y]
    );
    output_pair!(
        criterion,
        "vec2/ceil",
        r2::vec2_ceil,
        c2::oracle_vec2_ceil,
        vec2,
        c2::OracleVec2,
        (&rv2),
        (&cv2),
        [x, y]
    );
    output_pair!(
        criterion,
        "vec2/norm",
        r2::vec2_norm,
        c2::oracle_vec2_norm,
        vec2,
        c2::OracleVec2,
        (&rv2),
        (&cv2),
        [x, y]
    );
    pair(
        criterion,
        "vec2/close",
        || {
            // SAFETY: both inputs are readable vec2 values.
            unsafe { r2::vec2_close(black_box(&rv2), &rv2, 0.001) }
        },
        || {
            // SAFETY: both inputs are readable oracle vec2 values.
            unsafe { c2::oracle_vec2_close(black_box(&cv2), &cv2, 0.001) }
        },
    );

    // Exactly representable fixtures isolate call/operation cost. Existing
    // Tier 3 tests cover arbitrary floats and SIMD rounding separately.
    let rv3 = vec3 {
        x: 1.25,
        y: -2.5,
        z: 3.75,
        w: 0.0,
    };
    let cv3 = c::OracleVec3 {
        x: 1.25,
        y: -2.5,
        z: 3.75,
        w: 0.0,
    };
    let rv4 = vec4 {
        x: 1.25,
        y: -2.5,
        z: 3.75,
        w: 1.0,
    };
    let cv4 = c::OracleVec4 {
        x: 1.25,
        y: -2.5,
        z: 3.75,
        w: 1.0,
    };
    let rx3 = vec3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
        w: 0.0,
    };
    let ry3 = vec3 {
        x: -1.0,
        y: 0.0,
        z: 0.0,
        w: 0.0,
    };
    let rz3 = vec3 {
        x: 0.0,
        y: 0.0,
        z: 1.0,
        w: 0.0,
    };
    let rt3 = vec3 {
        x: 4.0,
        y: 8.0,
        z: 16.0,
        w: 0.0,
    };
    let cx3 = c::OracleVec3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
        w: 0.0,
    };
    let cy3 = c::OracleVec3 {
        x: -1.0,
        y: 0.0,
        z: 0.0,
        w: 0.0,
    };
    let cz3 = c::OracleVec3 {
        x: 0.0,
        y: 0.0,
        z: 1.0,
        w: 0.0,
    };
    let ct3 = c::OracleVec3 {
        x: 4.0,
        y: 8.0,
        z: 16.0,
        w: 0.0,
    };
    let rm3 = matrix3 {
        x: rx3,
        y: ry3,
        z: rz3,
        t: rt3,
    };
    let cm3 = c::OracleMatrix3 {
        x: cx3,
        y: cy3,
        z: cz3,
        t: ct3,
    };
    let rm4 = matrix4 {
        x: vec4 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
            w: 0.0,
        },
        y: vec4 {
            x: -1.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        },
        z: vec4 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
            w: 0.0,
        },
        t: vec4 {
            x: 4.0,
            y: 8.0,
            z: 16.0,
            w: 1.0,
        },
    };
    let cm4 = c::OracleMatrix4 {
        x: c::OracleVec4 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
            w: 0.0,
        },
        y: c::OracleVec4 {
            x: -1.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        },
        z: c::OracleVec4 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
            w: 0.0,
        },
        t: c::OracleVec4 {
            x: 4.0,
            y: 8.0,
            z: 16.0,
            w: 1.0,
        },
    };
    let rp = plane {
        dir: rz3,
        dist: 2.0,
    };
    let cp = c::OraclePlane {
        dir: cz3,
        dist: 2.0,
    };
    output_pair!(
        criterion,
        "vec3/from_vec4",
        r3::vec3_from_vec4,
        c::oracle_vec3_from_vec4,
        vec3,
        c::OracleVec3,
        (&rv4),
        (&cv4),
        [x, y, z, w]
    );
    output_pair!(
        criterion,
        "vec3/rotate",
        r3::vec3_rotate,
        c::oracle_vec3_rotate,
        vec3,
        c::OracleVec3,
        (&rv3, &rm3),
        (&cv3, &cm3),
        [x, y, z, w]
    );
    output_pair!(
        criterion,
        "vec3/transform",
        r3::vec3_transform,
        c::oracle_vec3_transform,
        vec3,
        c::OracleVec3,
        (&rv3, &rm4),
        (&cv3, &cm4),
        [x, y, z, w]
    );
    output_pair!(
        criterion,
        "vec3/transform3x4",
        r3::vec3_transform3x4,
        c::oracle_vec3_transform3x4,
        vec3,
        c::OracleVec3,
        (&rv3, &rm3),
        (&cv3, &cm3),
        [x, y, z, w]
    );
    output_pair!(
        criterion,
        "vec3/mirror",
        r3::vec3_mirror,
        c::oracle_vec3_mirror,
        vec3,
        c::OracleVec3,
        (&rv3, &rp),
        (&cv3, &cp),
        [x, y, z, w]
    );
    output_pair!(
        criterion,
        "vec3/mirrorv",
        r3::vec3_mirrorv,
        c::oracle_vec3_mirrorv,
        vec3,
        c::OracleVec3,
        (&rv3, &rz3),
        (&cv3, &cz3),
        [x, y, z, w]
    );
    pair(
        criterion,
        "vec3/plane_dist",
        || {
            // SAFETY: the vector and plane are readable, aligned ABI structs.
            unsafe { r3::vec3_plane_dist(black_box(&rv3), &rp) }
        },
        || {
            // SAFETY: same validity guarantees for independent oracle structs.
            unsafe { c::oracle_vec3_plane_dist(black_box(&cv3), &cp) }
        },
    );
    output_pair!(
        criterion,
        "vec4/from_vec3",
        r4::vec4_from_vec3,
        c::oracle_vec4_from_vec3,
        vec4,
        c::OracleVec4,
        (&rv3),
        (&cv3),
        [x, y, z, w]
    );
    output_pair!(
        criterion,
        "vec4/transform",
        r4::vec4_transform,
        c::oracle_vec4_transform,
        vec4,
        c::OracleVec4,
        (&rv4, &rm4),
        (&cv4, &cm4),
        [x, y, z, w]
    );
    for positive in [0, 1] {
        let mut r = vec3::default();
        let mut original = c::OracleVec3::default();
        // SAFETY: srand/rand use the same CRT as the linked C oracle; the
        // single-threaded runner owns the RNG during this fixture check.
        unsafe {
            srand(42);
            r3::vec3_rand(&mut r, positive);
            srand(42);
            c::oracle_vec3_rand(&mut original, positive);
        }
        assert_eq!(
            [r.x, r.y, r.z, r.w],
            [original.x, original.y, original.z, original.w]
        );
        measure_pair(
            criterion,
            &format!("vec3/rand/{positive}"),
            || {
                let mut out = vec3::default();
                // SAFETY: out is writable and RNG access is single-threaded.
                unsafe { r3::vec3_rand(&mut out, black_box(positive)) };
                [out.x, out.y, out.z, out.w]
            },
            || {
                let mut out = c::OracleVec3::default();
                // SAFETY: same output and single-threaded RNG guarantees.
                unsafe { c::oracle_vec3_rand(&mut out, black_box(positive)) };
                [out.x, out.y, out.z, out.w]
            },
        );
    }
    texrender(criterion);
}

/// `gs_texrender_*` over the oracle's `gs_*` stand-in with recording off,
/// so the time is the state machine and its graphics calls.
fn texrender(criterion: &mut Criterion) {
    use core::ffi::c_void;
    use obs_c_oracle::texture_render as ct;
    use obs_graphics::ffi::texture_render as rt;
    use obs_graphics::texture_render::{ColorFormat, ColorSpace, ZStencilFormat};

    // SAFETY: the stand-in's state belongs to this single-threaded runner.
    unsafe {
        ct::oracle_texrender_device_reset();
        ct::oracle_texrender_device_quiet(true);
    }
    // SAFETY: plain values; it stays live until the end of this function.
    let rust = unsafe { rt::gs_texrender_create(ColorFormat(3), ZStencilFormat(2)) };
    // SAFETY: same, on the original C.
    let original = unsafe { ct::oracle_gs_texrender_create(3, 2) };
    pair(
        criterion,
        "texture_render/begin_end_reset",
        || {
            // SAFETY: a live texrender; begin, end and reset are one frame.
            unsafe {
                let begun = rt::gs_texrender_begin_with_color_space(
                    rust,
                    black_box(64),
                    black_box(32),
                    ColorSpace(black_box(2)),
                );
                rt::gs_texrender_end(rust);
                rt::gs_texrender_reset(rust);
                begun
            }
        },
        || {
            // SAFETY: same, on the original C.
            unsafe {
                let begun = ct::oracle_gs_texrender_begin_with_color_space(
                    original,
                    black_box(64),
                    black_box(32),
                    black_box(2),
                );
                ct::oracle_gs_texrender_end(original);
                ct::oracle_gs_texrender_reset(original);
                begun
            }
        },
    );
    pair(
        criterion,
        "texture_render/create_resize_destroy",
        || {
            // SAFETY: a fresh texrender, sized once and destroyed.
            unsafe {
                let t = rt::gs_texrender_create(ColorFormat(3), ZStencilFormat(2));
                let begun = rt::gs_texrender_begin(t, black_box(64), black_box(32));
                rt::gs_texrender_end(t);
                rt::gs_texrender_destroy(t);
                begun
            }
        },
        || {
            // SAFETY: same, on the original C.
            unsafe {
                let t: *mut c_void = ct::oracle_gs_texrender_create(3, 2);
                let begun = ct::oracle_gs_texrender_begin(t, black_box(64), black_box(32));
                ct::oracle_gs_texrender_end(t);
                ct::oracle_gs_texrender_destroy(t);
                begun
            }
        },
    );
    // SAFETY: both live, destroyed once.
    unsafe {
        rt::gs_texrender_destroy(rust);
        ct::oracle_gs_texrender_destroy(original);
        ct::oracle_texrender_device_quiet(false);
    }
}
