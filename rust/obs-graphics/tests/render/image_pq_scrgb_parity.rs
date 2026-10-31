//! Tier 3: `obs_graphics::image_pq_scrgb` against the original C
//! (`libobs/graphics/graphics-ffmpeg-host-pq.c.h`, through
//! `obs_c_oracle::image_pq_scrgb`), on all three sides: the oracle, the
//! C ABI shim (`gs_image_pq10_to_scrgb`) and the safe core. Every output
//! byte is compared exactly.
//!
//! - `pq_to_linear` over every 10-bit code value, bit for bit;
//! - `half_from_f32` over every `f32` bit pattern (one in ten below the
//!   `full` parity level), against `half_from_float()`;
//! - the conversion over each channel alone and grey at every code value,
//!   every pair of channels at every eighth code value, each value of bits
//!   30-31, and a proptest of random texels in buffers of 1 to 256.
//!
//! Every buffer is sized from its texel count with a checked multiply, and
//! the C always gets a `dst` of exactly 8 bytes per source texel, which is
//! what it writes.
//!
//! Intentional difference (the one in the core's docs): `half_from_float()`
//! shifts a `uint32_t` by 32 or more for a float below 2^-45 in magnitude
//! (biased exponent 81 or less), which C leaves undefined. Those floats are
//! not passed to the C; the Rust side gives zero with the float's sign.
//! A texel any of whose three scaled 709 values is such a float is compared
//! with the C on its other halves only, and its excluded halves are checked
//! against signed zero. The scaled values are computed here from the
//! oracle's `pq_to_linear` and the C's matrix, to decide only that.

use obs_c_oracle::image_pq_scrgb::{
    oracle_gs_image_pq10_to_scrgb, oracle_half_from_float, oracle_pq_to_linear,
};
use obs_graphics::ffi::image_pq_scrgb::gs_image_pq10_to_scrgb;
use obs_graphics::image_pq_scrgb::{
    DST_TEXEL_BYTES, SRC_TEXEL_BYTES, half_from_f32, pq_to_linear, pq10_to_scrgb,
};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// helpers

/// The float is one the C's 32-bit shift can't take: nonzero, with a biased
/// exponent of 81 or less.
fn c_shift_undefined(f: f32) -> bool {
    let magnitude = f.to_bits() & 0x7fff_ffff;
    magnitude != 0 && magnitude >> 23 <= 81
}

/// The three scaled 709 values a texel's halves come from, the C's way.
#[allow(
    clippy::excessive_precision,
    reason = "the C's literals, spelled as the C spells them"
)]
fn scaled_709(rgb10: u32) -> [f32; 3] {
    let code = |shift: u32| f32::from(u16::try_from((rgb10 >> shift) & 0x3ff).unwrap()) / 1023.0;
    // SAFETY: the oracle's wrapper takes and returns plain floats.
    let linear = |u: f32| unsafe { oracle_pq_to_linear(u) };
    let (r, g, b) = (linear(code(20)), linear(code(10)), linear(code(0)));
    [
        (1.660_491_002_108_434_5_f32 * r
            - 0.587_641_138_788_549_51_f32 * g
            - 0.072_849_863_319_884_883_f32 * b)
            * 125.0,
        (-0.124_550_474_521_590_74_f32 * r + 1.132_899_897_125_960_3_f32 * g
            - 0.008_349_422_604_369_476_8_f32 * b)
            * 125.0,
        (-0.018_150_763_354_905_303_f32 * r - 0.100_578_898_008_007_39_f32 * g
            + 1.118_729_661_362_912_7_f32 * b)
            * 125.0,
    ]
}

fn byte_sizes(texels: usize) -> (usize, usize) {
    (
        texels.checked_mul(DST_TEXEL_BYTES).unwrap(),
        texels.checked_mul(SRC_TEXEL_BYTES).unwrap(),
    )
}

