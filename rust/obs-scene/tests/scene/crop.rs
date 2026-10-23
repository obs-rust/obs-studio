//! Tier 1: `obs_scene::crop`, the scene item crop arithmetic.
//!
//! The cases without a prefix port the first eight cases of
//! `test/cmocka/test_scene_item_crop.c` 1:1, under the same names, in the
//! same order, with the same values and expectations. Its other three
//! cases go through the real libobs (a scene, an item, the setter and a
//! nested scene rendered at new sizes) and have no Tier 1 twin.
//!
//! The `edge_*` cases are new: the size against every small sum, the
//! floats next to the edges of the rescale, NaN payloads, the product's
//! rounding, the offset's ties and the clamp of every sign.
//!
//! Cases marked characterized, not endorsed, are what the C does today,
//! including the odd parts.

use obs_scene::crop::{clamp_crop, crop_offset, cropped_size, scale_crop};

/// Bit for bit: -0 is not +0.
fn check_float(got: f32, want: f32, what: &str) {
    assert!(
        got.to_bits() == want.to_bits(),
        "{what}: {got} ({:#010x}), expected {want} ({:#010x})",
        got.to_bits(),
        want.to_bits()
    );
}

fn check_size(size: u32, crop1: i32, crop2: i32, bounds1: i32, bounds2: i32, want: u32) {
    assert_eq!(
        cropped_size(size, crop1, crop2, bounds1, bounds2),
        want,
        "cropped_size({size}, {crop1}, {crop2}, {bounds1}, {bounds2})"
    );
}

fn check_scale(crop: i32, factor: f32, want: i32) {
    assert_eq!(
        scale_crop(crop, factor),
        want,
        "scale_crop({crop}, {factor} [{:#010x}])",
        factor.to_bits()
    );
}

fn check_offset(crop: i32, bounds_crop: i32, want: f32) {
    check_float(
        crop_offset(crop, bounds_crop),
        want,
        &format!("crop_offset({crop}, {bounds_crop})"),
    );
}

fn check_clamp(crop: i32, want: i32) {
    assert_eq!(clamp_crop(crop), want, "clamp_crop({crop})");
}

const INT_MAX: i32 = i32::MAX;
const INT_MIN: i32 = i32::MIN;
const UINT32_MAX: u32 = u32::MAX;
/// `INT_MAX as uint32_t + 1`, 2^31.
const TWO_31: u32 = 1 << 31;

/// A crop that sums to the size leaves 0; one more pixel leaves 2.
#[test]
fn cropped_size_at_the_size() {
    check_size(200, 0, 0, 0, 0, 200);
    check_size(200, 10, 30, 0, 0, 160);
    check_size(200, 10, 30, 5, 5, 150);
    check_size(200, 0, 0, 7, 3, 190);
    check_size(200, 100, 100, 0, 0, 0);
    check_size(200, 50, 50, 50, 50, 0);
    check_size(200, 199, 0, 0, 0, 1);
    check_size(200, 100, 101, 0, 0, 2);
    check_size(200, 0, 0, 0, 201, 2);
    check_size(0, 0, 0, 0, 0, 0);
    check_size(0, 1, 0, 0, 0, 2);
    check_size(1, 0, 0, 0, 1, 0);
    check_size(2, 0, 0, 0, 0, 2);
    // 2 px wide either way: by the crop or because it is 2 px.
    check_size(3, 1, 0, 0, 0, 2);
}

/// The sum is 64 bits, so the four crops can be past the int range.
#[test]
fn cropped_size_past_the_int_range() {
    check_size(UINT32_MAX, 0, 0, 0, 0, UINT32_MAX);
    check_size(UINT32_MAX, INT_MAX, 0, 0, 0, TWO_31);
    check_size(UINT32_MAX, INT_MAX, 1, 0, 0, TWO_31 - 1);
    check_size(UINT32_MAX, INT_MAX, INT_MAX, 0, 0, 1);
    // Exactly UINT32_MAX: 0 left.
    check_size(UINT32_MAX, INT_MAX, INT_MAX, 1, 0, 0);
    // 2^32: one past the largest size.
    check_size(UINT32_MAX, INT_MAX, INT_MAX, 1, 1, 2);
    check_size(UINT32_MAX, INT_MAX, INT_MAX, INT_MAX, INT_MAX, 2);
    check_size(200, INT_MAX, INT_MAX, INT_MAX, INT_MAX, 2);
    check_size(TWO_31, INT_MAX, 1, 0, 0, 0);
}

