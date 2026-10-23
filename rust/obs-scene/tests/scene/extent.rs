//! Tier 1: `obs_scene::extent`, the content extent of a scene or group.
//!
//! The cases without a prefix port `test/cmocka/test_group_extent.c` 1:1,
//! under the same names, in the same order, with the same matrices and
//! expectations. As there, [`measure`] is what `resize_scene_base` computes
//! for a list of box transforms: start the fold, add each item in order,
//! size it.
//!
//! The `edge_*` cases are new: the size step on its own (the values of
//! `test_scene_crop_steps`), a NaN in the y column only, a non-finite z
//! row, which reaches the corners through `0 * z`, and the ties of -0 and
//! +0 on the min side and on y.
//!
//! Cases marked characterized, not endorsed, are what the C does today,
//! including the odd parts.

use obs_graphics::matrix4::Matrix4;
use obs_graphics::vec2::Vec2;
use obs_graphics::vec4::Vec4;
use obs_scene::extent::{Extent, Size, add, init, size, size_from_extent};

/// `M_INFINITE` (`graphics/math-defs.h`), where the fold starts.
const M_INFINITE: f32 = 3.4e38;

/// Bit for bit: -0 is not +0.
fn check_float(got: f32, want: f32, what: &str) {
    assert!(
        got.to_bits() == want.to_bits(),
        "{what}: {got} ({:#010x}), expected {want} ({:#010x})",
        got.to_bits(),
        want.to_bits()
    );
}

fn check_vec2(got: Vec2, x: f32, y: f32, what: &str) {
    assert!(
        got.x.to_bits() == x.to_bits() && got.y.to_bits() == y.to_bits(),
        "{what}: ({}, {}), expected ({x}, {y})",
        got.x,
        got.y
    );
}

