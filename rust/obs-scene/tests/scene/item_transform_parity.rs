//! Tier 3: the shims against the original `obs-scene-item-transform.c.h`.
//!
//! No intentional differences, so every comparison is bit for bit: each
//! float by its bits (a NaN by its payload and sign too), `-0` apart from
//! `+0`. The C oracle's `matrix4_*`, `quat_from_axisang` and crop calls are
//! the C graphics and crop oracles, so the C side is all C.
//!
//! - the layout of `struct obs_scene_item_transform_size` and
//!   `OBS_BOUNDS_NONE` against the C header;
//! - exhaustive over the flags: `prepare` over nested and each last size
//!   0 or not, for special crops and sizes; `bounded` over -2..=8 and the
//!   `i32` ends;
//! - proptests (20,000 cases each): `prepare` over any crops and sizes;
//!   `matrix` over finite floats (any finite bit pattern, and item-sized
//!   values) and rotations, with every value's bits; `box_scale` over any
//!   bits; and (2,000 cases, 448 compositions each)
//!   `compute_item_transform()`'s sequence (`compose`) with the shims on
//!   one side and the oracle on the other, exhaustive over the
//!   flags (absolute, update_tex, nested scene, group, parent group, crop
//!   to bounds) and over every bounds type and the 16 alignments, for
//!   random positions, scales, rotations, crops, bounds and sizes.
//!
//! `compose` gives both sides the same glue (the ported coordinate, bounds
//! placement and alignment functions, and x86-64's cast done in `i64`), so
//! only the four steps differ; the glue has its own Tier 3 tests.
//! `test_scene_item_transform_steps` compiles the C into itself and
//! `test_scene_item_transform_steps_rust` runs it against these shims.

use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::graphics_math::{OracleMatrix4, OracleVec4};
use obs_c_oracle::scene_bounds_place::OracleCrop;
use obs_c_oracle::scene_item_transform::{
    LAYOUT_NAMES, oracle_obs_scene_item_transform_bounded,
    oracle_obs_scene_item_transform_box_scale, oracle_obs_scene_item_transform_matrix,
    oracle_obs_scene_item_transform_prepare, oracle_scene_item_transform_layout,
};
use obs_c_oracle::vec2::OracleVec2;
use obs_graphics::ffi::matrix4::matrix4;
use obs_graphics::ffi::vec2::vec2;
use obs_graphics::vec2::Vec2;
use obs_scene::bounds::BoundsType;
use obs_scene::bounds_place::{self, Crop, Placement};
use obs_scene::coords;
use obs_scene::ffi::scene_bounds_place::SceneItemCrop;
use obs_scene::ffi::scene_item_transform::{
    obs_scene_item_transform_bounded, obs_scene_item_transform_box_scale,
    obs_scene_item_transform_matrix, obs_scene_item_transform_prepare,
    obs_scene_item_transform_size,
};
use proptest::prelude::*;

fn matrix_bits(m: &matrix4) -> [u32; 16] {
    [
        m.x.x, m.x.y, m.x.z, m.x.w, m.y.x, m.y.y, m.y.z, m.y.w, m.z.x, m.z.y, m.z.z, m.z.w, m.t.x,
        m.t.y, m.t.z, m.t.w,
    ]
    .map(f32::to_bits)
}

fn oracle_matrix_bits(m: &OracleMatrix4) -> [u32; 16] {
    let r = |v: &OracleVec4| [v.x, v.y, v.z, v.w];
    let mut out = [0; 16];
    for (i, lane) in [r(&m.x), r(&m.y), r(&m.z), r(&m.t)]
        .into_iter()
        .flatten()
        .enumerate()
    {
        out[i] = lane.to_bits();
    }
    out
}

fn garbage_matrix() -> matrix4 {
    let g = f32::from_bits(0x5a5a_5a5a);
    let v = obs_graphics::ffi::vec4::vec4 {
        x: g,
        y: g,
        z: g,
        w: g,
    };
    matrix4 {
        x: v,
        y: v,
        z: v,
        t: v,
    }
}

fn garbage_oracle_matrix() -> OracleMatrix4 {
    let g = f32::from_bits(0xa5a5_a5a5);
    let v = OracleVec4 {
        x: g,
        y: g,
        z: g,
        w: g,
    };
    OracleMatrix4 {
        x: v,
        y: v,
        z: v,
        t: v,
    }
}