/// A negative sum crops all of it. A negative crop that the others make up
/// for is subtracted. Characterized, not endorsed: the setter never stores
/// a negative crop, but the bounds crop is not clamped there.
#[test]
fn cropped_size_negative() {
    check_size(200, -1, 0, 0, 0, 2);
    check_size(200, -100, 0, 0, 0, 2);
    check_size(200, INT_MIN, INT_MIN, 0, 0, 2);
    check_size(200, INT_MIN, INT_MIN, INT_MIN, INT_MIN, 2);
    check_size(UINT32_MAX, INT_MIN, INT_MAX, 0, 0, 2);
    check_size(200, -10, 50, 0, 0, 160);
    check_size(200, -10, 10, 0, 0, 200);
    check_size(200, INT_MIN, INT_MAX, 1, 0, 200);
    check_size(200, 0, 0, -5, 25, 180);
}

/// The rescale of a nested scene's crop, truncated toward 0.
#[test]
fn scale_crop_truncates() {
    check_scale(0, 4000.0, 0);
    check_scale(100, 1.0, 100);
    check_scale(100, 2.0, 200);
    check_scale(100, 0.5, 50);
    check_scale(10, 0.33, 3);
    check_scale(-10, 0.33, -3);
    check_scale(3, 1.5, 4);
    check_scale(-3, 1.5, -4);
    check_scale(1, 0.999_999_94, 0);
    check_scale(-1, 0.999_999_94, 0);
    check_scale(5, 0.0, 0);
    // -0 is 0.
    check_scale(5, -0.0, 0);
    check_scale(5, -1.0, -5);
    check_scale(1_000_000_000, 2.0, 2_000_000_000);
    // Characterized, not endorsed: the crop is a float first, so a crop
    // past 2^24 loses its low bits even at a factor of 1.
    check_scale(16_777_217, 1.0, 16_777_216);
    check_scale(2_147_483_520, 1.0, 2_147_483_520);
    check_scale(-2_147_483_520, 1.0, -2_147_483_520);
    check_scale(2_147_483_583, 1.0, 2_147_483_520);
    // The smallest factors: the product is 0 or below 1.
    check_scale(INT_MAX, f32::MIN_POSITIVE, 0);
    check_scale(INT_MAX, f32::from_bits(1), 0);
    check_scale(INT_MIN, f32::from_bits(0x8000_0001), 0);
}

/// NaN gives 0. The infinities and huge factors give the nearest int.
#[test]
fn scale_crop_nan_inf_huge() {
    check_scale(5, f32::NAN, 0);
    check_scale(-5, -f32::NAN, 0);
    check_scale(0, f32::NAN, 0);
    check_scale(INT_MAX, f32::from_bits(0x7fc0_0001), 0);
    check_scale(INT_MIN, f32::from_bits(0xff80_0001), 0);
    check_scale(1, f32::INFINITY, INT_MAX);
    check_scale(-1, f32::INFINITY, INT_MIN);
    check_scale(1, f32::NEG_INFINITY, INT_MIN);
    check_scale(-1, f32::NEG_INFINITY, INT_MAX);
    // 0 x inf is NaN.
    check_scale(0, f32::INFINITY, 0);
    check_scale(0, f32::NEG_INFINITY, 0);
    check_scale(1, f32::MAX, INT_MAX);
    check_scale(-1, f32::MAX, INT_MIN);
    check_scale(1, -f32::MAX, INT_MIN);
    check_scale(INT_MAX, f32::MAX, INT_MAX);
    // The product overflows to inf.
    check_scale(INT_MIN, f32::MAX, INT_MIN);
    check_scale(INT_MIN, -f32::MAX, INT_MAX);
    check_scale(1_000_000_000, 4000.0, INT_MAX);
    check_scale(-1_000_000_000, 4000.0, INT_MIN);
    check_scale(2, 1e30, INT_MAX);
}