/// Converts `texels` through the oracle, the shim and the core, and checks
/// the three agree on every byte, but for the halves the C can't make.
/// Returns how many halves were excluded.
fn check_all_sides(texels: &[u32]) -> usize {
    let (dst_len, src_len) = byte_sizes(texels.len());
    let src: Vec<u8> = texels.iter().flat_map(|t| t.to_ne_bytes()).collect();
    assert_eq!(src.len(), src_len);
    let dst_size = u32::try_from(dst_len).unwrap();
    let src_size = u32::try_from(src_len).unwrap();

    let mut oracle = vec![0xa5_u8; dst_len];
    let mut shim = vec![0x5a_u8; dst_len];
    let mut core = vec![0x3c_u8; dst_len];
    // SAFETY: both buffers have exactly the sizes passed, and the C writes
    // 8 bytes per 4-byte source texel, which is `dst_len`.
    unsafe {
        oracle_gs_image_pq10_to_scrgb(oracle.as_mut_ptr(), dst_size, src.as_ptr(), src_size);
        gs_image_pq10_to_scrgb(shim.as_mut_ptr(), dst_size, src.as_ptr(), src_size);
    }
    pq10_to_scrgb(&mut core, &src);
    assert_eq!(shim, core, "shim and core differ");

    let mut excluded = 0;
    for (i, &value) in texels.iter().enumerate() {
        let scaled = scaled_709(value);
        let range = i * DST_TEXEL_BYTES..(i + 1) * DST_TEXEL_BYTES;
        let (want, got) = (&oracle[range.clone()], &core[range]);
        for half in 0..4 {
            let bytes = half * 2..half * 2 + 2;
            // The alpha half has no scaled value.
            if let Some(tiny) = scaled.get(half).copied().filter(|&v| c_shift_undefined(v)) {
                excluded += 1;
                let zero = if tiny.is_sign_negative() {
                    0x8000_u16
                } else {
                    0
                };
                assert_eq!(
                    u16::from_ne_bytes([got[bytes.start], got[bytes.start + 1]]),
                    zero,
                    "texel {value:#010x} half {half}: tiny value not signed zero"
                );
                continue;
            }
            assert_eq!(
                got[bytes.clone()],
                want[bytes],
                "texel {value:#010x} half {half}"
            );
        }
    }
    excluded
}

// ---------------------------------------------------------------------------
// exhaustive

#[test]
fn parity_pq_to_linear_every_code_value() {
    for code in 0..1024_u16 {
        let u = f32::from(code) / 1023.0;
        // SAFETY: a plain float in and out.
        let want = unsafe { oracle_pq_to_linear(u) };
        assert_eq!(pq_to_linear(u).to_bits(), want.to_bits(), "code {code}");
    }
}

/// Every `f32` bit pattern, split across threads: one in ten below the
/// `full` parity level, all 2^32 at `full`. The patterns the C can't take
/// are checked against signed zero instead.
#[test]
fn parity_half_every_float() {
    const CHUNKS: u64 = 64;
    let span = (1_u64 << 32) / CHUNKS;
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..CHUNKS)
            .map(|c| {
                s.spawn(move || {
                    for bits in obs_c_oracle::parity::sweep(c * span..(c + 1) * span) {
                        let f = f32::from_bits(u32::try_from(bits).unwrap());
                        let got = half_from_f32(f);
                        if c_shift_undefined(f) {
                            let zero = if f.is_sign_negative() { 0x8000 } else { 0 };
                            assert_eq!(got, zero, "float {bits:#010x}");
                        } else {
                            // SAFETY: a float the C's shift is defined for.
                            let want = unsafe { oracle_half_from_float(f) };
                            assert_eq!(got, want, "float {bits:#010x}");
                        }
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    });
}

/// The edges of every exponent, on every run.
#[test]
fn parity_half_edges() {
    for exponent in 0..=0xff_u32 {
        for mantissa in [
            0,
            1,
            0x0fff,
            0x1000,
            0x1001,
            0x2000,
            0x3000,
            0x0040_0000,
            0x007f_ffff,
        ] {
            for sign in [0, 0x8000_0000_u32] {
                let f = f32::from_bits(sign | exponent << 23 | mantissa);
                if c_shift_undefined(f) {
                    continue;
                }
                // SAFETY: a float the C's shift is defined for.
                let want = unsafe { oracle_half_from_float(f) };
                assert_eq!(half_from_f32(f), want, "float {:#010x}", f.to_bits());
            }
        }
    }
}

#[test]
fn parity_each_channel_and_grey() {
    for shift in [0, 10, 20] {
        let texels: Vec<u32> = (0..1024_u32).map(|c| c << shift).collect();
        check_all_sides(&texels);
    }
    let grey: Vec<u32> = (0..1024_u32).map(|c| c * 0x0010_0401).collect();
    check_all_sides(&grey);
}

/// Every pair of channels at every eighth code value, the third at 0 and
/// at full scale: 128 x 128 x 2 texels per pair.
#[test]
fn parity_channel_pairs() {
    for (a, b, c) in [(20, 10, 0), (20, 0, 10), (10, 0, 20)] {
        let mut texels = Vec::new();
        for x in (0..1024_u32).step_by(8) {
            for y in (0..1024_u32).step_by(8) {
                for z in [0, 1023_u32] {
                    texels.push(x << a | y << b | z << c);
                }
            }
        }
        check_all_sides(&texels);
    }
}

#[test]
fn parity_high_bits() {
    let texels: Vec<u32> = (0..4096_u32)
        .map(|i| ((i & 0x3ff) * 0x0010_0401) | ((i >> 10) << 30))
        .collect();
    check_all_sides(&texels);
}

#[test]
fn parity_empty() {
    assert_eq!(check_all_sides(&[]), 0);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn parity_random_texels(texels in proptest::collection::vec(any::<u32>(), 1..=256)) {
        check_all_sides(&texels);
    }
}