// ------------------------------------------------------------ the steps

/// Each side's prepare: (crop, bounds crop, last size, cx, cy).
type PrepareOut = ([i32; 4], [i32; 4], (u32, u32), (u32, u32));

fn rust_prepare(
    crop: [i32; 4],
    bc: [i32; 4],
    lw: u32,
    lh: u32,
    w: u32,
    h: u32,
    nested: bool,
) -> PrepareOut {
    let mk = |c: [i32; 4]| SceneItemCrop {
        left: c[0],
        top: c[1],
        right: c[2],
        bottom: c[3],
    };
    let (mut c, mut b, mut lw, mut lh) = (mk(crop), mk(bc), lw, lh);
    // SAFETY: four distinct live locals, as obs-scene.c passes four fields.
    let size = unsafe {
        obs_scene_item_transform_prepare(
            &raw mut c,
            &raw mut b,
            &raw mut lw,
            &raw mut lh,
            w,
            h,
            nested,
        )
    };
    (
        [c.left, c.top, c.right, c.bottom],
        [b.left, b.top, b.right, b.bottom],
        (lw, lh),
        (size.cx, size.cy),
    )
}

fn c_prepare(
    crop: [i32; 4],
    bc: [i32; 4],
    lw: u32,
    lh: u32,
    w: u32,
    h: u32,
    nested: bool,
) -> PrepareOut {
    let mk = |c: [i32; 4]| OracleCrop {
        left: c[0],
        top: c[1],
        right: c[2],
        bottom: c[3],
    };
    let (mut c, mut b, mut lw, mut lh) = (mk(crop), mk(bc), lw, lh);
    // SAFETY: four distinct live locals.
    let size = unsafe {
        oracle_obs_scene_item_transform_prepare(
            &raw mut c,
            &raw mut b,
            &raw mut lw,
            &raw mut lh,
            w,
            h,
            nested,
        )
    };
    (
        [c.left, c.top, c.right, c.bottom],
        [b.left, b.top, b.right, b.bottom],
        (lw, lh),
        (size.cx, size.cy),
    )
}

fn check_prepare(crop: [i32; 4], bc: [i32; 4], lw: u32, lh: u32, w: u32, h: u32, nested: bool) {
    assert_eq!(
        rust_prepare(crop, bc, lw, lh, w, h, nested),
        c_prepare(crop, bc, lw, lh, w, h, nested),
        "prepare crop {crop:?} bounds crop {bc:?} last {lw}x{lh} size {w}x{h} nested {nested}"
    );
}

fn rust_matrix(s: (f32, f32), o: (f32, f32), rot: f32, p: (f32, f32)) -> [u32; 16] {
    let mut m = garbage_matrix();
    let v = |(x, y)| vec2 { x, y };
    let (s, o, p) = (v(s), v(o), v(p));
    // SAFETY: live locals.
    unsafe {
        obs_scene_item_transform_matrix(&raw mut m, &raw const s, &raw const o, rot, &raw const p)
    };
    matrix_bits(&m)
}

fn c_matrix(s: (f32, f32), o: (f32, f32), rot: f32, p: (f32, f32)) -> [u32; 16] {
    let mut m = garbage_oracle_matrix();
    let v = |(x, y)| OracleVec2 { x, y };
    let (s, o, p) = (v(s), v(o), v(p));
    // SAFETY: live locals.
    unsafe {
        oracle_obs_scene_item_transform_matrix(
            &raw mut m,
            &raw const s,
            &raw const o,
            rot,
            &raw const p,
        )
    };
    oracle_matrix_bits(&m)
}

fn check_matrix(s: (f32, f32), o: (f32, f32), rot: f32, p: (f32, f32)) {
    assert_eq!(
        rust_matrix(s, o, rot, p),
        c_matrix(s, o, rot, p),
        "matrix scale {s:?} origin {o:?} rot {rot} position {p:?}"
    );
}

