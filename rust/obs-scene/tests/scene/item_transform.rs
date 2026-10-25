//! Tier 1 mirror of `test/cmocka/test_scene_item_transform_steps.c`, and
//! edge cases.
//!
//! The `steps_*` tests port the C cases 1:1, under the same names, in the
//! same order, with the same inputs and pins. `compose` runs the steps in
//! `compute_item_transform()`'s order, with what `obs-scene.c.h` keeps
//! between them: the canvas size `get_scene_dimensions()` reads (given
//! here), the ported coordinate, bounds placement and alignment functions,
//! and the casts. The two `(uint32_t)((float)cx * scale.x)` of an item
//! without bounds are x86-64's result, through `i64` (`x86_cast_u32`), so
//! the pins are the same on every architecture, as in the C test; the
//! scene keeps the C cast itself.
//!
//! The rotation cases pin the matrix bits only with glibc on x86-64, the C
//! library the bits were taken from; everywhere they check the matrix
//! against the same `Matrix4` calls in the C's order.
//!
//! The `edge_*` tests add the values the C cases do not reach.

use obs_graphics::axisang::AxisAng;
use obs_graphics::matrix4::Matrix4;
use obs_graphics::vec2::Vec2;
use obs_graphics::vec3::Vec3;
use obs_graphics::vec4::Vec4;
use obs_scene::bounds::BoundsType;
use obs_scene::bounds_place::{self, Crop, Placement};
use obs_scene::coords;
use obs_scene::item_transform::{self, Prepared, RAD};
use proptest::prelude::*;

const ALIGN_CENTER: u32 = 0;
const ALIGN_LEFT: u32 = 1;
const ALIGN_RIGHT: u32 = 2;
const ALIGN_TOP: u32 = 4;
const ALIGN_BOTTOM: u32 = 8;

const BOUNDS_NONE: i32 = 0;
const BOUNDS_STRETCH: i32 = 1;
const BOUNDS_SCALE_INNER: i32 = 2;
const BOUNDS_SCALE_OUTER: i32 = 3;
const BOUNDS_MAX_ONLY: i32 = 6;

const ALIGNMENTS: [u32; 11] = [
    ALIGN_CENTER,
    ALIGN_LEFT,
    ALIGN_RIGHT,
    ALIGN_TOP,
    ALIGN_TOP | ALIGN_LEFT,
    ALIGN_TOP | ALIGN_RIGHT,
    ALIGN_BOTTOM,
    ALIGN_BOTTOM | ALIGN_LEFT,
    ALIGN_BOTTOM | ALIGN_RIGHT,
    // extra bits: both sides of each axis (right and bottom win), and a
    // bit above the four
    ALIGN_LEFT | ALIGN_RIGHT | ALIGN_TOP | ALIGN_BOTTOM,
    0x10 | ALIGN_TOP | ALIGN_LEFT,
];
const N_ALIGN: usize = ALIGNMENTS.len();

const SIGN_X: [f32; 4] = [1.0, -1.0, 1.0, -1.0];
const SIGN_Y: [f32; 4] = [1.0, 1.0, -1.0, -1.0];

const SRC_CX: u32 = 200;
const SRC_CY: u32 = 100;

fn crop_of(left: i32, top: i32, right: i32, bottom: i32) -> Crop {
    Crop {
        left,
        top,
        right,
        bottom,
    }
}

const NO_CROP: Crop = Crop {
    left: 0,
    top: 0,
    right: 0,
    bottom: 0,
};

fn lanes(m: &Matrix4) -> [f32; 16] {
    [
        m.x.x, m.x.y, m.x.z, m.x.w, m.y.x, m.y.y, m.y.z, m.y.w, m.z.x, m.z.y, m.z.z, m.z.w, m.t.x,
        m.t.y, m.t.z, m.t.w,
    ]
}

fn bits16(m: &Matrix4) -> [u32; 16] {
    lanes(m).map(f32::to_bits)
}

#[track_caller]
fn check_bits(actual: f32, expected: u32, what: &str) {
    assert_eq!(
        actual.to_bits(),
        expected,
        "{what}: {actual} (0x{:08x}), expected {} (0x{expected:08x})",
        actual.to_bits(),
        f32::from_bits(expected)
    );
}

#[track_caller]
fn check_matrix(m: &Matrix4, want: &[u32; 16], what: &str) {
    assert_eq!(&bits16(m), want, "{what}");
}

#[track_caller]
fn check_crop(c: Crop, left: i32, top: i32, right: i32, bottom: i32, what: &str) {
    assert_eq!(c, crop_of(left, top, right, bottom), "{what}");
}

/// The item fields `compute_item_transform()` reads and writes.
#[derive(Debug, Clone, Copy)]
struct Item {
    pos: Vec2,
    scale: Vec2,
    scale_ref: Vec2,
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

    draw: Matrix4,
    boxed: Matrix4,
    output_scale: Vec2,
    box_scale: Vec2,
    // the size after the bounds or the cast, which add_alignment() gets
    cx: u32,
    cy: u32,
}

/// A 200 x 100 source at (100, 50) on a 1920 x 1080 canvas, scale 1.5,
/// top-left, no bounds, no crop, absolute coordinates.
fn base_item() -> Item {
    Item {
        pos: Vec2::new(100.0, 50.0),
        scale: Vec2::new(1.5, 1.5),
        scale_ref: Vec2::new(1920.0, 1080.0),
        rot: 0.0,
        align: ALIGN_TOP | ALIGN_LEFT,
        crop: NO_CROP,
        bounds_crop: NO_CROP,
        absolute: true,
        is_scene: false,
        is_group: false,
        parent_is_group: false,
        last_width: 0,
        last_height: 0,
        bounds_type: BOUNDS_NONE,
        bounds_align: ALIGN_CENTER,
        bounds: Vec2::new(800.0, 600.0),
        crop_to_bounds: false,
        draw: Matrix4::default(),
        boxed: Matrix4::default(),
        output_scale: Vec2::new(0.0, 0.0),
        box_scale: Vec2::new(0.0, 0.0),
        cx: 0,
        cy: 0,
    }
}

/// x86-64's `(uint32_t)` of a float: a 64-bit signed conversion, then the
/// low 32 bits. The C cast on x86-64 for the finite values below 2^63 the
/// test uses.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn x86_cast_u32(f: f32) -> u32 {
    (f as i64) as u64 as u32
}

#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
fn as_int(v: u32) -> i32 {
    v as i32
}

#[allow(clippy::cast_possible_truncation)]
fn float_to_int(f: f32) -> i32 {
    f as i32
}

