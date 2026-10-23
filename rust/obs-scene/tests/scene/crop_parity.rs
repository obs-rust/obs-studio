//! Tier 3: the shims of `obs_scene::ffi::scene_crop` against the original
//! `obs-scene-crop.c.h` (the oracle).
//!
//! Every output is compared bit for bit: the size and the crops as
//! integers, and the offset as `f32` bits, so -0 against +0 or a different
//! rounding would fail. No input is excluded: since #1840 the C is defined
//! on every one.
//!
//! Runs: the `test_scene_item_crop` values; sweeps of the factor's bit
//! patterns around the rescale's edges (2^31, 1, 0 and the NaNs) and of
//! the sum around the size; and proptests over full `i32` crops, `u32`
//! sizes and any-bit `f32` factors, with crops near the int edges and
//! the size, and factors that put the product next to +/-2^31.
//!
//! No intentional differences. `test_scene_item_crop` compiles the C into
//! itself, so this is what compares the swap; `test_scene_item_crop_rust`
//! runs the same C test against the shims, and the real-libobs crop and
//! nested scene tests see it.

use obs_c_oracle::scene_crop::{
    oracle_obs_scene_clamp_crop, oracle_obs_scene_crop_offset, oracle_obs_scene_cropped_size,
    oracle_obs_scene_scale_crop,
};
use obs_scene::ffi::scene_crop::{
    obs_scene_clamp_crop, obs_scene_crop_offset, obs_scene_cropped_size, obs_scene_scale_crop,
};
use proptest::prelude::*;

const INT_MAX: i32 = i32::MAX;
const INT_MIN: i32 = i32::MIN;

fn check_size(size: u32, crop1: i32, crop2: i32, bounds1: i32, bounds2: i32) {
    let rust = obs_scene_cropped_size(size, crop1, crop2, bounds1, bounds2);
    // SAFETY: the oracle takes and returns plain values.
    let c = unsafe { oracle_obs_scene_cropped_size(size, crop1, crop2, bounds1, bounds2) };
    assert_eq!(
        rust, c,
        "cropped_size({size}, {crop1}, {crop2}, {bounds1}, {bounds2})"
    );
}

fn check_scale(crop: i32, factor: f32) {
    let rust = obs_scene_scale_crop(crop, factor);
    // SAFETY: as above.
    let c = unsafe { oracle_obs_scene_scale_crop(crop, factor) };
    assert_eq!(
        rust,
        c,
        "scale_crop({crop}, {factor} [{:#010x}])",
        factor.to_bits()
    );
}

fn check_offset(crop: i32, bounds_crop: i32) {
    let rust = obs_scene_crop_offset(crop, bounds_crop);
    // SAFETY: as above.
    let c = unsafe { oracle_obs_scene_crop_offset(crop, bounds_crop) };
    assert!(
        rust.to_bits() == c.to_bits(),
        "crop_offset({crop}, {bounds_crop}): rust {rust} [{:#010x}] oracle {c} [{:#010x}]",
        rust.to_bits(),
        c.to_bits()
    );
}

fn check_clamp(crop: i32) {
    let rust = obs_scene_clamp_crop(crop);
    // SAFETY: as above.
    let c = unsafe { oracle_obs_scene_clamp_crop(crop) };
    assert_eq!(rust, c, "clamp_crop({crop})");
}

/// Crops at and next to the edges of `i32` and of 0.
const EDGE_CROPS: [i32; 13] = [
    INT_MIN,
    INT_MIN + 1,
    -16_777_217,
    -2,
    -1,
    0,
    1,
    2,
    16_777_216,
    16_777_217,
    2_147_483_520,
    INT_MAX - 1,
    INT_MAX,
];

/// Factors at the edges: the signed zeros and infinities, the smallest
/// and largest, 1 and the floats next to it, and NaNs of both signs.
fn edge_factors() -> Vec<f32> {
    vec![
        0.0,
        -0.0,
        f32::from_bits(1),
        f32::from_bits(0x8000_0001),
        f32::MIN_POSITIVE,
        0.25,
        0.5,
        f32::from_bits(0x3f7f_ffff),
        1.0,
        f32::from_bits(0x3f80_0001),
        -1.0,
        1.5,
        4000.0,
        2_147_483_520.0,
        2_147_483_648.0,
        f32::MAX,
        -f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        -f32::NAN,
        f32::from_bits(0x7f80_0001),
        f32::from_bits(0xffff_ffff),
    ]
}