fn rust_box_scale(s: (f32, f32), w: u32, h: u32) -> (u32, u32) {
    let mut v = vec2 { x: s.0, y: s.1 };
    // SAFETY: a live local.
    unsafe { obs_scene_item_transform_box_scale(&raw mut v, w, h) };
    (v.x.to_bits(), v.y.to_bits())
}

fn c_box_scale(s: (f32, f32), w: u32, h: u32) -> (u32, u32) {
    let mut v = OracleVec2 { x: s.0, y: s.1 };
    // SAFETY: a live local.
    unsafe { oracle_obs_scene_item_transform_box_scale(&raw mut v, w, h) };
    (v.x.to_bits(), v.y.to_bits())
}

#[test]
fn layout_matches_the_c() {
    let rust = [
        size_of::<obs_scene_item_transform_size>(),
        align_of::<obs_scene_item_transform_size>(),
        offset_of!(obs_scene_item_transform_size, cx),
        offset_of!(obs_scene_item_transform_size, cy),
        0,
        size_of::<i32>(),
    ];
    for (i, (name, want)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: in range.
        let c = unsafe { oracle_scene_item_transform_layout(i) };
        assert_eq!(c, want, "{name}");
    }
    assert_eq!(BoundsType::None as i32, 0);
    assert_eq!(size_of::<SceneItemCrop>(), size_of::<OracleCrop>());
}

#[test]
fn bounded_exhaustive() {
    for t in (-2..=8).chain([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX]) {
        // SAFETY: plain values.
        let c = unsafe { oracle_obs_scene_item_transform_bounded(t) };
        assert_eq!(obs_scene_item_transform_bounded(t), c, "bounded({t})");
    }
}

fn special_crops() -> Vec<[i32; 4]> {
    vec![
        [0, 0, 0, 0],
        [10, 20, 30, 40],
        [100, 100, 101, 1],
        [-5, 0, 3, -1],
        [i32::MAX, 0, 0, i32::MAX],
        [i32::MIN, i32::MAX, i32::MIN, 1],
        [i32::MAX; 4],
        [1000, -3, 0, 7],
    ]
}

fn special_sizes() -> Vec<u32> {
    vec![
        0,
        1,
        2,
        3,
        100,
        960,
        1080,
        1920,
        16_777_216,
        16_777_217,
        0x7fff_ffff,
        0x8000_0000,
        u32::MAX,
    ]
}

#[test]
fn prepare_flags_exhaustive() {
    let sizes = special_sizes();
    for crop in special_crops() {
        for nested in [false, true] {
            for &lw in &sizes {
                for &lh in &[0, 1, 540, u32::MAX] {
                    for &(w, h) in &[
                        (0, 0),
                        (1920, 1080),
                        (960, 540),
                        (u32::MAX, 2),
                        (16_777_217, 1),
                    ] {
                        check_prepare(crop, [7, -8, i32::MAX, i32::MIN], lw, lh, w, h, nested);
                    }
                }
            }
        }
    }
}

#[test]
fn matrix_special_values() {
    let vals = [
        0.0_f32,
        -0.0,
        1.0,
        -1.0,
        0.5,
        1.5,
        -1.5,
        1920.0,
        -3e9,
        f32::MAX,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::from_bits(0xffc0_1234),
    ];
    let rots = [
        0.0_f32,
        -0.0,
        90.0,
        45.0,
        -30.0,
        720.0,
        180.0,
        1e30,
        f32::NAN,
        f32::INFINITY,
    ];
    for &a in &vals {
        for &b in &vals {
            for &rot in &rots {
                check_matrix((a, b), (b, a), rot, (a, 1.0));
                check_matrix((1.5, a), (100.0, b), rot, (b, 50.0));
            }
        }
    }
}

#[test]
fn box_scale_special_values() {
    let vals = [
        0.0_f32,
        -0.0,
        1.0,
        -1.5,
        0.1,
        f32::MAX,
        f32::INFINITY,
        f32::NAN,
        f32::from_bits(0x7fc0_1234),
    ];
    for &a in &vals {
        for &b in &vals {
            for &(w, h) in &[(0, 0), (1, 1920), (16_777_217, u32::MAX), (3, 7)] {
                assert_eq!(
                    rust_box_scale((a, b), w, h),
                    c_box_scale((a, b), w, h),
                    "box scale {a} {b} {w} {h}"
                );
            }
        }
    }
}

