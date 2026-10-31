//! Tier 1: `obs_graphics::image_pq_scrgb`, the JPEG XR PQ-to-scRGB
//! conversion of the image loader's Windows host.
//!
//! The `steps_*` cases port `test/cmocka/test_graphics_ffmpeg_pq_steps.c`
//! 1:1: its 10 cases under the same names, in the same order, with the
//! same values and expectations, the characterized-not-endorsed negative
//! 709 values included. As in the C test, values that depend on `powf` are
//! checked to within a float's or a half's last place, and the rest
//! exactly.
//!
//! The `edge_*` cases are new: `half_from_f32` against a model of
//! `half.h`'s `half_from_float()` (its expressions inline, with the
//! denormal shift done in 64 bits) over every exponent with mantissas at
//! and around each rounding point, and by proptest; the floats below
//! 2^-45 in magnitude, where the C's 32-bit shift is 32 or more and so
//! undefined, give zero with their sign (the intentional difference); and
//! `pq10_to_scrgb` over buffers: texel by texel, with bits 30-31, and a
//! `dst` or `src` that ends early.

use obs_graphics::image_pq_scrgb::{
    ALPHA_ONE, DST_TEXEL_BYTES, SRC_TEXEL_BYTES, half_from_f32, pq_to_linear, pq10_to_scrgb,
};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// helpers

fn texel(red: u32, green: u32, blue: u32) -> u32 {
    red << 20 | green << 10 | blue
}

fn convert(value: u32) -> [u16; 4] {
    let mut dst = [0xa5_u8; DST_TEXEL_BYTES];
    pq10_to_scrgb(&mut dst, &value.to_ne_bytes());
    let mut out = [0_u16; 4];
    for (half, bytes) in out.iter_mut().zip(dst.as_chunks::<2>().0) {
        *half = u16::from_ne_bytes(*bytes);
    }
    out
}

fn expect_texel(value: u32, red: u16, green: u16, blue: u16) {
    assert_eq!(
        convert(value),
        [red, green, blue, ALPHA_ONE],
        "texel {value:#010x}"
    );
}

/// Within one unit in the last place of the half, with the same sign.
fn expect_half_near(got: u16, want: u16) {
    assert!(
        got & 0x8000 == want & 0x8000 && got.abs_diff(want) <= 1,
        "half {got:#06x}, expected {want:#06x} within 1"
    );
}

fn expect_texel_near(value: u32, red: u16, green: u16, blue: u16) {
    let out = convert(value);
    expect_half_near(out[0], red);
    expect_half_near(out[1], green);
    expect_half_near(out[2], blue);
    assert_eq!(out[3], ALPHA_ONE);
}

fn expect_float_near(got: f32, want: f32) {
    assert!(
        (got - want).abs() <= want.abs() * 4e-7,
        "{got:e}, expected {want:e}"
    );
}

fn half_bits(float_bits: u32) -> u16 {
    half_from_f32(f32::from_bits(float_bits))
}

/// `half_from_float()` of `half.h`, its expressions inline, with the
/// denormal shift done in 64 bits (zero past 63) so it is defined for
/// every exponent.
fn model_half(float_bits: u32) -> u16 {
    let sign = (float_bits & 0x8000_0000) >> 16;
    let mut value = float_bits & 0x7fff_ffff;
    let result = if value > 0x477f_e000 {
        if value & 0x7f80_0000 == 0x7f80_0000 && value & 0x007f_ffff != 0 {
            0x7fff
        } else {
            0x7c00
        }
    } else if value == 0 {
        0
    } else {
        if value < 0x3880_0000 {
            let shift = 113 - (value >> 23);
            value = u64::from(0x0080_0000 | (value & 0x007f_ffff))
                .checked_shr(shift)
                .map_or(0, |v| v as u32);
        } else {
            value = value.wrapping_add(0xc800_0000);
        }
        (value.wrapping_add(0x0fff).wrapping_add((value >> 13) & 1) >> 13) & 0x7fff
    };
    (result | sign) as u16
}

// ---------------------------------------------------------------------------
// test_graphics_ffmpeg_pq_steps.c, 1:1

#[test]
#[allow(clippy::float_cmp, reason = "code values 0 and 1023 are exact")]
#[allow(
    clippy::excessive_precision,
    reason = "the C test's values, spelled as it spells them"
)]
fn steps_pq_to_linear() {
    assert_eq!(pq_to_linear(0.0 / 1023.0), 0.0);
    expect_float_near(pq_to_linear(1.0 / 1023.0), 4.042_244_76e-09);
    expect_float_near(pq_to_linear(2.0 / 1023.0), 1.311_137_42e-08);
    expect_float_near(pq_to_linear(512.0 / 1023.0), 0.009_269_863_37);
    expect_float_near(pq_to_linear(1022.0 / 1023.0), 0.990_744_293);
    assert_eq!(pq_to_linear(1023.0 / 1023.0), 1.0);
}