/// The `test_scene_item_crop` values, through the shims.
#[test]
fn cmocka_cases_match_oracle() {
    for (size, a, b, c, d) in [
        (200, 0, 0, 0, 0),
        (200, 10, 30, 0, 0),
        (200, 10, 30, 5, 5),
        (200, 0, 0, 7, 3),
        (200, 100, 100, 0, 0),
        (200, 50, 50, 50, 50),
        (200, 199, 0, 0, 0),
        (200, 100, 101, 0, 0),
        (200, 0, 0, 0, 201),
        (0, 0, 0, 0, 0),
        (0, 1, 0, 0, 0),
        (1, 0, 0, 0, 1),
        (2, 0, 0, 0, 0),
        (3, 1, 0, 0, 0),
        (u32::MAX, 0, 0, 0, 0),
        (u32::MAX, INT_MAX, 0, 0, 0),
        (u32::MAX, INT_MAX, 1, 0, 0),
        (u32::MAX, INT_MAX, INT_MAX, 0, 0),
        (u32::MAX, INT_MAX, INT_MAX, 1, 0),
        (u32::MAX, INT_MAX, INT_MAX, 1, 1),
        (u32::MAX, INT_MAX, INT_MAX, INT_MAX, INT_MAX),
        (200, INT_MAX, INT_MAX, INT_MAX, INT_MAX),
        (1 << 31, INT_MAX, 1, 0, 0),
        (200, -1, 0, 0, 0),
        (200, -100, 0, 0, 0),
        (200, INT_MIN, INT_MIN, 0, 0),
        (200, INT_MIN, INT_MIN, INT_MIN, INT_MIN),
        (u32::MAX, INT_MIN, INT_MAX, 0, 0),
        (200, -10, 50, 0, 0),
        (200, -10, 10, 0, 0),
        (200, INT_MIN, INT_MAX, 1, 0),
        (200, 0, 0, -5, 25),
    ] {
        check_size(size, a, b, c, d);
    }

    for crop in
        EDGE_CROPS
            .into_iter()
            .chain([3, -3, 5, -5, 10, -10, 100, 1_000_000_000, -1_000_000_000])
    {
        for factor in edge_factors()
            .into_iter()
            .chain([0.33, 2.0, 1e30, 0.999_999_94, 1.000_000_1])
        {
            check_scale(crop, factor);
        }
    }

    for (crop, bounds) in [
        (0, 0),
        (10, 5),
        (-10, 5),
        (INT_MAX, 0),
        (INT_MAX, 1),
        (INT_MAX, INT_MAX),
        (INT_MIN, 0),
        (INT_MIN, INT_MIN),
        (INT_MAX, INT_MIN),
        (16_777_217, 0),
        (16_777_219, 0),
        (16_777_216, 1),
    ] {
        check_offset(crop, bounds);
    }

    for crop in EDGE_CROPS.into_iter().chain([5, -100]) {
        check_clamp(crop);
    }
}

/// Every factor bit pattern near the rescale's edges, for crops of +/-1,
/// +/-2 and the int edges: 2^31 and the floats next to it, 1, the
/// smallest floats of both signs, and the first and last NaNs.
#[test]
fn factor_bit_sweeps_match_oracle() {
    let bases: [u32; 8] = [
        0x4f00_0000, // 2^31
        0x4e80_0000, // 2^30, x 2 is 2^31
        0x3f80_0000, // 1
        0x3f00_0000, // 0.5
        0x0000_0000, // +0 and the subnormals
        0x7f80_0000, // +inf and the first NaNs
        0x7fff_ff00, // the last NaNs
        0x3000_0000, // 2^-31, x INT_MAX is near 1
    ];
    for base in bases {
        for delta in 0..=0x200u32 {
            let low = base.wrapping_sub(0x100).wrapping_add(delta);
            for bits in [low, low | 0x8000_0000] {
                let factor = f32::from_bits(bits);
                for crop in [1, -1, 2, -2, INT_MAX, INT_MIN, INT_MAX - 1, INT_MIN + 1] {
                    check_scale(crop, factor);
                }
            }
        }
    }
}