// ------------------------------------------------------ the composition

/// The four steps, as one side calls them. `prepare` takes the C's seven
/// arguments, as `rust_prepare` and `c_prepare` do.
trait Steps {
    #[allow(clippy::too_many_arguments)]
    fn prepare(
        &self,
        crop: &mut Crop,
        bc: &mut Crop,
        lw: &mut u32,
        lh: &mut u32,
        w: u32,
        h: u32,
        nested: bool,
    ) -> (u32, u32);
    fn bounded(&self, t: i32) -> bool;
    fn matrix(&self, scale: Vec2, origin: Vec2, rot: f32, position: Vec2) -> [u32; 16];
    fn box_scale(&self, scale: Vec2, w: u32, h: u32) -> Vec2;
}

struct Shims;
struct Oracle;

fn to4(c: Crop) -> [i32; 4] {
    [c.left, c.top, c.right, c.bottom]
}

fn from4(c: [i32; 4]) -> Crop {
    Crop {
        left: c[0],
        top: c[1],
        right: c[2],
        bottom: c[3],
    }
}

impl Steps for Shims {
    fn prepare(
        &self,
        crop: &mut Crop,
        bc: &mut Crop,
        lw: &mut u32,
        lh: &mut u32,
        w: u32,
        h: u32,
        nested: bool,
    ) -> (u32, u32) {
        let (c, b, (nw, nh), size) = rust_prepare(to4(*crop), to4(*bc), *lw, *lh, w, h, nested);
        (*crop, *bc, *lw, *lh) = (from4(c), from4(b), nw, nh);
        size
    }
    fn bounded(&self, t: i32) -> bool {
        obs_scene_item_transform_bounded(t)
    }
    fn matrix(&self, s: Vec2, o: Vec2, rot: f32, p: Vec2) -> [u32; 16] {
        rust_matrix((s.x, s.y), (o.x, o.y), rot, (p.x, p.y))
    }
    fn box_scale(&self, s: Vec2, w: u32, h: u32) -> Vec2 {
        let (x, y) = rust_box_scale((s.x, s.y), w, h);
        Vec2::new(f32::from_bits(x), f32::from_bits(y))
    }
}

impl Steps for Oracle {
    fn prepare(
        &self,
        crop: &mut Crop,
        bc: &mut Crop,
        lw: &mut u32,
        lh: &mut u32,
        w: u32,
        h: u32,
        nested: bool,
    ) -> (u32, u32) {
        let (c, b, (nw, nh), size) = c_prepare(to4(*crop), to4(*bc), *lw, *lh, w, h, nested);
        (*crop, *bc, *lw, *lh) = (from4(c), from4(b), nw, nh);
        size
    }
    fn bounded(&self, t: i32) -> bool {
        // SAFETY: a plain value.
        unsafe { oracle_obs_scene_item_transform_bounded(t) }
    }
    fn matrix(&self, s: Vec2, o: Vec2, rot: f32, p: Vec2) -> [u32; 16] {
        c_matrix((s.x, s.y), (o.x, o.y), rot, (p.x, p.y))
    }
    fn box_scale(&self, s: Vec2, w: u32, h: u32) -> Vec2 {
        let (x, y) = c_box_scale((s.x, s.y), w, h);
        Vec2::new(f32::from_bits(x), f32::from_bits(y))
    }
}

#[derive(Debug, Clone, Copy)]
struct Item {
    pos: Vec2,
    scale: Vec2,
    scale_ref_y: f32,
    rot: f32,
    align: u32,
    crop: Crop,
    bounds_crop: Crop,
    absolute: bool,
    is_scene: bool,
    is_group: bool,
    parent_is_group: bool,
    last_width: u32,
    last_height: u32,
    bounds_type: i32,
    bounds_align: u32,
    bounds: Vec2,
    crop_to_bounds: bool,
}