/// At `INT_MIN` and `INT_MAX`. `INT_MAX as f32` is 2^31, so `INT_MAX` x 1
/// is at the upper edge, and -2^31 is the lower edge, both included.
#[test]
fn scale_crop_int_edges() {
    check_scale(INT_MAX, 1.0, INT_MAX);
    check_scale(INT_MIN, 1.0, INT_MIN);
    check_scale(INT_MIN, -1.0, INT_MAX);
    check_scale(INT_MAX, -1.0, INT_MIN);
    check_scale(INT_MAX, 0.5, 1_073_741_824);
    check_scale(INT_MIN, 0.5, -1_073_741_824);
    check_scale(INT_MAX, 0.0, 0);
    check_scale(INT_MIN, 0.0, 0);
    // The largest float below 2^31, on both sides.
    check_scale(1, 2_147_483_520.0, 2_147_483_520);
    check_scale(-1, 2_147_483_520.0, -2_147_483_520);
    check_scale(1, 2_147_483_648.0, INT_MAX);
    check_scale(-1, 2_147_483_648.0, INT_MIN);
    check_scale(INT_MAX, 0.999_999_94, 2_147_483_520);
    check_scale(INT_MIN, 0.999_999_94, -2_147_483_520);
    check_scale(INT_MAX, 1.000_000_1, INT_MAX);
    check_scale(INT_MIN, 1.000_000_1, INT_MIN);
}

/// The offset is the 64-bit sum as a float, rounded to the nearest.
#[test]
fn crop_offset_sums() {
    check_offset(0, 0, 0.0);
    check_offset(10, 5, 15.0);
    check_offset(-10, 5, -5.0);
    check_offset(INT_MAX, 0, 2_147_483_648.0);
    check_offset(INT_MAX, 1, 2_147_483_648.0);
    check_offset(INT_MAX, INT_MAX, 4_294_967_294.0);
    check_offset(INT_MIN, 0, -2_147_483_648.0);
    check_offset(INT_MIN, INT_MIN, -4_294_967_296.0);
    check_offset(INT_MAX, INT_MIN, -1.0);
    // Rounded: past 2^24 not every int is a float.
    check_offset(16_777_217, 0, 16_777_216.0);
    check_offset(16_777_219, 0, 16_777_220.0);
    check_offset(16_777_216, 1, 16_777_216.0);
}

/// A negative crop is 0.
#[test]
fn clamp_crop_values() {
    check_clamp(0, 0);
    check_clamp(5, 5);
    check_clamp(INT_MAX, INT_MAX);
    check_clamp(-1, 0);
    check_clamp(-100, 0);
    check_clamp(INT_MIN, 0);
}

/// Every split of every small sum against every small size: 0 to `size`
/// leaves the rest, anything else 2.
#[test]
fn edge_cropped_size_every_small_sum() {
    for size in 0u32..6 {
        for sum in -4i32..10 {
            let want = u32::try_from(sum)
                .ok()
                .filter(|&s| s <= size)
                .map_or(2, |s| size - s);
            for (a, b, c, d) in [
                (sum, 0, 0, 0),
                (0, sum, 0, 0),
                (0, 0, sum, 0),
                (0, 0, 0, sum),
                (sum - 3, 1, 1, 1),
                (1, sum - 1, 0, 0),
            ] {
                check_size(size, a, b, c, d, want);
            }
        }
    }
}

/// The sum at the edges of the `u32` range of the size.
#[test]
fn edge_cropped_size_u32_edges() {
    // The sum is UINT32_MAX - 1, UINT32_MAX and 2^32.
    check_size(UINT32_MAX, INT_MAX, INT_MAX, 0, 0, 1);
    check_size(UINT32_MAX - 1, INT_MAX, INT_MAX, 1, 0, 2);
    check_size(UINT32_MAX, INT_MAX, INT_MAX, 1, 1, 2);
    // The largest sum, 4 x INT_MAX, and the smallest, 4 x INT_MIN.
    check_size(UINT32_MAX, INT_MAX, INT_MAX, INT_MAX, INT_MAX, 2);
    check_size(UINT32_MAX, INT_MIN, INT_MIN, INT_MIN, INT_MIN, 2);
    // Sums that wrap a 32-bit int to a small value: 2^32 + 5 is not 5.
    check_size(200, INT_MAX, INT_MAX, 7, 0, 2);
    check_size(200, INT_MIN, INT_MIN, 5, 0, 2);
}