#[test]
fn steps_half_from_float() {
    assert_eq!(half_bits(0x0000_0000), 0x0000);
    assert_eq!(half_bits(0x8000_0000), 0x8000);
    assert_eq!(half_bits(0x3880_0000), 0x0400); // 2^-14
    assert_eq!(half_bits(0x3380_0000), 0x0001); // 2^-24
    assert_eq!(half_bits(0xb380_0000), 0x8001);
    assert_eq!(half_bits(0x477f_e000), 0x7bff); // 65504
    assert_eq!(half_bits(0x477f_f000), 0x7c00); // 65520
    assert_eq!(half_bits(0x7f80_0000), 0x7c00);
    assert_eq!(half_bits(0xff80_0000), 0xfc00);
    assert_eq!(half_bits(0x7fc0_0000), 0x7fff);
    assert_eq!(half_bits(0xffc0_0000), 0xffff);
}

/// 0 and 10 000 nits: 0 and 125.0 (10 000 / 80).
#[test]
fn steps_black_and_white() {
    expect_texel(texel(0, 0, 0), 0x0000, 0x0000, 0x0000);
    expect_texel(texel(1023, 1023, 1023), 0x57d0, 0x57d0, 0x57d0);
}

#[test]
fn steps_grey_ramp() {
    const HALVES: [u16; 8] = [
        0x0000, 0x1f9f, 0x2c23, 0x34ef, 0x3ca3, 0x43c5, 0x4a30, 0x50e1,
    ];
    for (i, half) in (0_u32..).zip(HALVES) {
        expect_texel_near(texel(i * 128, i * 128, i * 128), half, half, half);
    }
}

/// A full-scale BT.2020 primary lies outside BT.709: two of its 709 values
/// are negative, and the texture keeps them. Characterized, not endorsed.
#[test]
fn steps_primaries() {
    expect_texel(texel(1023, 0, 0), 0x5a7c, 0xcbc9, 0xc08a);
    expect_texel(texel(0, 1023, 0), 0xd497, 0x586d, 0xca49);
    expect_texel(texel(0, 0, 1023), 0xc88e, 0xbc2d, 0x585f);
}

#[test]
fn steps_primaries_at_code_value_one() {
    expect_texel_near(texel(1, 0, 0), 0x000e, 0x8001, 0x8000);
    expect_texel_near(texel(0, 1, 0), 0x8005, 0x000a, 0x8001);
    expect_texel_near(texel(0, 0, 1), 0x8001, 0x8000, 0x0009);
}

/// Characterized, not endorsed.
#[test]
fn steps_negative_709() {
    expect_texel_near(texel(0x100, 0x3c0, 0x080), 0xd11e, 0x54f2, 0xc704);
}

#[test]
fn steps_high_bits_ignored() {
    let base = texel(0x123, 0x2ab, 0x077);
    let want = convert(base);
    expect_texel_near(base, 0xc26c, 0x4681, 0xb897);
    for high in 1..4_u32 {
        expect_texel(base | high << 30, want[0], want[1], want[2]);
    }
}

#[test]
fn steps_buffer() {
    let texels = [texel(1023, 0, 0), texel(0, 0, 0), texel(1023, 1023, 1023)];
    let src: Vec<u8> = texels.iter().flat_map(|t| t.to_ne_bytes()).collect();
    let mut dst = [0xa5_u8; 3 * DST_TEXEL_BYTES + 1];
    pq10_to_scrgb(&mut dst[..3 * DST_TEXEL_BYTES], &src);
    let want: [u16; 12] = [
        0x5a7c, 0xcbc9, 0xc08a, ALPHA_ONE, 0x0000, 0x0000, 0x0000, ALPHA_ONE, 0x57d0, 0x57d0,
        0x57d0, ALPHA_ONE,
    ];
    for (i, half) in want.iter().enumerate() {
        assert_eq!(
            u16::from_ne_bytes([dst[i * 2], dst[i * 2 + 1]]),
            *half,
            "half {i}"
        );
    }
    assert_eq!(dst[3 * DST_TEXEL_BYTES], 0xa5);
}

#[test]
fn steps_empty() {
    let mut dst = [0xa5_u8; DST_TEXEL_BYTES];
    pq10_to_scrgb(&mut dst[..0], &[0xff; 4][..0]);
    assert_eq!(dst, [0xa5; DST_TEXEL_BYTES]);
}

// ---------------------------------------------------------------------------
// edge cases