/// Everything one composition leaves, as bits.
#[derive(Debug, PartialEq, Eq)]
struct Composed {
    draw: [u32; 16],
    boxed: [u32; 16],
    output_scale: (u32, u32),
    box_scale: (u32, u32),
    crop: [i32; 4],
    bounds_crop: [i32; 4],
    last: (u32, u32),
    cast: (u32, u32),
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn x86_cast_u32(f: f32) -> u32 {
    (f as i64) as u64 as u32
}

#[allow(clippy::cast_possible_wrap)]
fn as_int(v: u32) -> i32 {
    v as i32
}

#[allow(clippy::cast_possible_truncation)]
fn float_to_int(f: f32) -> i32 {
    f as i32
}

#[allow(clippy::cast_precision_loss)]
fn compose(
    steps: &dyn Steps,
    mut it: Item,
    w: u32,
    h: u32,
    update_tex: bool,
    canvas: (f32, f32),
) -> Composed {
    let nudge = coords::coords_nudge(it.is_group, it.parent_is_group);
    let (mut cx, mut cy) = steps.prepare(
        &mut it.crop,
        &mut it.bounds_crop,
        &mut it.last_width,
        &mut it.last_height,
        w,
        h,
        update_tex && it.is_scene,
    );
    let (width, height) = (cx, cy);
    let mut origin = Vec2::new(0.0, 0.0);
    let (mut scale, position) = if it.absolute {
        (it.scale, it.pos)
    } else {
        let scale = if coords::coords_keep_scale(it.is_group, it.is_scene) {
            it.scale
        } else {
            coords::item_canvas_scale(it.scale, canvas.1, it.scale_ref_y)
        };
        (
            scale,
            coords::pos_to_absolute(it.pos, canvas.0, canvas.1, nudge),
        )
    };
    if steps.bounded(it.bounds_type) {
        let bounds = if it.absolute {
            it.bounds
        } else {
            coords::size_to_absolute(it.bounds, canvas.1, nudge)
        };
        let placed = bounds_place::bounds_place(
            BoundsType::from_c(it.bounds_type),
            bounds,
            Placement {
                origin,
                scale,
                cx,
                cy,
                crop: it.bounds_crop,
            },
            it.bounds_align,
            it.crop_to_bounds,
        );
        (origin, scale, cx, cy, it.bounds_crop) = (
            placed.origin,
            placed.scale,
            placed.cx,
            placed.cy,
            placed.crop,
        );
    } else {
        cx = x86_cast_u32(width as f32 * scale.x);
        cy = x86_cast_u32(height as f32 * scale.y);
    }
    origin = bounds_place::add_alignment(origin, it.align, as_int(cx), as_int(cy));
    let draw = steps.matrix(scale, origin, it.rot, position);
    let output_scale = scale;
    scale = if steps.bounded(it.bounds_type) {
        if it.absolute {
            it.bounds
        } else {
            coords::size_to_absolute(it.bounds, canvas.1, nudge)
        }
    } else {
        steps.box_scale(scale, width, height)
    };
    let base_origin = bounds_place::add_alignment(
        Vec2::new(0.0, 0.0),
        it.align,
        float_to_int(scale.x),
        float_to_int(scale.y),
    );
    let boxed = steps.matrix(scale, base_origin, it.rot, position);
    Composed {
        draw,
        boxed,
        output_scale: (output_scale.x.to_bits(), output_scale.y.to_bits()),
        box_scale: (scale.x.to_bits(), scale.y.to_bits()),
        crop: to4(it.crop),
        bounds_crop: to4(it.bounds_crop),
        last: (it.last_width, it.last_height),
        cast: (cx, cy),
    }
}

/// Finite floats: any finite bit pattern, item-sized values, and the
/// values a scene uses.
fn finite() -> impl Strategy<Value = f32> {
    prop_oneof![
        any::<u32>()
            .prop_map(f32::from_bits)
            .prop_filter("finite", |f| f.is_finite()),
        -5000.0_f32..5000.0,
        -4.0_f32..4.0,
        prop::sample::select(vec![
            0.0_f32, -0.0, 1.0, -1.0, 0.5, 1.5, -1.5, 1920.0, 1080.0, 90.0, 45.0, -30.0, 720.0
        ]),
    ]
}

/// Values the scene's own casts and the bounds placement take without
/// leaving the C's defined range: a scale and a position within the
/// canvas's reach, bounds that are positive and below 2^31.
fn item_strategy() -> impl Strategy<Value = (Item, u32, u32)> {
    let crop = prop::array::uniform4(prop_oneof![Just(0_i32), 0_i32..400, -50_i32..0]);
    (
        (
            -2000.0_f32..2000.0,
            -2000.0_f32..2000.0,
            -8.0_f32..8.0,
            -8.0_f32..8.0,
            1.0_f32..4000.0,
        ),
        (
            -1000.0_f32..1000.0,
            0_u32..16,
            crop.clone(),
            crop,
            prop_oneof![Just(0_u32), 1_u32..4000],
        ),
        (
            prop_oneof![Just(0_u32), 1_u32..4000],
            0_u32..16,
            1.0_f32..3000.0,
            1.0_f32..3000.0,
        ),
        (0_u32..3000, 0_u32..3000),
    )
        .prop_map(
            |((px, py, sx, sy, sref), (rot, align, crop, bc, lw), (lh, balign, bx, by), (w, h))| {
                (
                    Item {
                        pos: Vec2::new(px, py),
                        scale: Vec2::new(sx, sy),
                        scale_ref_y: sref,
                        rot,
                        align,
                        crop: from4(crop),
                        bounds_crop: from4(bc),
                        absolute: true,
                        is_scene: false,
                        is_group: false,
                        parent_is_group: false,
                        last_width: lw,
                        last_height: lh,
                        bounds_type: 0,
                        bounds_align: balign,
                        bounds: Vec2::new(bx, by),
                        crop_to_bounds: false,
                    },
                    w,
                    h,
                )
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn prepare_any(
        crop in prop::array::uniform4(any::<i32>()),
        bc in prop::array::uniform4(any::<i32>()),
        lw in prop_oneof![Just(0_u32), any::<u32>(), 1_u32..4000],
        lh in prop_oneof![Just(0_u32), any::<u32>(), 1_u32..4000],
        w in prop_oneof![Just(0_u32), any::<u32>(), 1_u32..4000],
        h in prop_oneof![Just(0_u32), any::<u32>(), 1_u32..4000],
        nested in any::<bool>(),
    ) {
        prop_assert_eq!(
            rust_prepare(crop, bc, lw, lh, w, h, nested),
            c_prepare(crop, bc, lw, lh, w, h, nested)
        );
    }

    #[test]
    fn matrix_finite_floats(
        s in (finite(), finite()), o in (finite(), finite()), rot in finite(), p in (finite(), finite()),
    ) {
        prop_assert_eq!(rust_matrix(s, o, rot, p), c_matrix(s, o, rot, p));
    }

    #[test]
    fn box_scale_any_bits(sx in any::<u32>(), sy in any::<u32>(), w in any::<u32>(), h in any::<u32>()) {
        let s = (f32::from_bits(sx), f32::from_bits(sy));
        prop_assert_eq!(rust_box_scale(s, w, h), c_box_scale(s, w, h));
    }

}

proptest! {
    // 448 compositions on each side per case
    #![proptest_config(ProptestConfig::with_cases(2_000))]

    #[test]
    fn compose_flags_exhaustive((it, w, h) in item_strategy(), canvas in prop::sample::select(vec![(1920.0_f32, 1080.0_f32), (1280.0, 720.0), (0.0, 0.0), (3840.0, 2160.0)])) {
        // 6 flags x 7 bounds types for each random item; the alignment
        // and bounds alignment cover all 16 values across the cases
        for flags in 0..64_u32 {
            for t in 0..=6 {
                let mut it = it;
                it.absolute = flags & 1 != 0;
                let update_tex = flags & 2 != 0;
                it.is_scene = flags & 4 != 0;
                it.is_group = flags & 8 != 0;
                it.parent_is_group = flags & 16 != 0;
                it.crop_to_bounds = flags & 32 != 0;
                it.bounds_type = t;
                if !it.absolute {
                    // relative values are fractions of the canvas
                    it.pos = Vec2::new(it.pos.x / 2000.0, it.pos.y / 2000.0);
                    it.bounds = Vec2::new(it.bounds.x / 3000.0, it.bounds.y / 3000.0);
                }
                prop_assert_eq!(
                    compose(&Shims, it, w, h, update_tex, canvas),
                    compose(&Oracle, it, w, h, update_tex, canvas),
                    "flags {:#x} bounds type {}", flags, t
                );
            }
        }
    }
}