fn compose(it: &mut Item, width: u32, height: u32, update_tex: bool, canvas_x: f32, canvas_y: f32) {
    let nudge = coords::coords_nudge(it.is_group, it.parent_is_group);

    let p: Prepared = item_transform::prepare(
        it.crop,
        it.last_width,
        it.last_height,
        width,
        height,
        update_tex && it.is_scene,
    );
    it.crop = p.crop;
    it.bounds_crop = p.bounds_crop;
    it.last_width = p.last_width;
    it.last_height = p.last_height;
    let (mut cx, mut cy) = (p.cx, p.cy);
    let (width, height) = (cx, cy);

    let mut origin = Vec2::new(0.0, 0.0);
    let (mut scale, position);
    if it.absolute {
        scale = it.scale;
        position = it.pos;
    } else {
        scale = if coords::coords_keep_scale(it.is_group, it.is_scene) {
            it.scale
        } else {
            coords::item_canvas_scale(it.scale, canvas_y, it.scale_ref.y)
        };
        position = coords::pos_to_absolute(it.pos, canvas_x, canvas_y, nudge);
    }

    if item_transform::bounded(it.bounds_type) {
        let bounds = if it.absolute {
            it.bounds
        } else {
            coords::size_to_absolute(it.bounds, canvas_y, nudge)
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
        origin = placed.origin;
        scale = placed.scale;
        cx = placed.cx;
        cy = placed.cy;
        it.bounds_crop = placed.crop;
    } else {
        cx = x86_cast_u32(width as f32 * scale.x);
        cy = x86_cast_u32(height as f32 * scale.y);
    }
    it.cx = cx;
    it.cy = cy;

    origin = bounds_place::add_alignment(origin, it.align, as_int(cx), as_int(cy));
    it.draw = item_transform::matrix(scale, origin, it.rot, position);
    it.output_scale = scale;

    scale = if item_transform::bounded(it.bounds_type) {
        if it.absolute {
            it.bounds
        } else {
            coords::size_to_absolute(it.bounds, canvas_y, nudge)
        }
    } else {
        item_transform::box_scale(scale, width, height)
    };
    it.box_scale = scale;

    let base_origin = bounds_place::add_alignment(
        Vec2::new(0.0, 0.0),
        it.align,
        float_to_int(scale.x),
        float_to_int(scale.y),
    );
    it.boxed = item_transform::matrix(scale, base_origin, it.rot, position);
}

const FNV_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

fn fnv_u32(mut h: u64, v: u32) -> u64 {
    for byte in v.to_le_bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[allow(clippy::cast_sign_loss)]
fn fnv_item(mut h: u64, it: &Item) -> u64 {
    for lane in lanes(&it.draw).into_iter().chain(lanes(&it.boxed)) {
        h = fnv_u32(h, lane.to_bits());
    }
    for v in [
        it.output_scale.x,
        it.output_scale.y,
        it.box_scale.x,
        it.box_scale.y,
    ] {
        h = fnv_u32(h, v.to_bits());
    }
    for c in [it.crop, it.bounds_crop] {
        for v in [c.left, c.top, c.right, c.bottom] {
            h = fnv_u32(h, v as u32);
        }
    }
    for v in [it.last_width, it.last_height, it.cx, it.cy] {
        h = fnv_u32(h, v);
    }
    h
}

/// The matrix calls `compute_item_transform()` made, in its order.
fn reference_matrix(sx: f32, sy: f32, ox: f32, oy: f32, rot: f32, px: f32, py: f32) -> Matrix4 {
    identity()
        .scale(Vec3::new(sx, sy, 1.0))
        .translate3v(Vec3::new(-ox, -oy, 0.0))
        .rotate_aa(AxisAng::new(0.0, 0.0, 1.0, rot * RAD))
        .translate3v(Vec3::new(px, py, 0.0))
}

fn identity() -> Matrix4 {
    Matrix4::new(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

fn matrix_of(sx: f32, sy: f32, ox: f32, oy: f32, rot: f32, px: f32, py: f32) -> Matrix4 {
    item_transform::matrix(Vec2::new(sx, sy), Vec2::new(ox, oy), rot, Vec2::new(px, py))
}

fn prepare(crop: &mut Crop, lw: &mut u32, lh: &mut u32, w: u32, h: u32, nested: bool) -> Prepared {
    let p = item_transform::prepare(*crop, *lw, *lh, w, h, nested);
    *crop = p.crop;
    *lw = p.last_width;
    *lh = p.last_height;
    p
}

// --------------------------------------------------------------------------

#[test]
fn steps_layout() {
    assert_eq!(BoundsType::None as i32, 0);
    assert_eq!(BoundsType::Stretch as i32, 1);
    assert_eq!(BoundsType::ScaleInner as i32, 2);
    assert_eq!(BoundsType::ScaleOuter as i32, 3);
    assert_eq!(BoundsType::ScaleToWidth as i32, 4);
    assert_eq!(BoundsType::ScaleToHeight as i32, 5);
    assert_eq!(BoundsType::MaxOnly as i32, 6);
    assert_eq!(bounds_place::CENTER, ALIGN_CENTER);
    assert_eq!(bounds_place::LEFT, ALIGN_LEFT);
    assert_eq!(bounds_place::RIGHT, ALIGN_RIGHT);
    assert_eq!(bounds_place::TOP, ALIGN_TOP);
    assert_eq!(bounds_place::BOTTOM, ALIGN_BOTTOM);
    // RAD() is one float multiply by this constant
    check_bits(1.0 * RAD, 0x3c8e_fa35, "RAD(1)");
}

#[test]
fn steps_prepare_crop() {
    let mut crop;
    let mut lw;
    let mut lh;

    // no crop: the whole source; the bounds crop is zeroed whatever it
    // was; the last size becomes the source size
    crop = NO_CROP;
    lw = 11;
    lh = 22;
    let p = prepare(&mut crop, &mut lw, &mut lh, 1920, 1080, false);
    assert_eq!(p.cx, 1920, "no crop cx");
    assert_eq!(p.cy, 1080, "no crop cy");
    check_crop(p.bounds_crop, 0, 0, 0, 0, "bounds crop reset");
    assert_eq!(lw, 1920, "last width");
    assert_eq!(lh, 1080, "last height");

    // a crop on each side, not nested: the crop is kept, even with a size
    // change
    crop = crop_of(10, 20, 30, 40);
    lw = 960;
    lh = 540;
    let p = prepare(&mut crop, &mut lw, &mut lh, 1920, 1080, false);
    assert_eq!(p.cx, 1880, "cropped cx");
    assert_eq!(p.cy, 1020, "cropped cy");
    check_crop(crop, 10, 20, 30, 40, "crop kept");

    // cropping all of it: 0; more: 2; a negative sum: 2 (characterized,
    // not endorsed); two INT_MAX crops sum in 64 bits
    crop = crop_of(100, 0, 100, 0);
    let p = prepare(&mut crop, &mut lw, &mut lh, 200, 100, false);
    assert_eq!(p.cx, 0, "cropped to nothing");
    crop = crop_of(100, 0, 101, 0);
    let p = prepare(&mut crop, &mut lw, &mut lh, 200, 100, false);
    assert_eq!(p.cx, 2, "cropped past the size");
    crop = crop_of(-5, 0, 0, 0);
    let p = prepare(&mut crop, &mut lw, &mut lh, 200, 100, false);
    assert_eq!(p.cx, 2, "negative crop");
    crop = crop_of(i32::MAX, i32::MAX, i32::MAX, i32::MAX);
    let p = prepare(&mut crop, &mut lw, &mut lh, u32::MAX, u32::MAX, false);
    assert_eq!(p.cx, 1, "two INT_MAX crops of UINT32_MAX");
    assert_eq!(p.cy, 1, "two INT_MAX crops of UINT32_MAX, y");
    crop = crop_of(i32::MAX, 0, 0, 0);
    let p = prepare(&mut crop, &mut lw, &mut lh, u32::MAX, 0, false);
    assert_eq!(p.cx, u32::MAX - 0x7fff_ffff, "INT_MAX crop of UINT32_MAX");
    assert_eq!(p.cy, 0, "zero height");
    assert_eq!(lw, u32::MAX, "last width UINT32_MAX");
    assert_eq!(lh, 0, "last height 0");
}

#[test]
fn steps_prepare_nested() {
    let mut crop;
    let mut lw;
    let mut lh;

    // last_width 0 (never sized): the crop is kept
    crop = crop_of(10, 20, 30, 40);
    lw = 0;
    lh = 540;
    let p = prepare(&mut crop, &mut lw, &mut lh, 1920, 1080, true);
    check_crop(crop, 10, 20, 30, 40, "last width 0");
    assert_eq!(p.cx, 1880, "last width 0 cx");
    assert_eq!(lw, 1920, "last width set");

    // last_height 0
    crop = crop_of(10, 20, 30, 40);
    lw = 960;
    lh = 0;
    prepare(&mut crop, &mut lw, &mut lh, 1920, 1080, true);
    check_crop(crop, 10, 20, 30, 40, "last height 0");

    // the nested scene doubled: so does its crop
    crop = crop_of(10, 20, 30, 40);
    lw = 960;
    lh = 540;
    let p = prepare(&mut crop, &mut lw, &mut lh, 1920, 1080, true);
    check_crop(crop, 20, 40, 60, 80, "doubled");
    assert_eq!(p.cx, 1840, "doubled cx");
    assert_eq!(p.cy, 960, "doubled cy");

    // the same size: unchanged
    prepare(&mut crop, &mut lw, &mut lh, 1920, 1080, true);
    check_crop(crop, 20, 40, 60, 80, "same size");

    // shrunk to two thirds: truncated toward 0
    crop = crop_of(100, 100, 1, 2);
    lw = 1920;
    lh = 1080;
    prepare(&mut crop, &mut lw, &mut lh, 1280, 720, true);
    check_crop(crop, 66, 66, 0, 1, "two thirds");

    // a different factor per axis
    crop = crop_of(10, 10, 10, 10);
    lw = 100;
    lh = 100;
    prepare(&mut crop, &mut lw, &mut lh, 300, 50, true);
    check_crop(crop, 30, 5, 30, 5, "per axis");

    // to 0 x 0: every crop 0
    crop = crop_of(10, 20, 30, 40);
    lw = 1920;
    lh = 1080;
    let p = prepare(&mut crop, &mut lw, &mut lh, 0, 0, true);
    check_crop(crop, 0, 0, 0, 0, "to zero");
    assert_eq!(p.cx, 0, "to zero cx");
    assert_eq!(lw, 0, "last width 0 after");

    // a huge factor saturates; a negative crop scales negative
    // (characterized, not endorsed)
    crop = crop_of(1000, -3, 0, 7);
    lw = 1;
    lh = 1;
    let p = prepare(&mut crop, &mut lw, &mut lh, u32::MAX, 2, true);
    check_crop(crop, i32::MAX, -6, 0, 14, "saturated and negative");
    assert_eq!(p.cx, 2_147_483_648, "saturated cx");
    assert_eq!(p.cy, 2, "negative crop cy");

    // sizes past 2^24 convert to float rounded: 16777217 / 16777216 is 1
    crop = crop_of(1000, 0, 0, 0);
    lw = 16_777_216;
    lh = 1;
    prepare(&mut crop, &mut lw, &mut lh, 16_777_217, 1, true);
    check_crop(crop, 1000, 0, 0, 0, "past 2^24");
}

#[test]
fn steps_bounded() {
    assert!(!item_transform::bounded(BOUNDS_NONE), "NONE");
    for t in BOUNDS_STRETCH..=BOUNDS_MAX_ONLY {
        assert!(item_transform::bounded(t), "bounds type {t}");
    }
    // Characterized, not endorsed: anything but 0 is bounded. A value
    // outside the enum is OBS_BOUNDS_NONE before compute_item_transform()
    // runs (#2281, test_scene_bounds_type_range).
    for t in [7, -1, i32::MAX, i32::MIN] {
        assert!(item_transform::bounded(t), "{t}");
    }
}

#[test]
fn steps_box_scale() {
    let s = item_transform::box_scale(Vec2::new(1.5, -2.0), 1920, 1080);
    check_bits(s.x, 0x4534_0000, "1.5 * 1920");
    check_bits(s.y, 0xc507_0000, "-2 * 1080");

    // the size converts to float first, rounded: 16777217 is 16777216
    let s = item_transform::box_scale(Vec2::new(1.0, 3.0), 16_777_217, u32::MAX);
    check_bits(s.x, 0x4b80_0000, "16777217 as float");
    check_bits(s.y, 0x5040_0000, "3 * UINT32_MAX as float");

    // one rounding of the float product
    let s = item_transform::box_scale(Vec2::new(0.1, 1.0 / 3.0), 3, 7);
    check_bits(s.x, 0x3e99_999a, "0.1 * 3");
    check_bits(s.y, 0x4015_5556, "1/3 * 7");

    // zero size and signed zeros
    let s = item_transform::box_scale(Vec2::new(-1.0, 1.0), 0, 0);
    check_bits(s.x, 0x8000_0000, "-1 * 0");
    check_bits(s.y, 0x0000_0000, "1 * 0");

    // infinities and NaN pass through the multiply
    let s = item_transform::box_scale(Vec2::new(f32::INFINITY, f32::from_bits(0x7fc0_1234)), 0, 5);
    assert!(s.x.is_nan(), "inf * 0");
    check_bits(s.y, 0x7fc0_1234, "NaN payload");
}

#[test]
fn steps_matrix_rot0() {
    // the identity
    let m = matrix_of(1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let identity: [u32; 16] = [
        0x3f80_0000,
        0,
        0,
        0,
        0,
        0x3f80_0000,
        0,
        0,
        0,
        0,
        0x3f80_0000,
        0,
        0,
        0,
        0,
        0x3f80_0000,
    ];
    check_matrix(&m, &identity, "identity");

    // scale (2, 3), origin (10, 20), position (100, 200): the origin is in
    // scaled units, so it is subtracted as it is, then the position added
    let m = matrix_of(2.0, 3.0, 10.0, 20.0, 0.0, 100.0, 200.0);
    let scaled: [u32; 16] = [
        0x4000_0000,
        0,
        0,
        0,
        0,
        0x4040_0000,
        0,
        0,
        0,
        0,
        0x3f80_0000,
        0,
        0x42b4_0000,
        0x4334_0000,
        0,
        0x3f80_0000,
    ];
    check_matrix(&m, &scaled, "scaled");

    // flipped: the scale's sign, and the signed zeros it makes
    let m = matrix_of(-1.5, -1.5, 300.0, 150.0, 0.0, 100.0, 50.0);
    let flipped: [u32; 16] = [
        0xbfc00000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0xbfc00000, 0x00000000,
        0x00000000, 0x00000000, 0x00000000, 0x3f800000, 0x00000000, 0xc3480000, 0xc2c80000,
        0x00000000, 0x3f800000,
    ];
    check_matrix(&m, &flipped, "flipped");

    // -0 rotation is exact too
    let m = matrix_of(2.0, 3.0, 10.0, 20.0, -0.0, 100.0, 200.0);
    check_matrix(&m, &scaled, "-0 rotation");
}

#[test]
fn steps_matrix_rotations() {
    let rots: [f32; 8] = [0.0, 90.0, 45.0, -30.0, 720.0, 180.0, 360.0, -90.0];
    let rad_bits: [u32; 8] = [
        0x00000000, 0x3fc90fdb, 0x3f490fdb, 0xbf060a92, 0x41490fdb, 0x40490fdb, 0x40c90fdb,
        0xbfc90fdb,
    ];
    for (rot, bits) in rots.into_iter().zip(rad_bits) {
        check_bits(rot * RAD, bits, "RAD(rot)");
        // each order of the same calls gives the matrix the C makes
        let m = matrix_of(1.5, -2.0, 150.0, 50.0, rot, 100.0, 50.0);
        let r = reference_matrix(1.5, -2.0, 150.0, 50.0, rot, 100.0, 50.0);
        assert_eq!(bits16(&m), bits16(&r), "the matrix calls in order, {rot}");
    }

    // rotating before the origin translation is a different matrix
    let m = matrix_of(1.5, -2.0, 150.0, 50.0, 45.0, 100.0, 50.0);
    let r = identity()
        .scale(Vec3::new(1.5, -2.0, 1.0))
        .rotate_aa(AxisAng::new(0.0, 0.0, 1.0, 45.0 * RAD))
        .translate3v(Vec3::new(-150.0, -50.0, 0.0))
        .translate3v(Vec3::new(100.0, 50.0, 0.0));
    assert_ne!(bits16(&m), bits16(&r), "rotation before the origin");

    #[cfg(all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"))]
    {
        // the bits, with glibc's sinf and cosf
        let rot90: [u32; 16] = [
            0x00000000, 0x3fc00000, 0x00000000, 0x00000000, 0x40000000, 0x00000000, 0x00000000,
            0x00000000, 0x00000000, 0x00000000, 0x3f800000, 0x00000000, 0x43160000, 0xc2c80000,
            0x00000000, 0x3f800000,
        ];
        let rot45: [u32; 16] = [
            0x3f87c3b6, 0x3f87c3b7, 0x00000000, 0x00000000, 0x3fb504f4, 0xbfb504f2, 0x00000000,
            0x00000000, 0x00000000, 0x00000000, 0x3f800000, 0x00000000, 0x41ea5090, 0xc2b6d7bc,
            0x00000000, 0x3f800000,
        ];
        let rotm30: [u32; 16] = [
            0x3fa646e1, 0xbf400002, 0x00000000, 0x00000000, 0xbf800001, 0xbfddb3d7, 0x00000000,
            0x00000000, 0x00000000, 0x00000000, 0x3f800000, 0x00000000, 0xc25b9d80, 0x42a365c1,
            0x00000000, 0x3f800000,
        ];
        let rot720: [u32; 16] = [
            0x3fc00000, 0x350ccde2, 0x00000000, 0x00000000, 0x353bbd2e, 0xc0000000, 0x00000000,
            0x00000000, 0x00000000, 0x00000000, 0x3f800000, 0x00000000, 0xc247fffc, 0xb8600000,
            0x00000000, 0x3f800000,
        ];
        let at = |rot| matrix_of(1.5, -2.0, 150.0, 50.0, rot, 100.0, 50.0);
        check_matrix(&at(90.0), &rot90, "90");
        check_matrix(&at(45.0), &rot45, "45");
        check_matrix(&at(-30.0), &rotm30, "-30");
        check_matrix(&at(720.0), &rot720, "720");
    }
}

#[track_caller]
fn check_placed(it: &Item, want: &[u32; 4], what: &str) {
    check_bits(it.draw.t.x, want[0], what);
    check_bits(it.draw.t.y, want[1], what);
    check_bits(it.boxed.t.x, want[2], what);
    check_bits(it.boxed.t.y, want[3], what);
}

#[test]
fn steps_compose_alignments() {
    // An unbounded 200 x 100 source at (100, 50), scale 1.5: the alignment
    // moves the origin by none, half or all of the scaled size (300 x 150)
    // on each axis, and the box by the box scale.
    let want: [[u32; 4]; N_ALIGN] = [
        [0xc2480000, 0xc1c80000, 0xc2480000, 0xc1c80000], /* -50 -25 -50 -25 */
        [0x42c80000, 0xc1c80000, 0x42c80000, 0xc1c80000], /* 100 -25 100 -25 */
        [0xc3480000, 0xc1c80000, 0xc3480000, 0xc1c80000], /* -200 -25 -200 -25 */
        [0xc2480000, 0x42480000, 0xc2480000, 0x42480000], /* -50 50 -50 50 */
        [0x42c80000, 0x42480000, 0x42c80000, 0x42480000], /* 100 50 100 50 */
        [0xc3480000, 0x42480000, 0xc3480000, 0x42480000], /* -200 50 -200 50 */
        [0xc2480000, 0xc2c80000, 0xc2480000, 0xc2c80000], /* -50 -100 -50 -100 */
        [0x42c80000, 0xc2c80000, 0x42c80000, 0xc2c80000], /* 100 -100 100 -100 */
        [0xc3480000, 0xc2c80000, 0xc3480000, 0xc2c80000], /* -200 -100 -200 -100 */
        [0xc3480000, 0xc2c80000, 0xc3480000, 0xc2c80000], /* -200 -100 -200 -100 */
        [0x42c80000, 0x42480000, 0x42c80000, 0x42480000], /* 100 50 100 50 */
    ];
    for (align, want) in ALIGNMENTS.into_iter().zip(want.iter()) {
        let mut it = base_item();
        it.align = align;
        compose(&mut it, SRC_CX, SRC_CY, false, 1920.0, 1080.0);
        check_placed(&it, want, "alignment");
        assert_eq!(it.cx, 300, "scaled cx");
        assert_eq!(it.cy, 150, "scaled cy");
        check_bits(it.output_scale.x, 0x3fc0_0000, "output scale");
        check_bits(it.box_scale.x, 0x4396_0000, "box scale x 300");
        check_bits(it.box_scale.y, 0x4316_0000, "box scale y 150");
    }
}

#[test]
fn steps_compose_flipped() {
    // Flipped items without bounds: (float)cx * scale.x is negative, and
    // the cast to uint32_t is x86-64's (the width wraps, and add_alignment()
    // gets the negative width back as an int). arm64 gives 0 there (R7-3),
    // which obs-scene.c keeps; these pins are x86-64's on every platform.
    let want: [[[u32; 4]; N_ALIGN]; 4] = [
        [
            [0xc2480000, 0xc1c80000, 0xc2480000, 0xc1c80000], /* -50 -25 -50 -25 */
            [0x42c80000, 0xc1c80000, 0x42c80000, 0xc1c80000], /* 100 -25 100 -25 */
            [0xc3480000, 0xc1c80000, 0xc3480000, 0xc1c80000], /* -200 -25 -200 -25 */
            [0xc2480000, 0x42480000, 0xc2480000, 0x42480000], /* -50 50 -50 50 */
            [0x42c80000, 0x42480000, 0x42c80000, 0x42480000], /* 100 50 100 50 */
            [0xc3480000, 0x42480000, 0xc3480000, 0x42480000], /* -200 50 -200 50 */
            [0xc2480000, 0xc2c80000, 0xc2480000, 0xc2c80000], /* -50 -100 -50 -100 */
            [0x42c80000, 0xc2c80000, 0x42c80000, 0xc2c80000], /* 100 -100 100 -100 */
            [0xc3480000, 0xc2c80000, 0xc3480000, 0xc2c80000], /* -200 -100 -200 -100 */
            [0xc3480000, 0xc2c80000, 0xc3480000, 0xc2c80000], /* -200 -100 -200 -100 */
            [0x42c80000, 0x42480000, 0x42c80000, 0x42480000], /* 100 50 100 50 */
        ],
        [
            [0x437a0000, 0xc1c80000, 0x437a0000, 0xc1c80000], /* 250 -25 250 -25 */
            [0x42c80000, 0xc1c80000, 0x42c80000, 0xc1c80000], /* 100 -25 100 -25 */
            [0x43c80000, 0xc1c80000, 0x43c80000, 0xc1c80000], /* 400 -25 400 -25 */
            [0x437a0000, 0x42480000, 0x437a0000, 0x42480000], /* 250 50 250 50 */
            [0x42c80000, 0x42480000, 0x42c80000, 0x42480000], /* 100 50 100 50 */
            [0x43c80000, 0x42480000, 0x43c80000, 0x42480000], /* 400 50 400 50 */
            [0x437a0000, 0xc2c80000, 0x437a0000, 0xc2c80000], /* 250 -100 250 -100 */
            [0x42c80000, 0xc2c80000, 0x42c80000, 0xc2c80000], /* 100 -100 100 -100 */
            [0x43c80000, 0xc2c80000, 0x43c80000, 0xc2c80000], /* 400 -100 400 -100 */
            [0x43c80000, 0xc2c80000, 0x43c80000, 0xc2c80000], /* 400 -100 400 -100 */
            [0x42c80000, 0x42480000, 0x42c80000, 0x42480000], /* 100 50 100 50 */
        ],
        [
            [0xc2480000, 0x42fa0000, 0xc2480000, 0x42fa0000], /* -50 125 -50 125 */
            [0x42c80000, 0x42fa0000, 0x42c80000, 0x42fa0000], /* 100 125 100 125 */
            [0xc3480000, 0x42fa0000, 0xc3480000, 0x42fa0000], /* -200 125 -200 125 */
            [0xc2480000, 0x42480000, 0xc2480000, 0x42480000], /* -50 50 -50 50 */
            [0x42c80000, 0x42480000, 0x42c80000, 0x42480000], /* 100 50 100 50 */
            [0xc3480000, 0x42480000, 0xc3480000, 0x42480000], /* -200 50 -200 50 */
            [0xc2480000, 0x43480000, 0xc2480000, 0x43480000], /* -50 200 -50 200 */
            [0x42c80000, 0x43480000, 0x42c80000, 0x43480000], /* 100 200 100 200 */
            [0xc3480000, 0x43480000, 0xc3480000, 0x43480000], /* -200 200 -200 200 */
            [0xc3480000, 0x43480000, 0xc3480000, 0x43480000], /* -200 200 -200 200 */
            [0x42c80000, 0x42480000, 0x42c80000, 0x42480000], /* 100 50 100 50 */
        ],
        [
            [0x437a0000, 0x42fa0000, 0x437a0000, 0x42fa0000], /* 250 125 250 125 */
            [0x42c80000, 0x42fa0000, 0x42c80000, 0x42fa0000], /* 100 125 100 125 */
            [0x43c80000, 0x42fa0000, 0x43c80000, 0x42fa0000], /* 400 125 400 125 */
            [0x437a0000, 0x42480000, 0x437a0000, 0x42480000], /* 250 50 250 50 */
            [0x42c80000, 0x42480000, 0x42c80000, 0x42480000], /* 100 50 100 50 */
            [0x43c80000, 0x42480000, 0x43c80000, 0x42480000], /* 400 50 400 50 */
            [0x437a0000, 0x43480000, 0x437a0000, 0x43480000], /* 250 200 250 200 */
            [0x42c80000, 0x43480000, 0x42c80000, 0x43480000], /* 100 200 100 200 */
            [0x43c80000, 0x43480000, 0x43c80000, 0x43480000], /* 400 200 400 200 */
            [0x43c80000, 0x43480000, 0x43c80000, 0x43480000], /* 400 200 400 200 */
            [0x42c80000, 0x42480000, 0x42c80000, 0x42480000], /* 100 50 100 50 */
        ],
    ];
    for s in 0..4 {
        for (a, align) in ALIGNMENTS.into_iter().enumerate() {
            let mut it = base_item();
            it.align = align;
            it.scale.x *= SIGN_X[s];
            it.scale.y *= SIGN_Y[s];
            compose(&mut it, SRC_CX, SRC_CY, false, 1920.0, 1080.0);
            check_placed(&it, &want[s][a], "flipped");
            let cx = if SIGN_X[s] < 0.0 {
                300_u32.wrapping_neg()
            } else {
                300
            };
            let cy = if SIGN_Y[s] < 0.0 {
                150_u32.wrapping_neg()
            } else {
                150
            };
            assert_eq!(it.cx, cx, "x86 cast cx");
            assert_eq!(it.cy, cy, "x86 cast cy");
        }
    }
}

#[test]
fn steps_compose_bounds() {
    // Each bounds type, a 400 x 400 box, the source centred in it, crop to
    // bounds off and on, upright and flipped on both axes.
    let want: [[[[u32; 8]; 2]; 2]; 7] = [
        [
            [
                [
                    300, 150, 0x3fc00000, 0x3fc00000, 0x43960000, 0x43160000, 0x42c80000,
                    0x42480000,
                ], /* 1.5 1.5 300 150 100 50 */
                [
                    4294966996, 4294967146, 0xbfc00000, 0xbfc00000, 0xc3960000, 0xc3160000,
                    0x42c80000, 0x42480000,
                ], /* -1.5 -1.5 -300 -150 100 50 */
            ],
            [
                [
                    300, 150, 0x3fc00000, 0x3fc00000, 0x43960000, 0x43160000, 0x42c80000,
                    0x42480000,
                ], /* 1.5 1.5 300 150 100 50 */
                [
                    4294966996, 4294967146, 0xbfc00000, 0xbfc00000, 0xc3960000, 0xc3160000,
                    0x42c80000, 0x42480000,
                ], /* -1.5 -1.5 -300 -150 100 50 */
            ],
        ],
        [
            [
                [
                    400, 400, 0x40000000, 0x40800000, 0x43c80000, 0x43c80000, 0x42c80000,
                    0x42480000,
                ], /* 2 4 400 400 100 50 */
                [
                    400, 400, 0xc0000000, 0xc0800000, 0x43c80000, 0x43c80000, 0x43fa0000,
                    0x43e10000,
                ], /* -2 -4 400 400 500 450 */
            ],
            [
                [
                    400, 400, 0x40000000, 0x40800000, 0x43c80000, 0x43c80000, 0x42c80000,
                    0x42480000,
                ], /* 2 4 400 400 100 50 */
                [
                    400, 400, 0xc0000000, 0xc0800000, 0x43c80000, 0x43c80000, 0x43fa0000,
                    0x43e10000,
                ], /* -2 -4 400 400 500 450 */
            ],
        ],
        [
            [
                [
                    400, 400, 0x40000000, 0x40000000, 0x43c80000, 0x43c80000, 0x42c80000,
                    0x43160000,
                ], /* 2 2 400 400 100 150 */
                [
                    400, 400, 0xc0000000, 0xc0000000, 0x43c80000, 0x43c80000, 0x43fa0000,
                    0x43af0000,
                ], /* -2 -2 400 400 500 350 */
            ],
            [
                [
                    400, 400, 0x40000000, 0x40000000, 0x43c80000, 0x43c80000, 0x42c80000,
                    0x43160000,
                ], /* 2 2 400 400 100 150 */
                [
                    400, 400, 0xc0000000, 0xc0000000, 0x43c80000, 0x43c80000, 0x43fa0000,
                    0x43af0000,
                ], /* -2 -2 400 400 500 350 */
            ],
        ],
        [
            [
                [
                    400, 400, 0x40800000, 0x40800000, 0x43c80000, 0x43c80000, 0xc2c80000,
                    0x42480000,
                ], /* 4 4 400 400 -100 50 */
                [
                    400, 400, 0xc0800000, 0xc0800000, 0x43c80000, 0x43c80000, 0x442f0000,
                    0x43e10000,
                ], /* -4 -4 400 400 700 450 */
            ],
            [
                [
                    400, 400, 0x40800000, 0x40800000, 0x43c80000, 0x43c80000, 0x42c80000,
                    0x42480000,
                ], /* 4 4 400 400 100 50 */
                [
                    400, 400, 0xc0800000, 0xc0800000, 0x43c80000, 0x43c80000, 0x43fa0000,
                    0x43e10000,
                ], /* -4 -4 400 400 500 450 */
            ],
        ],
        [
            [
                [
                    400, 400, 0x40000000, 0x40000000, 0x43c80000, 0x43c80000, 0x42c80000,
                    0x43160000,
                ], /* 2 2 400 400 100 150 */
                [
                    400, 400, 0xc0000000, 0xc0000000, 0x43c80000, 0x43c80000, 0x43fa0000,
                    0x43af0000,
                ], /* -2 -2 400 400 500 350 */
            ],
            [
                [
                    400, 400, 0x40000000, 0x40000000, 0x43c80000, 0x43c80000, 0x42c80000,
                    0x43160000,
                ], /* 2 2 400 400 100 150 */
                [
                    400, 400, 0xc0000000, 0xc0000000, 0x43c80000, 0x43c80000, 0x43fa0000,
                    0x43af0000,
                ], /* -2 -2 400 400 500 350 */
            ],
        ],
        [
            [
                [
                    400, 400, 0x40800000, 0x40800000, 0x43c80000, 0x43c80000, 0xc2c80000,
                    0x42480000,
                ], /* 4 4 400 400 -100 50 */
                [
                    400, 400, 0xc0800000, 0xc0800000, 0x43c80000, 0x43c80000, 0x442f0000,
                    0x43e10000,
                ], /* -4 -4 400 400 700 450 */
            ],
            [
                [
                    400, 400, 0x40800000, 0x40800000, 0x43c80000, 0x43c80000, 0x42c80000,
                    0x42480000,
                ], /* 4 4 400 400 100 50 */
                [
                    400, 400, 0xc0800000, 0xc0800000, 0x43c80000, 0x43c80000, 0x43fa0000,
                    0x43e10000,
                ], /* -4 -4 400 400 500 450 */
            ],
        ],
        [
            [
                [
                    400, 400, 0x3fc00000, 0x3fc00000, 0x43c80000, 0x43c80000, 0x43160000,
                    0x432f0000,
                ], /* 1.5 1.5 400 400 150 175 */
                [
                    400, 400, 0xbfc00000, 0xbfc00000, 0x43c80000, 0x43c80000, 0x43e10000,
                    0x43a28000,
                ], /* -1.5 -1.5 400 400 450 325 */
            ],
            [
                [
                    400, 400, 0x3fc00000, 0x3fc00000, 0x43c80000, 0x43c80000, 0x43160000,
                    0x432f0000,
                ], /* 1.5 1.5 400 400 150 175 */
                [
                    400, 400, 0xbfc00000, 0xbfc00000, 0x43c80000, 0x43c80000, 0x43e10000,
                    0x43a28000,
                ], /* -1.5 -1.5 400 400 450 325 */
            ],
        ],
    ];
    let want_crop: [[[[i32; 4]; 2]; 2]; 7] = [
        [[[0, 0, 0, 0], [0, 0, 0, 0]], [[0, 0, 0, 0], [0, 0, 0, 0]]],
        [[[0, 0, 0, 0], [0, 0, 0, 0]], [[0, 0, 0, 0], [0, 0, 0, 0]]],
        [[[0, 0, 0, 0], [0, 0, 0, 0]], [[0, 0, 0, 0], [0, 0, 0, 0]]],
        [
            [[0, 0, 0, 0], [0, 0, 0, 0]],
            [[50, 0, 50, 0], [50, 0, 50, 0]],
        ],
        [[[0, 0, 0, 0], [0, 0, 0, 0]], [[0, 0, 0, 0], [0, 0, 0, 0]]],
        [
            [[0, 0, 0, 0], [0, 0, 0, 0]],
            [[50, 0, 50, 0], [50, 0, 50, 0]],
        ],
        [[[0, 0, 0, 0], [0, 0, 0, 0]], [[0, 0, 0, 0], [0, 0, 0, 0]]],
    ];
    for t in 0..=BOUNDS_MAX_ONLY {
        for c in 0..2 {
            for f in 0..2 {
                let mut it = base_item();
                it.bounds_type = t;
                it.crop_to_bounds = c == 1;
                it.scale = if f == 1 {
                    Vec2::new(-1.5, -1.5)
                } else {
                    Vec2::new(1.5, 1.5)
                };
                it.bounds = Vec2::new(400.0, 400.0);
                compose(&mut it, SRC_CX, SRC_CY, false, 1920.0, 1080.0);
                let ti = usize::try_from(t).unwrap();
                let w = &want[ti][c][f];
                assert_eq!(it.cx, w[0], "bounds cx");
                assert_eq!(it.cy, w[1], "bounds cy");
                check_bits(it.output_scale.x, w[2], "output scale x");
                check_bits(it.output_scale.y, w[3], "output scale y");
                check_bits(it.box_scale.x, w[4], "box scale x");
                check_bits(it.box_scale.y, w[5], "box scale y");
                check_bits(it.draw.t.x, w[6], "draw tx");
                check_bits(it.draw.t.y, w[7], "draw ty");
                let wc = want_crop[ti][c][f];
                check_crop(it.bounds_crop, wc[0], wc[1], wc[2], wc[3], "bounds crop");
            }
        }
    }
}

#[test]
fn steps_compose_relative() {
    // Relative coordinates: the position, the scale and the bounds are read
    // against the canvas (1920 x 1080 here, 1280 x 720 for the second), and
    // a group's items are not nudged.
    let want: [[u32; 8]; 6] = [
        [
            0x3f400000, 0x3f400000, 0x43160000, 0x42960000, 0x447f0000, 0x44414000, 0x447f0000,
            0x44414000,
        ], /* 0.75 0.75 150 75 1020 773 1020 773 */
        [
            0x3faccccc, 0x3faccccc, 0x43870000, 0x43070000, 0x44700000, 0x4439c000, 0x44700000,
            0x4439c000,
        ], /* 1.35 1.35 270 135 960 743 960 743 */
        [
            0x3f400000, 0x3f400000, 0x43160000, 0x42960000, 0x447f0000, 0x44414000, 0x447f0000,
            0x44414000,
        ], /* 0.75 0.75 150 75 1020 773 1020 773 */
        [
            0x3f000000, 0x3f000000, 0x42c80000, 0x42480000, 0x442a0000, 0x4400c000, 0x442a0000,
            0x4400c000,
        ], /* 0.5 0.5 100 50 680 515 680 515 */
        [
            0x3f666666, 0x3f666666, 0x43340000, 0x42b40000, 0x44200000, 0x43f78000, 0x44200000,
            0x43f78000,
        ], /* 0.9 0.9 180 90 640 495 640 495 */
        [
            0x3f400000, 0x3f400000, 0x43160000, 0x42960000, 0x4423c000, 0x43fb8000, 0x4423c000,
            0x43fb8000,
        ], /* 0.75 0.75 150 75 655 503 655 503 */
    ];
    let mut i = 0;
    for canvas in 0..2 {
        let (cw, ch) = if canvas == 1 {
            (1280.0, 720.0)
        } else {
            (1920.0, 1080.0)
        };
        for k in 0..3 {
            let mut it = base_item();
            it.absolute = false;
            it.pos = Vec2::new(0.25, 0.5);
            it.scale = Vec2::new(0.75, 0.75);
            it.bounds = Vec2::new(0.5, 0.25);
            it.align = ALIGN_CENTER;
            if k == 1 {
                it.bounds_type = BOUNDS_SCALE_INNER;
            }
            if k == 2 {
                it.parent_is_group = true;
                it.is_scene = true;
            }
            compose(&mut it, SRC_CX, SRC_CY, false, cw, ch);
            let w = &want[i];
            check_bits(it.output_scale.x, w[0], "relative output scale x");
            check_bits(it.output_scale.y, w[1], "relative output scale y");
            check_bits(it.box_scale.x, w[2], "relative box scale x");
            check_bits(it.box_scale.y, w[3], "relative box scale y");
            check_bits(it.draw.t.x, w[4], "relative draw tx");
            check_bits(it.draw.t.y, w[5], "relative draw ty");
            check_bits(it.boxed.t.x, w[6], "relative box tx");
            check_bits(it.boxed.t.y, w[7], "relative box ty");
            i += 1;
        }
    }
}

#[test]
fn steps_compose_nested_crop() {
    // a nested scene seen for the first time (last_width 0): its crop is
    // kept and the last size set
    let mut it = base_item();
    it.is_scene = true;
    it.crop = crop_of(10, 20, 30, 40);
    compose(&mut it, 1920, 1080, true, 1920.0, 1080.0);
    check_crop(it.crop, 10, 20, 30, 40, "first size");
    assert_eq!(it.last_width, 1920, "first last width");
    assert_eq!(it.last_height, 1080, "first last height");
    assert_eq!(it.cx, 2820, "first cx: (1920 - 40) * 1.5");
    assert_eq!(it.cy, 1530, "first cy: (1080 - 60) * 1.5");

    // the scene shrinks to half: so does the crop
    compose(&mut it, 960, 540, true, 1920.0, 1080.0);
    check_crop(it.crop, 5, 10, 15, 20, "half");
    assert_eq!(it.cx, 1410, "half cx: (960 - 20) * 1.5");
    assert_eq!(it.cy, 765, "half cy: (540 - 30) * 1.5");
    check_bits(it.box_scale.x, 0x44b0_4000, "half box scale x 1410");
    check_bits(it.box_scale.y, 0x443f_4000, "half box scale y 765");

    // without update_tex (a setter's update): the crop is kept through a
    // size change, but the last size still follows
    compose(&mut it, 1920, 1080, false, 1920.0, 1080.0);
    check_crop(it.crop, 5, 10, 15, 20, "no update_tex");
    assert_eq!(it.last_width, 1920, "no update_tex last width");

    // not a scene: kept
    let mut it = base_item();
    it.crop = crop_of(10, 20, 30, 40);
    it.last_width = 960;
    it.last_height = 540;
    compose(&mut it, 1920, 1080, true, 1920.0, 1080.0);
    check_crop(it.crop, 10, 20, 30, 40, "not a scene");

    // a nested scene with bounds: the bounds crop of the last composition
    // is reset before the cropped size is taken
    let mut it = base_item();
    it.is_scene = true;
    it.bounds_type = BOUNDS_SCALE_OUTER;
    it.crop_to_bounds = true;
    it.bounds = Vec2::new(100.0, 100.0);
    compose(&mut it, 200, 100, true, 1920.0, 1080.0);
    check_crop(it.bounds_crop, 50, 0, 50, 0, "outer crop");
    assert_eq!(it.cx, 100, "outer cx");
    compose(&mut it, 200, 100, true, 1920.0, 1080.0);
    check_crop(
        it.bounds_crop,
        50,
        0,
        50,
        0,
        "outer crop again, not doubled",
    );
}

/// Every alignment x bounds type x crop x absolute/relative x scale sign
/// at one rotation, folded into one hash, in the C test's order.
fn sweep(rot: f32) -> u64 {
    let mut h = FNV_BASIS;
    for a in 0..N_ALIGN {
        for t in 0..=BOUNDS_MAX_ONLY {
            for crop in 0..2 {
                for rel in 0..2 {
                    for s in 0..4 {
                        let mut it = base_item();
                        it.align = ALIGNMENTS[a];
                        it.bounds_type = t;
                        it.bounds_align = ALIGNMENTS[(a + 3) % N_ALIGN];
                        it.crop_to_bounds = crop == 1;
                        it.rot = rot;
                        if crop == 1 {
                            it.crop = crop_of(10, 5, 30, 15);
                        }
                        if rel == 1 {
                            it.absolute = false;
                            it.pos = Vec2::new(0.25, 0.5);
                            it.scale = Vec2::new(0.75, 0.75);
                            it.bounds = Vec2::new(0.5, 0.25);
                        }
                        it.scale.x *= SIGN_X[s];
                        it.scale.y *= SIGN_Y[s];
                        compose(&mut it, SRC_CX, SRC_CY, false, 1920.0, 1080.0);
                        h = fnv_item(h, &it);
                    }
                }
            }
        }
    }
    h
}

#[test]
fn steps_compose_sweep() {
    assert_eq!(sweep(0.0), 0x2a4d_d455_6ee7_0059, "sweep at 0");
    assert_eq!(sweep(-0.0), 0x2a4d_d455_6ee7_0059, "sweep at -0");
    #[cfg(all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"))]
    {
        assert_eq!(sweep(90.0), 0xfab5_343e_4ab1_7d6d, "sweep at 90");
        assert_eq!(sweep(45.0), 0xbc16_7d4b_772b_f747, "sweep at 45");
        assert_eq!(sweep(-30.0), 0x7134_3631_9643_9e8d, "sweep at -30");
        assert_eq!(sweep(720.0), 0x7cb0_d4f8_e2c7_cb3d, "sweep at 720");
    }
}

// ------------------------------------------------------------------ edges

/// `update_nested_scene_crop` and `calc_cx`/`calc_cy` as the C wrote them,
/// in `f64`-free `f32` and 64-bit sums: the model `prepare` must equal.
#[allow(clippy::cast_precision_loss)]
fn prepare_model(crop: Crop, lw: u32, lh: u32, w: u32, h: u32, nested: bool) -> Prepared {
    let mut c = crop;
    if nested && lh != 0 && lw != 0 {
        let fx = w as f32 / lw as f32;
        let fy = h as f32 / lh as f32;
        let scale = |v: i32, f: f32| {
            let s = v as f32 * f;
            if s.is_nan() {
                0
            } else {
                // Rust's `as` saturates, as obs_scene_scale_crop clamps
                #[allow(clippy::cast_possible_truncation)]
                let r = s as i32;
                r
            }
        };
        c = crop_of(
            scale(c.left, fx),
            scale(c.top, fy),
            scale(c.right, fx),
            scale(c.bottom, fy),
        );
    }
    let sized = |size: u32, a: i32, b: i32| {
        let sum = i64::from(a) + i64::from(b);
        if sum < 0 || sum > i64::from(size) {
            2
        } else {
            u32::try_from(i64::from(size) - sum).unwrap()
        }
    };
    Prepared {
        crop: c,
        bounds_crop: NO_CROP,
        last_width: w,
        last_height: h,
        cx: sized(w, c.left, c.right),
        cy: sized(h, c.top, c.bottom),
    }
}

#[test]
fn edge_prepare_guard_each_zero() {
    // the guard needs both last sizes; nested or not decides the rest
    for (lw, lh) in [(0, 0), (0, 5), (5, 0), (5, 5)] {
        for nested in [false, true] {
            let p = item_transform::prepare(crop_of(4, 4, 4, 4), lw, lh, 10, 10, nested);
            let scaled = nested && lw != 0 && lh != 0;
            let want = if scaled { 8 } else { 4 };
            check_crop(p.crop, want, want, want, want, "guard");
            assert_eq!((p.last_width, p.last_height), (10, 10));
        }
    }
}

#[test]
fn edge_prepare_bounds_crop_always_zero() {
    for nested in [false, true] {
        let p = item_transform::prepare(crop_of(1, 2, 3, 4), 7, 9, 0, u32::MAX, nested);
        assert_eq!(p.bounds_crop, NO_CROP);
    }
}

#[test]
fn edge_box_scale_signed_zero_and_nan() {
    let s = item_transform::box_scale(Vec2::new(-0.0, f32::NAN), 5, 0);
    check_bits(s.x, 0x8000_0000, "-0 * 5");
    assert!(s.y.is_nan());
    let s = item_transform::box_scale(Vec2::new(f32::MAX, -f32::MAX), 2, 2);
    check_bits(s.x, 0x7f80_0000, "overflow to +inf");
    check_bits(s.y, 0xff80_0000, "overflow to -inf");
}

#[test]
fn edge_matrix_rot0_lanes() {
    // with no rotation the matrix is the scale on the diagonal and the
    // position minus the origin in t, for any finite values
    for &(sx, sy, ox, oy, px, py) in &[
        (1.0_f32, 1.0_f32, 0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32),
        (2.5, -0.5, 3.0, -7.0, 11.0, 13.0),
        (1e-30, 1e30, 1e10, -1e10, 0.1, 0.2),
    ] {
        let m = matrix_of(sx, sy, ox, oy, 0.0, px, py);
        assert_eq!(m.x.x.to_bits(), sx.to_bits());
        assert_eq!(m.y.y.to_bits(), sy.to_bits());
        assert_eq!(m.t.x.to_bits(), (-ox + px).to_bits());
        assert_eq!(m.t.y.to_bits(), (-oy + py).to_bits());
        assert_eq!(m.z.z, 1.0);
        assert_eq!(m.t.w, 1.0);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn edge_prepare_matches_model(
        crop in prop::array::uniform4(prop_oneof![
            Just(0_i32), 0_i32..2000, any::<i32>(), Just(i32::MAX), Just(i32::MIN)
        ]),
        lw in prop_oneof![Just(0_u32), 1_u32..5000, any::<u32>()],
        lh in prop_oneof![Just(0_u32), 1_u32..5000, any::<u32>()],
        w in prop_oneof![Just(0_u32), 1_u32..5000, any::<u32>()],
        h in prop_oneof![Just(0_u32), 1_u32..5000, any::<u32>()],
        nested in any::<bool>(),
    ) {
        let c = crop_of(crop[0], crop[1], crop[2], crop[3]);
        prop_assert_eq!(
            item_transform::prepare(c, lw, lh, w, h, nested),
            prepare_model(c, lw, lh, w, h, nested)
        );
    }

    #[test]
    fn edge_matrix_is_the_reference_chain(
        sx in -4000.0_f32..4000.0, sy in -4000.0_f32..4000.0,
        ox in -4000.0_f32..4000.0, oy in -4000.0_f32..4000.0,
        rot in -1000.0_f32..1000.0,
        px in -4000.0_f32..4000.0, py in -4000.0_f32..4000.0,
    ) {
        prop_assert_eq!(
            bits16(&matrix_of(sx, sy, ox, oy, rot, px, py)),
            bits16(&reference_matrix(sx, sy, ox, oy, rot, px, py))
        );
    }

    #[test]
    fn edge_box_scale_is_one_multiply(
        sx in any::<u32>(), sy in any::<u32>(), w in any::<u32>(), h in any::<u32>()
    ) {
        let (sx, sy) = (f32::from_bits(sx), f32::from_bits(sy));
        let s = item_transform::box_scale(Vec2::new(sx, sy), w, h);
        #[allow(clippy::cast_precision_loss)]
        let (wx, wy) = (sx * w as f32, sy * h as f32);
        prop_assert!(s.x.to_bits() == wx.to_bits() || (s.x.is_nan() && wx.is_nan()));
        prop_assert!(s.y.to_bits() == wy.to_bits() || (s.y.is_nan() && wy.is_nan()));
    }

    #[test]
    fn edge_bounded_is_not_zero(t in any::<i32>()) {
        prop_assert_eq!(item_transform::bounded(t), t != 0);
    }
}