/// A box transform: (1, 0) lands on (xx, xy) and (0, 1) on (yx, yy) before
/// the translation (tx, ty).
fn box_transform(xx: f32, xy: f32, yx: f32, yy: f32, tx: f32, ty: f32) -> Matrix4 {
    Matrix4::new(
        Vec4::new(xx, xy, 0.0, 0.0),
        Vec4::new(yx, yy, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(tx, ty, 0.0, 1.0),
    )
}

/// What `resize_scene_base` computes for these items, in this order.
fn measure(items: &[Matrix4]) -> (Extent, Size) {
    let mut extent = init();
    for item in items {
        add(&mut extent, item);
    }
    (extent, size(&extent))
}

#[test]
fn one_item() {
    let (e, s) = measure(&[box_transform(100.0, 0.0, 0.0, 50.0, 10.0, 20.0)]);

    check_vec2(e.min, 10.0, 20.0, "min");
    check_vec2(e.max, 110.0, 70.0, "max");
    check_vec2(s.scale, 100.0, 50.0, "scale");
    assert_eq!(s.cx, 100);
    assert_eq!(s.cy, 50);
}

/// The extent covers every item, whichever item each edge comes from.
#[test]
fn several_items() {
    let (e, s) = measure(&[
        box_transform(100.0, 0.0, 0.0, 50.0, 10.0, 20.0),
        box_transform(30.0, 0.0, 0.0, 30.0, -5.0, 60.0),
        box_transform(10.0, 0.0, 0.0, 10.0, 200.0, -40.0),
    ]);

    check_vec2(e.min, -5.0, -40.0, "min");
    check_vec2(e.max, 210.0, 90.0, "max");
    check_vec2(s.scale, 215.0, 130.0, "scale");
    assert_eq!(s.cx, 215);
    assert_eq!(s.cy, 130);
}

/// A box turned 45 degrees: the extent is that of the corners, not the
/// box's own translation and size.
#[test]
fn rotated_item_uses_the_corners() {
    let (e, s) = measure(&[box_transform(10.0, 10.0, -10.0, 10.0, 50.0, 50.0)]);

    check_vec2(e.min, 40.0, 50.0, "min");
    check_vec2(e.max, 60.0, 70.0, "max");
    check_vec2(s.scale, 20.0, 20.0, "scale");
    assert_eq!(s.cx, 20);
    assert_eq!(s.cy, 20);
}

/// The size is the extent rounded up; a whole one stays.
#[test]
fn fractional_extent_rounds_up() {
    let (e, s) = measure(&[box_transform(99.5, 0.0, 0.0, 0.25, 0.5, -0.75)]);

    check_vec2(e.min, 0.5, -0.75, "min");
    check_vec2(e.max, 100.0, -0.5, "max");
    check_vec2(s.scale, 99.5, 0.25, "scale");
    assert_eq!(s.cx, 100);
    assert_eq!(s.cy, 1);

    let (_, s) = measure(&[box_transform(1280.0, 0.0, 0.0, 720.0, 0.25, 0.25)]);

    check_vec2(s.scale, 1280.0, 720.0, "whole scale");
    assert_eq!(s.cx, 1280);
    assert_eq!(s.cy, 720);
}

/// The fold starts at (`M_INFINITE`, `M_INFINITE`) and (-`M_INFINITE`,
/// -`M_INFINITE`). `M_INFINITE` is 3.4e38, not infinity. Sized with no
/// item, the fold overflows to -inf wide, so 0 x 0.
#[test]
fn empty_scene() {
    let (e, s) = measure(&[]);

    check_vec2(e.min, M_INFINITE, M_INFINITE, "min");
    check_vec2(e.max, -M_INFINITE, -M_INFINITE, "max");
    check_vec2(s.scale, f32::NEG_INFINITY, f32::NEG_INFINITY, "scale");
    assert_eq!(s.cx, 0);
    assert_eq!(s.cy, 0);
}

/// A NaN anywhere in the x column makes every corner's x NaN, and the
/// compares skip it. Characterized, not endorsed.
#[test]
fn nan_is_skipped() {
    let nan_item = box_transform(100.0, 0.0, 0.0, 50.0, f32::NAN, 20.0);
    let (e, s) = measure(&[nan_item]);

    check_vec2(e.min, M_INFINITE, 20.0, "NaN alone min");
    check_vec2(e.max, -M_INFINITE, 70.0, "NaN alone max");
    check_vec2(s.scale, f32::NEG_INFINITY, 50.0, "NaN alone scale");
    assert_eq!(s.cx, 0);
    assert_eq!(s.cy, 50);

    let finite = box_transform(30.0, 0.0, 0.0, 10.0, 5.0, 0.0);
    let (e, s) = measure(&[nan_item, finite]);

    check_vec2(e.min, 5.0, 0.0, "NaN first min");
    check_vec2(e.max, 35.0, 70.0, "NaN first max");
    assert_eq!(s.cx, 30);
    assert_eq!(s.cy, 70);

    let (e, s) = measure(&[finite, box_transform(f32::NAN, 0.0, 0.0, 50.0, 0.0, 20.0)]);

    check_vec2(e.min, 5.0, 0.0, "NaN second min");
    check_vec2(e.max, 35.0, 70.0, "NaN second max");
    assert_eq!(s.cx, 30);
    assert_eq!(s.cy, 70);
}

/// Infinities. Characterized, not endorsed: the start values are finite,
/// so an item at +inf or -inf alone is `u32::MAX` wide (or high), next to a
/// finite item the extent reaches +inf too, and an infinitely wide item at
/// 0 (corners NaN and +inf) is `u32::MAX` wide as well.
#[test]
fn infinity() {
    let at_inf = box_transform(100.0, 0.0, 0.0, 50.0, f32::INFINITY, 20.0);
    let (e, s) = measure(&[at_inf]);

    check_vec2(e.min, M_INFINITE, 20.0, "+inf min");
    check_vec2(e.max, f32::INFINITY, 70.0, "+inf max");
    check_float(s.scale.x, f32::INFINITY, "+inf scale x");
    assert_eq!(s.cx, u32::MAX);
    assert_eq!(s.cy, 50);

    let (e, s) = measure(&[at_inf, box_transform(30.0, 0.0, 0.0, 10.0, 5.0, 0.0)]);

    check_vec2(e.min, 5.0, 0.0, "+inf and finite min");
    check_vec2(e.max, f32::INFINITY, 70.0, "+inf and finite max");
    check_float(s.scale.x, f32::INFINITY, "+inf and finite scale x");
    assert_eq!(s.cx, u32::MAX);
    assert_eq!(s.cy, 70);

    let (e, s) = measure(&[box_transform(
        100.0,
        0.0,
        0.0,
        50.0,
        10.0,
        f32::NEG_INFINITY,
    )]);

    check_vec2(e.min, 10.0, f32::NEG_INFINITY, "-inf min");
    check_vec2(e.max, 110.0, -M_INFINITE, "-inf max");
    check_float(s.scale.y, f32::INFINITY, "-inf scale y");
    assert_eq!(s.cx, 100);
    assert_eq!(s.cy, u32::MAX);

    let (e, s) = measure(&[box_transform(f32::INFINITY, 0.0, 0.0, 50.0, 0.0, 0.0)]);

    check_vec2(e.min, M_INFINITE, 0.0, "infinite width min");
    check_vec2(e.max, f32::INFINITY, 50.0, "infinite width max");
    check_float(s.scale.x, f32::INFINITY, "infinite width scale x");
    assert_eq!(s.cx, u32::MAX);
    assert_eq!(s.cy, 50);
}

/// The start values are finite, so an item wholly past them leaves them in
/// place. Characterized, not endorsed.
#[test]
fn past_the_start_values() {
    let (e, s) = measure(&[box_transform(0.0, 0.0, 0.0, 0.0, f32::MAX, -f32::MAX)]);

    check_vec2(e.min, M_INFINITE, -f32::MAX, "min");
    check_vec2(e.max, f32::MAX, -M_INFINITE, "max");
    assert_eq!(s.cx, u32::MAX);
    assert_eq!(s.cy, u32::MAX);
}

/// Sizes past the `uint32_t` range saturate (#1840).
#[test]
fn out_of_range_sizes() {
    let (_, s) = measure(&[box_transform(5e9, 0.0, 0.0, 4_294_967_040.0, 0.0, 0.0)]);

    check_vec2(s.scale, 5e9, 4_294_967_040.0, "scale");
    assert_eq!(s.cx, u32::MAX);
    assert_eq!(s.cy, 4_294_967_040);

    let (_, s) = measure(&[box_transform(4_294_967_296.0, 0.0, 0.0, 1e30, 0.0, 0.0)]);

    assert_eq!(s.cx, u32::MAX);
    assert_eq!(s.cy, u32::MAX);

    let (_, s) = measure(&[
        box_transform(1.0, 0.0, 0.0, 1.0, -3e38, -3e38),
        box_transform(1.0, 0.0, 0.0, 1.0, 3e38, 3e38),
    ]);

    check_vec2(s.scale, f32::INFINITY, f32::INFINITY, "overflowed scale");
    assert_eq!(s.cx, u32::MAX);
    assert_eq!(s.cy, u32::MAX);
}

/// A corner equal to the current max does not replace it, so with -0 and
/// +0 the first one stays. Characterized, not endorsed.
#[test]
fn tie_keeps_the_first() {
    let neg_zero = Matrix4::new(
        Vec4::new(-1.0, 0.0, 0.0, 0.0),
        Vec4::new(-1.0, 0.0, 0.0, 0.0),
        Vec4::new(-1.0, 0.0, 1.0, 0.0),
        Vec4::new(-0.0, 0.0, 0.0, 1.0),
    );
    let pos_zero = box_transform(-1.0, 0.0, -1.0, 0.0, 0.0, 0.0);

    let (e, _) = measure(&[neg_zero, pos_zero]);
    check_float(e.max.x, -0.0, "-0 first max x");
    check_float(e.min.x, -2.0, "-0 first min x");

    let (e, _) = measure(&[pos_zero, neg_zero]);
    check_float(e.max.x, 0.0, "+0 first max x");
    check_float(e.min.x, -2.0, "+0 first min x");
}

/// `obs_scene_size_from_extent` on its own: the values
/// `test_scene_crop_steps` pins.
#[test]
fn edge_size_from_extent() {
    assert_eq!(size_from_extent(0.0), 0);
    assert_eq!(size_from_extent(1280.0), 1280);
    assert_eq!(size_from_extent(199.2), 200);
    assert_eq!(size_from_extent(-0.5), 0);
    assert_eq!(size_from_extent(4_294_967_040.0), 4_294_967_040);
    assert_eq!(size_from_extent(4_294_967_296.0), u32::MAX);
    assert_eq!(size_from_extent(1e20), u32::MAX);
    assert_eq!(size_from_extent(f32::INFINITY), u32::MAX);
    assert_eq!(size_from_extent(-1e20), 0);
    assert_eq!(size_from_extent(f32::NEG_INFINITY), 0);
    assert_eq!(size_from_extent(f32::NAN), 0);
    assert_eq!(size_from_extent(-0.0), 0);
    assert_eq!(size_from_extent(f32::MIN_POSITIVE), 1);
}

/// A NaN in the y column only: x is measured, y is skipped.
#[test]
fn edge_nan_in_y_column() {
    let (e, s) = measure(&[box_transform(100.0, 0.0, 0.0, 50.0, 10.0, f32::NAN)]);

    check_vec2(e.min, 10.0, M_INFINITE, "min");
    check_vec2(e.max, 110.0, -M_INFINITE, "max");
    assert_eq!(s.cx, 100);
    assert_eq!(s.cy, 0);
}

/// The corners have z = 0, but the z row still takes part: an infinite z
/// row x lane makes every corner's x NaN (0 * inf), so x is skipped.
#[test]
fn edge_infinite_z_row() {
    let m = Matrix4::new(
        Vec4::new(100.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 50.0, 0.0, 0.0),
        Vec4::new(f32::INFINITY, 0.0, 1.0, 0.0),
        Vec4::new(10.0, 20.0, 0.0, 1.0),
    );
    let (e, s) = measure(&[m]);

    check_vec2(e.min, M_INFINITE, 20.0, "min");
    check_vec2(e.max, -M_INFINITE, 70.0, "max");
    assert_eq!(s.cx, 0);
    assert_eq!(s.cy, 50);
}

/// The tie on the min side: +0 then -0 leaves +0, and the other way -0.
#[test]
fn edge_min_tie_keeps_the_first() {
    // (0, 0) lands on x = +0 and every other corner on 1 or 2.
    let pos_zero = box_transform(1.0, 0.0, 1.0, 0.0, 0.0, 0.0);
    // Every product of every corner's x is -0, so every x is -0.
    let neg_zero = Matrix4::new(
        Vec4::new(-0.0, 0.0, 0.0, 0.0),
        Vec4::new(-0.0, 0.0, 0.0, 0.0),
        Vec4::new(-1.0, 0.0, 1.0, 0.0),
        Vec4::new(-0.0, 0.0, 0.0, 1.0),
    );

    let (e, _) = measure(&[pos_zero, neg_zero]);
    check_float(e.min.x, 0.0, "+0 first min x");
    check_float(e.max.x, 2.0, "+0 first max x");

    let (e, _) = measure(&[neg_zero, pos_zero]);
    check_float(e.min.x, -0.0, "-0 first min x");
    check_float(e.max.x, 2.0, "-0 first max x");
}

/// The ties on y, both sides: the first of -0 and +0 stays.
#[test]
fn edge_y_ties_keep_the_first() {
    // (0, 0) lands on y = +0 and every other corner on 1 or 2.
    let pos_min = box_transform(0.0, 1.0, 0.0, 1.0, 0.0, 0.0);
    // Every product of every corner's y is -0, so every y is -0.
    let neg_all = Matrix4::new(
        Vec4::new(0.0, -0.0, 0.0, 0.0),
        Vec4::new(0.0, -0.0, 0.0, 0.0),
        Vec4::new(0.0, -1.0, 1.0, 0.0),
        Vec4::new(0.0, -0.0, 0.0, 1.0),
    );
    let (e, _) = measure(&[pos_min, neg_all]);
    check_float(e.min.y, 0.0, "+0 first min y");
    let (e, _) = measure(&[neg_all, pos_min]);
    check_float(e.min.y, -0.0, "-0 first min y");

    // (0, 0) lands on y = -0 and every other corner on -1 or -2.
    let neg_max = Matrix4::new(
        Vec4::new(0.0, -1.0, 0.0, 0.0),
        Vec4::new(0.0, -1.0, 0.0, 0.0),
        Vec4::new(0.0, -1.0, 1.0, 0.0),
        Vec4::new(0.0, -0.0, 0.0, 1.0),
    );
    // (0, 0) lands on y = +0 and every other corner on -1 or -2.
    let pos_max = box_transform(0.0, -1.0, 0.0, -1.0, 0.0, 0.0);
    let (e, _) = measure(&[neg_max, pos_max]);
    check_float(e.max.y, -0.0, "-0 first max y");
    let (e, _) = measure(&[pos_max, neg_max]);
    check_float(e.max.y, 0.0, "+0 first max y");
}