/// Every sum from size - 3 to size + 3, split four ways, for sizes at the
/// edges of `u32` and small ones.
#[test]
fn sums_around_the_size_match_oracle() {
    for size in [
        0u32,
        1,
        2,
        3,
        200,
        (1 << 31) - 1,
        1 << 31,
        u32::MAX - 1,
        u32::MAX,
    ] {
        for delta in -3i64..=3 {
            let sum = i64::from(size) + delta;
            // Split the sum into four ints, each as large as it can be.
            let mut left = sum;
            let mut parts = [0i32; 4];
            for part in &mut parts {
                let take = left.clamp(i64::from(INT_MIN), i64::from(INT_MAX));
                *part = i32::try_from(take).unwrap();
                left -= take;
            }
            if left == 0 {
                let [a, b, c, d] = parts;
                check_size(size, a, b, c, d);
                check_size(size, d, c, b, a);
            }
        }
    }
}

/// Any i32, biased to the edges of the int range and of 0.
fn crop() -> impl Strategy<Value = i32> {
    prop_oneof![
        4 => any::<i32>(),
        2 => prop::sample::select(EDGE_CROPS.to_vec()),
        2 => -10_000i32..10_000,
        1 => (INT_MAX - 1000)..=INT_MAX,
        1 => INT_MIN..=(INT_MIN + 1000),
    ]
}

/// Any bit pattern, or one of the edge factors.
fn factor() -> impl Strategy<Value = f32> {
    prop_oneof![
        4 => any::<u32>().prop_map(f32::from_bits),
        2 => prop::sample::select(edge_factors()),
        2 => -8.0f32..8.0,
        1 => prop_oneof![-f32::MAX..-1.0e30f32, 1.0e30f32..f32::MAX],
    ]
}

/// Any u32, biased to the small sizes and the edges.
fn size() -> impl Strategy<Value = u32> {
    prop_oneof![
        4 => any::<u32>(),
        2 => 0u32..4096,
        1 => prop::sample::select(vec![0u32, 1, 2, 3, (1 << 31) - 1, 1 << 31, u32::MAX - 1, u32::MAX]),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8192))]

    #[test]
    fn random_sizes_match_oracle(
        size in size(),
        crop1 in crop(),
        crop2 in crop(),
        bounds1 in crop(),
        bounds2 in crop(),
    ) {
        check_size(size, crop1, crop2, bounds1, bounds2);
    }

    /// Crops that sum to within a few pixels of the size: two crops are
    /// any, and the other two split the rest, so most cases land there.
    #[test]
    fn sums_near_the_size_match_oracle(
        size in size(),
        crop1 in crop(),
        crop2 in crop(),
        aim in crop(),
        delta in -3i64..=3,
    ) {
        let rest = i64::from(size) + delta - i64::from(crop1) - i64::from(crop2);
        let take = (rest - i64::from(aim)).clamp(i64::from(INT_MIN), i64::from(INT_MAX));
        let bounds1 = i32::try_from(take).unwrap();
        if let Ok(bounds2) = i32::try_from(rest - take) {
            check_size(size, crop1, crop2, bounds1, bounds2);
            check_size(size, bounds2, bounds1, crop2, crop1);
        }
    }

    #[test]
    fn random_scales_match_oracle(crop in crop(), factor in factor()) {
        check_scale(crop, factor);
    }

    /// Factors that put the product within a few floats of +/-2^31, or of
    /// a whole number, so the edges and the truncation are reached.
    #[test]
    fn products_near_the_edges_match_oracle(
        crop in crop().prop_filter("nonzero", |c| *c != 0),
        target in prop::sample::select(vec![2_147_483_648.0f32, -2_147_483_648.0, 1.0, -1.0, 7.0, 1e6]),
        nudge in -4i32..=4,
    ) {
        let factor = target / crop as f32;
        let bits = factor.to_bits().wrapping_add_signed(nudge);
        check_scale(crop, f32::from_bits(bits));
    }

    #[test]
    fn random_offsets_match_oracle(crop in crop(), bounds_crop in crop()) {
        check_offset(crop, bounds_crop);
    }

    #[test]
    fn random_clamps_match_oracle(crop in crop()) {
        check_clamp(crop);
    }
}