/// The floats next to the edges of the rescale: one below 2^31 truncates,
/// 2^31 and up saturate, on each side.
#[test]
fn edge_scale_crop_next_to_the_edges() {
    let below = f32::from_bits(0x4eff_ffff);
    let edge = f32::from_bits(0x4f00_0000);
    let above = f32::from_bits(0x4f00_0001);
    check_scale(1, below, 2_147_483_520);
    check_scale(1, edge, INT_MAX);
    check_scale(1, above, INT_MAX);
    check_scale(-1, below, -2_147_483_520);
    check_scale(-1, edge, INT_MIN);
    check_scale(-1, above, INT_MIN);
    // Just under 1 and just over -1: 0.
    check_scale(1, f32::from_bits(0x3f7f_ffff), 0);
    check_scale(-1, f32::from_bits(0x3f7f_ffff), 0);
    check_scale(1, 1.0, 1);
    check_scale(-1, 1.0, -1);
}

/// Every NaN payload and sign gives 0.
#[test]
fn edge_scale_crop_nan_payloads() {
    for bits in [
        0x7f80_0001u32,
        0x7fbf_ffff,
        0x7fc0_0000,
        0x7fff_ffff,
        0xff80_0001,
        0xffc0_0000,
        0xffff_ffff,
    ] {
        for crop in [INT_MIN, -1, 0, 1, INT_MAX] {
            check_scale(crop, f32::from_bits(bits), 0);
        }
    }
}

/// The product is one float multiply, rounded to the nearest, then
/// truncated: 3 x 0.1 is 0.3 (below 1), 10 x 0.1 rounds up to 1.
#[test]
fn edge_scale_crop_product_rounding() {
    check_scale(3, 0.1, 0);
    check_scale(10, 0.1, 1);
    check_scale(-10, 0.1, -1);
    check_scale(7, 1.0 / 7.0, 1);
    check_scale(49, 1.0 / 7.0, 7);
    check_scale(100, 0.07, 7);
    // 1 / 4000 of INT_MAX, as in the nested scene that shrinks back.
    check_scale(INT_MAX, 100.0 / 400_000.0, 536_870);
}

/// The offset's ties go to the even float; -0 never comes out of a sum.
#[test]
fn edge_crop_offset_ties() {
    check_offset(16_777_217, 0, 16_777_216.0);
    check_offset(16_777_216, 3, 16_777_220.0);
    check_offset(0, 16_777_219, 16_777_220.0);
    check_offset(-16_777_217, 0, -16_777_216.0);
    check_offset(INT_MAX, -INT_MAX, 0.0);
    check_offset(0, 0, 0.0);
    // 2^32 - 2 rounds to 2^32; 2^31 - 1 rounds to 2^31.
    check_offset(INT_MAX, INT_MAX, 4_294_967_296.0);
    check_offset(INT_MAX, 0, 2_147_483_648.0);
    check_offset(INT_MIN, -1, -2_147_483_648.0);
}

/// The clamp keeps every crop of 0 and up, and gives 0 for every other.
#[test]
fn edge_clamp_every_sign() {
    for crop in [0, 1, 2, 1000, INT_MAX - 1, INT_MAX] {
        check_clamp(crop, crop);
    }
    for crop in [-1, -2, -1000, INT_MIN + 1, INT_MIN] {
        check_clamp(crop, 0);
    }
}

/// The product is an `f32` multiply, not a wider one: 16777217 is
/// 16777216 as a float, and x 3 is 50331648, where the exact 50331651
/// would round to 50331652.
#[test]
fn edge_scale_crop_product_is_f32() {
    check_scale(16_777_217, 3.0, 50_331_648);
    check_scale(-16_777_217, 3.0, -50_331_648);
    check_scale(16_777_219, 3.0, 50_331_660);
}