/// Every exponent with the mantissas at, below and above each rounding
/// point of the half it lands on, both signs, against the model. Below
/// 2^-45 (biased exponent 81 or less) the model's 64-bit shift gives zero.
#[test]
fn edge_half_every_exponent() {
    let mantissas = [
        0x0000_0000,
        0x0000_0001,
        0x0000_0fff,
        0x0000_1000,
        0x0000_1001,
        0x0000_2000,
        0x0000_3000,
        0x0040_0000,
        0x007f_e000,
        0x007f_efff,
        0x007f_f000,
        0x007f_ffff,
    ];
    for exponent in 0..=0xff_u32 {
        for mantissa in mantissas {
            for sign in [0, 0x8000_0000_u32] {
                let bits = sign | exponent << 23 | mantissa;
                assert_eq!(half_bits(bits), model_half(bits), "float {bits:#010x}");
            }
        }
    }
}

/// Where the C's `uint32_t` shift is 32 or more (biased exponent 81 or
/// less, a float below 2^-45 in magnitude) it is undefined, and x86 masks
/// it into a non-zero half. The rounding gives zero there, as it does for
/// every shift from 24 to 31: the core returns zero with the float's sign.
#[test]
fn edge_half_tiny_is_signed_zero() {
    for exponent in 0..=88_u32 {
        for mantissa in [0, 1, 0x0040_0000, 0x007f_ffff] {
            let bits = exponent << 23 | mantissa;
            assert_eq!(half_bits(bits), 0x0000, "float {bits:#010x}");
            assert_eq!(half_bits(bits | 0x8000_0000), 0x8000, "float -{bits:#010x}");
        }
    }
    assert_eq!(half_bits(89 << 23), 0x0000); // shift 24
    assert_eq!(half_bits(103 << 23), 0x0001); // 2^-24, shift 10
}

/// Each code value of each channel alone, and grey: the buffer of all 1024
/// gives the same halves as converting each texel alone, alpha 1.0 on each.
#[test]
fn edge_buffer_matches_each_texel() {
    for shift in [0, 10, 20, 99] {
        let texels: Vec<u32> = (0..1024_u32)
            .map(|c| {
                if shift == 99 {
                    c * 0x0010_0401
                } else {
                    c << shift
                }
            })
            .collect();
        let src: Vec<u8> = texels.iter().flat_map(|t| t.to_ne_bytes()).collect();
        let mut dst = vec![0_u8; texels.len() * DST_TEXEL_BYTES];
        pq10_to_scrgb(&mut dst, &src);
        for (value, out) in texels.iter().zip(dst.as_chunks::<DST_TEXEL_BYTES>().0) {
            let mut one = [0_u8; DST_TEXEL_BYTES];
            pq10_to_scrgb(&mut one, &value.to_ne_bytes());
            assert_eq!(*out, one, "texel {value:#010x}");
            assert_eq!(u16::from_ne_bytes([out[6], out[7]]), ALPHA_ONE);
        }
    }
}

/// A `dst` shorter than its texels: only the whole texels it holds are
/// written, and the rest of it stays as it was.
#[test]
fn edge_short_dst() {
    let src: Vec<u8> = (0..4_u32)
        .flat_map(|t| texel(t * 100, 1, 2).to_ne_bytes())
        .collect();
    let mut full = [0_u8; 4 * DST_TEXEL_BYTES];
    pq10_to_scrgb(&mut full, &src);
    for len in 0..4 * DST_TEXEL_BYTES {
        let mut dst = [0xa5_u8; 4 * DST_TEXEL_BYTES];
        pq10_to_scrgb(&mut dst[..len], &src);
        let written = len / DST_TEXEL_BYTES * DST_TEXEL_BYTES;
        assert_eq!(dst[..written], full[..written], "len {len}");
        assert!(dst[written..].iter().all(|&b| b == 0xa5), "len {len}");
    }
}

/// A `src` whose length is not a whole number of texels: the tail is not
/// read, and its texel is not written.
#[test]
fn edge_src_tail() {
    let src: Vec<u8> = (0..3_u32)
        .flat_map(|t| texel(t * 300, 7, 9).to_ne_bytes())
        .collect();
    let mut full = [0_u8; 3 * DST_TEXEL_BYTES];
    pq10_to_scrgb(&mut full, &src);
    for len in 0..src.len() {
        let mut dst = [0xa5_u8; 3 * DST_TEXEL_BYTES];
        pq10_to_scrgb(&mut dst, &src[..len]);
        let written = len / SRC_TEXEL_BYTES * DST_TEXEL_BYTES;
        assert_eq!(dst[..written], full[..written], "len {len}");
        assert!(dst[written..].iter().all(|&b| b == 0xa5), "len {len}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(2048)))]

    #[test]
    fn edge_half_matches_model(bits in any::<u32>()) {
        prop_assert_eq!(half_bits(bits), model_half(bits));
    }

    #[test]
    fn edge_texel_high_bits_and_alpha(value in any::<u32>()) {
        let out = convert(value);
        prop_assert_eq!(out, convert(value & 0x3fff_ffff));
        prop_assert_eq!(out[3], ALPHA_ONE);
    }
}
