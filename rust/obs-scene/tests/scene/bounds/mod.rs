//! Tier 1 mirror of `test/cmocka/test_scene_bounds_fit.c`.
//!
//! The box is 800 by 600, as `fit` hardcodes it there. The live empty-group
//! case stays in `test_group_empty_bounds`.

use obs_graphics::vec2::Vec2;
use obs_scene::bounds::{BoundsType, bounds_fit_scale};

mod parity;

/// The six types `all_types` walks. `OBS_BOUNDS_NONE` is not in that list.
const TYPES: [BoundsType; 6] = [
    BoundsType::Stretch,
    BoundsType::ScaleInner,
    BoundsType::ScaleOuter,
    BoundsType::ScaleToWidth,
    BoundsType::ScaleToHeight,
    BoundsType::MaxOnly,
];

fn fit(kind: BoundsType, cx: u32, cy: u32, scale_x: f32, scale_y: f32) -> Vec2 {
    bounds_fit_scale(
        kind,
        Vec2::new(800.0, 600.0),
        cx,
        cy,
        Vec2::new(scale_x, scale_y),
    )
}

fn finite_nonzero(v: Vec2) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.x != 0.0 && v.y != 0.0
}

fn expect_scale(
    kind: BoundsType,
    cx: u32,
    cy: u32,
    scale_x: f32,
    scale_y: f32,
    want_x: f32,
    want_y: f32,
) {
    let s = fit(kind, cx, cy, scale_x, scale_y);
    assert!(
        (s.x - want_x).abs() <= 1e-5 && (s.y - want_y).abs() <= 1e-5,
        "{kind:?}: {cx}x{cy} at ({scale_x}, {scale_y}) gives ({}, {}), expected ({want_x}, {want_y})",
        s.x,
        s.y
    );
}

#[test]
fn bounds_type_values_match_obs_h() {
    assert_eq!(BoundsType::from_c(0), BoundsType::None);
    assert_eq!(BoundsType::from_c(1), BoundsType::Stretch);
    assert_eq!(BoundsType::from_c(2), BoundsType::ScaleInner);
    assert_eq!(BoundsType::from_c(3), BoundsType::ScaleOuter);
    assert_eq!(BoundsType::from_c(4), BoundsType::ScaleToWidth);
    assert_eq!(BoundsType::from_c(5), BoundsType::ScaleToHeight);
    assert_eq!(BoundsType::from_c(6), BoundsType::MaxOnly);
    assert_eq!(BoundsType::from_c(7), BoundsType::Other);
    assert_eq!(BoundsType::from_c(-1), BoundsType::Other);
}

#[test]
fn empty_source_keeps_scale() {
    for kind in TYPES {
        let s = fit(kind, 0, 0, 1.0, 1.0);
        assert_eq!((s.x, s.y), (1.0, 1.0), "{kind:?}");
    }
}

#[test]
fn empty_source_keeps_flip() {
    for kind in TYPES {
        let s = fit(kind, 0, 0, -1.0, 2.0);
        assert_eq!((s.x, s.y), (-1.0, 2.0), "{kind:?}");
    }
}

#[test]
fn zero_width_or_height_stays_finite() {
    for (cx, cy) in [(0, 100), (200, 0)] {
        for kind in TYPES {
            let s = fit(kind, cx, cy, 1.0, 1.0);
            assert!(
                finite_nonzero(s),
                "{kind:?}: {cx}x{cy} gives ({}, {})",
                s.x,
                s.y
            );
        }
    }
}

#[test]
fn zero_scale_stays_finite() {
    for kind in TYPES {
        let s = fit(kind, 200, 100, 0.0, 0.0);
        assert!(
            s.x.is_finite() && s.y.is_finite(),
            "{kind:?}: ({}, {})",
            s.x,
            s.y
        );
    }
}

