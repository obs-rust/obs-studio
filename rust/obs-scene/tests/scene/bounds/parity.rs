//! Tier 3: the shim against the original `obs-scene-bounds.c.h`.
//!
//! No intentional differences. A null `bounds` or `scale` is not called.
//! `vec2_mulf` stays the header inline on the C side.

use std::ffi::c_int;

use obs_c_oracle::scene_bounds::oracle_obs_bounds_fit_scale;
use obs_c_oracle::vec2::OracleVec2;
use obs_graphics::ffi::vec2::vec2;
use obs_scene::ffi::scene_bounds::obs_bounds_fit_scale;
use proptest::prelude::*;

/// Bit-exact equality, except that any NaN equals any NaN.
fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn check(kind: i32, bounds_x: f32, bounds_y: f32, cx: u32, cy: u32, scale_x: f32, scale_y: f32) {
    let rust_bounds = vec2 {
        x: bounds_x,
        y: bounds_y,
    };
    let mut rust_scale = vec2 {
        x: scale_x,
        y: scale_y,
    };
    let c_bounds = OracleVec2 {
        x: bounds_x,
        y: bounds_y,
    };
    let mut c_scale = OracleVec2 {
        x: scale_x,
        y: scale_y,
    };
    // SAFETY: both pairs are live stack values. The shim and the oracle
    // write the scale and leave the box alone.
    unsafe {
        obs_bounds_fit_scale(
            kind as c_int,
            &raw const rust_bounds,
            cx,
            cy,
            &raw mut rust_scale,
        );
        oracle_obs_bounds_fit_scale(kind as c_int, &raw const c_bounds, cx, cy, &raw mut c_scale);
    }
    assert!(
        same(rust_bounds.x, bounds_x) && same(rust_bounds.y, bounds_y),
        "rust wrote the box"
    );
    assert!(
        same(c_bounds.x, bounds_x) && same(c_bounds.y, bounds_y),
        "oracle wrote the box"
    );
    assert!(
        same(rust_scale.x, c_scale.x) && same(rust_scale.y, c_scale.y),
        "kind {kind} bounds ({bounds_x}, {bounds_y}) {cx}x{cy} scale ({scale_x}, {scale_y}): rust ({}, {}) oracle ({}, {})",
        rust_scale.x,
        rust_scale.y,
        c_scale.x,
        c_scale.y
    );
}

#[test]
fn cmocka_cases_match_oracle() {
    let cases = [
        (2, 800.0, 600.0, 100, 200, 1.0, 1.0),
        (3, 800.0, 600.0, 100, 200, 1.0, 1.0),
        (4, 800.0, 600.0, 100, 200, 1.0, 1.0),
        (5, 800.0, 600.0, 100, 200, 1.0, 1.0),
        (1, 800.0, 600.0, 100, 200, 1.0, 1.0),
        (6, 800.0, 600.0, 100, 200, 1.0, 1.0),
        (2, 800.0, 600.0, 200, 100, 2.0, 1.0),
        (3, 800.0, 600.0, 100, 200, 2.0, 0.5),
        (4, 800.0, 600.0, 200, 100, 2.0, 0.5),
        (5, 800.0, 600.0, 200, 100, 2.0, 0.5),
        (6, 800.0, 600.0, 2000, 100, -2.0, 0.5),
        (6, 800.0, 600.0, 100, 2000, -1.0, 2.0),
        (1, 800.0, 600.0, 0, 100, -2.0, 3.0),
        (1, 800.0, 600.0, 200, 0, -2.0, 3.0),
        (4, 800.0, 600.0, 0, 100, -2.0, 3.0),
        (5, 800.0, 600.0, 200, 0, -2.0, 3.0),
        (3, 800.0, 600.0, 0, 100, -2.0, 3.0),
        (2, 800.0, 600.0, 200, 0, -2.0, 3.0),
        (1, 800.0, 600.0, 200, 100, 0.0, 0.0),
        (1, 800.0, 600.0, 200, 100, 0.0, -2.0),
        (6, 800.0, 600.0, 800, 600, 1.0, 1.0),
        (6, 800.0, 600.0, 400, 300, 2.0, 2.0),
        (6, 800.0, 600.0, 401, 300, 2.0, 2.0),
        (6, 800.0, 600.0, 400, 301, 2.0, 2.0),
        (0, 800.0, 600.0, 200, 100, -2.0, 3.0),
        (0, 800.0, 600.0, 0, 0, -2.0, 3.0),
        (7, 800.0, 600.0, 200, 100, -2.0, 3.0),
    ];
    for (kind, bx, by, cx, cy, sx, sy) in cases {
        check(kind, bx, by, cx, cy, sx, sy);
    }
}

/// Any bit pattern, so NaNs, infinities, zeros and subnormals all appear.
fn any_f32() -> impl Strategy<Value = f32> {
    any::<u32>().prop_map(f32::from_bits)
}

proptest! {
    #[test]
    fn random_inputs_match_oracle(
        kind in -1i32..=8,
        bounds_x in any_f32(),
        bounds_y in any_f32(),
        cx in any::<u32>(),
        cy in any::<u32>(),
        scale_x in any_f32(),
        scale_y in any_f32(),
    ) {
        check(kind, bounds_x, bounds_y, cx, cy, scale_x, scale_y);
    }
}