#[test]
fn sized_source_fits() {
    expect_scale(BoundsType::Stretch, 200, 100, 1.0, 1.0, 4.0, 6.0);
    expect_scale(BoundsType::ScaleInner, 200, 100, 1.0, 1.0, 4.0, 4.0);
    expect_scale(BoundsType::ScaleOuter, 200, 100, 1.0, 1.0, 6.0, 6.0);
    expect_scale(BoundsType::ScaleToWidth, 200, 100, 1.0, 1.0, 4.0, 4.0);
    expect_scale(BoundsType::ScaleToHeight, 200, 100, 1.0, 1.0, 6.0, 6.0);
    expect_scale(BoundsType::MaxOnly, 200, 100, 1.0, 1.0, 1.0, 1.0);

    expect_scale(BoundsType::ScaleInner, 200, 100, 2.0, 2.0, 4.0, 4.0);
    expect_scale(BoundsType::Stretch, 200, 100, 2.0, 3.0, 4.0, 6.0);
    expect_scale(BoundsType::MaxOnly, 2000, 100, 1.0, 1.0, 0.4, 0.4);
    expect_scale(BoundsType::ScaleInner, 200, 100, -1.0, 1.0, -4.0, 4.0);
    expect_scale(BoundsType::Stretch, 200, 100, 1.0, -1.0, 4.0, -6.0);
    expect_scale(BoundsType::ScaleInner, 0, 100, 1.0, 1.0, 6.0, 6.0);
    expect_scale(BoundsType::ScaleToWidth, 200, 0, 1.0, 1.0, 4.0, 4.0);
    expect_scale(BoundsType::ScaleToHeight, 0, 100, 1.0, 1.0, 6.0, 6.0);
}

#[test]
fn tall_source_uses_the_other_axis() {
    expect_scale(BoundsType::ScaleInner, 100, 200, 1.0, 1.0, 3.0, 3.0);
    expect_scale(BoundsType::ScaleOuter, 100, 200, 1.0, 1.0, 8.0, 8.0);
    expect_scale(BoundsType::ScaleToWidth, 100, 200, 1.0, 1.0, 8.0, 8.0);
    expect_scale(BoundsType::ScaleToHeight, 100, 200, 1.0, 1.0, 3.0, 3.0);
    expect_scale(BoundsType::Stretch, 100, 200, 1.0, 1.0, 8.0, 3.0);
    expect_scale(BoundsType::MaxOnly, 100, 200, 1.0, 1.0, 1.0, 1.0);
}

#[test]
fn uneven_base_scale_is_multiplied() {
    expect_scale(BoundsType::ScaleInner, 200, 100, 2.0, 1.0, 4.0, 2.0);
    expect_scale(BoundsType::ScaleOuter, 100, 200, 2.0, 0.5, 12.0, 3.0);
    expect_scale(BoundsType::ScaleToWidth, 200, 100, 2.0, 0.5, 4.0, 1.0);
    expect_scale(BoundsType::ScaleToHeight, 200, 100, 2.0, 0.5, 24.0, 6.0);
    expect_scale(BoundsType::MaxOnly, 2000, 100, -2.0, 0.5, -0.4, 0.1);
    expect_scale(BoundsType::MaxOnly, 100, 2000, -1.0, 2.0, -0.15, 0.3);
}

#[test]
fn zero_axis_keeps_its_scale() {
    expect_scale(BoundsType::Stretch, 0, 100, -2.0, 3.0, -2.0, 6.0);
    expect_scale(BoundsType::Stretch, 200, 0, -2.0, 3.0, -4.0, 3.0);
    expect_scale(BoundsType::ScaleToWidth, 0, 100, -2.0, 3.0, -2.0, 3.0);
    expect_scale(BoundsType::ScaleToHeight, 200, 0, -2.0, 3.0, -2.0, 3.0);
    expect_scale(BoundsType::ScaleOuter, 0, 100, -2.0, 3.0, -2.0, 3.0);
    expect_scale(BoundsType::ScaleInner, 200, 0, -2.0, 3.0, -4.0, 6.0);
}

#[test]
fn stretch_zero_scale_is_positive() {
    expect_scale(BoundsType::Stretch, 200, 100, 0.0, 0.0, 4.0, 6.0);
    expect_scale(BoundsType::Stretch, 200, 100, 0.0, -2.0, 4.0, -6.0);
}

#[test]
fn max_only_shrinks_only_past_the_box() {
    expect_scale(BoundsType::MaxOnly, 800, 600, 1.0, 1.0, 1.0, 1.0);
    expect_scale(BoundsType::MaxOnly, 400, 300, 2.0, 2.0, 2.0, 2.0);
    expect_scale(
        BoundsType::MaxOnly,
        401,
        300,
        2.0,
        2.0,
        1.995_012_5,
        1.995_012_5,
    );
    expect_scale(
        BoundsType::MaxOnly,
        400,
        301,
        2.0,
        2.0,
        1.993_355_5,
        1.993_355_5,
    );
}

#[test]
fn bounds_none_keeps_scale() {
    expect_scale(BoundsType::None, 200, 100, -2.0, 3.0, -2.0, 3.0);
    expect_scale(BoundsType::None, 0, 0, -2.0, 3.0, -2.0, 3.0);
}
