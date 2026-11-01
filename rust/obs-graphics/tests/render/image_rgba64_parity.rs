//! Tier 3 parity: the shim of `obs_graphics::ffi::image_rgba64` (and the
//! safe core under it) against the original
//! `libobs/graphics/graphics-ffmpeg-rgba64.c.h`, compiled as
//! `oracle_gs_ffmpeg_image_rgba64be_rows` (obs-c-oracle,
//! oracle/image_rgba64.c, built with `-ffp-contract=off`).
//!
//! Every frame runs on the shim, the C and the safe core, each into an
//! output filled with the same canary, and the three outputs are compared
//! byte for byte, so a byte one side leaves unwritten counts too.
//!
//! Runs: every 16-bit value in each colour channel at alphas 0, 1, 32768,
//! 33025 and 65535; every 16-bit alpha under greys 0, 1, 257, 32768, 33025
//! and 65535; both in each alpha mode; 24 grey and alpha pairs where the
//! sRGB curve's last bit decides the byte. Every width from 1 to 20 at 1 to 6
//! rows with padded, tight and short line sizes in each mode; alpha modes
//! outside the three; and a proptest over frames up to 64 x 64 with random
//! channels, padding or a short line size, and any mode from -1 to 3.
//!
//! No intentional difference. Left out, as the C reads or writes out of
//! bounds there and `graphics-ffmpeg.c` never passes them: a null pointer,
//! a negative width and a size that overflows, where the shim writes
//! nothing (checked here on the shim alone).
//! `test_graphics_ffmpeg_rgba64_steps` runs the C test against the C; the
//! real-libobs image tests run against the hybrid libobs, where this shim
//! replaces the C.

use obs_c_oracle::image_rgba64::oracle_gs_ffmpeg_image_rgba64be_rows;
use obs_graphics::ffi::image_rgba64::gs_ffmpeg_image_rgba64be_rows;
use obs_graphics::image_rgba64::{alpha_mode, rgba64be_rows, src_len};
use proptest::prelude::*;

const CANARY: u8 = 0xa5;
const MODES: [i32; 3] = [
    alpha_mode::STRAIGHT,
    alpha_mode::PREMULTIPLY_SRGB,
    alpha_mode::PREMULTIPLY,
];

fn to_i32(n: usize) -> i32 {
    i32::try_from(n).unwrap()
}

fn same(what: &str, got: &[u8], want: &[u8]) {
    if let Some(i) = got.iter().zip(want).position(|(a, b)| a != b) {
        panic!(
            "{what}: byte {i} (texel {}, channel {}) is {}, expected {}",
            i / 4,
            i % 4,
            got[i],
            want[i]
        );
    }
    assert_eq!(got.len(), want.len(), "{what}: length");
}

/// One frame on all three sides; the shim's output.
fn check(what: &str, src: &[u8], cx: usize, cy: usize, linesize: usize, mode: i32) -> Vec<u8> {
    let (cx, cy) = (to_i32(cx), to_i32(cy));
    let len = usize::try_from(cx).unwrap() * usize::try_from(cy).unwrap() * 4;
    assert!(
        src.len() >= src_len(cx, cy, linesize).unwrap(),
        "{what}: source too short"
    );
    let mut shim = vec![CANARY; len];
    let mut c = vec![CANARY; len];
    let mut core = vec![CANARY; len];
    // SAFETY: `len` writable bytes in each output and at least `src_len()`
    // readable bytes in `src`, which is all either side touches.
    unsafe {
        gs_ffmpeg_image_rgba64be_rows(shim.as_mut_ptr(), src.as_ptr(), cx, cy, linesize, mode);
        oracle_gs_ffmpeg_image_rgba64be_rows(c.as_mut_ptr(), src.as_ptr(), cx, cy, linesize, mode);
    }
    rgba64be_rows(&mut core, src, cx, cy, linesize, mode);
    same(&format!("{what}: shim vs C"), &shim, &c);
    same(&format!("{what}: core vs C"), &core, &c);
    shim
}

fn put(src: &mut [u8], at: usize, rgba: [u16; 4]) {
    for (c, v) in rgba.into_iter().enumerate() {
        src[at + 2 * c..at + 2 * c + 2].copy_from_slice(&v.to_be_bytes());
    }
}

/// A 256 x 256 frame (65536 texels, tight) whose texel `i` is `f(i)`.
fn frame_of(f: impl Fn(u16) -> [u16; 4]) -> Vec<u8> {
    let mut src = vec![0u8; 65536 * 8];
    for i in 0..=u16::MAX {
        put(&mut src, usize::from(i) * 8, f(i));
    }
    src
}

// ---------------------------------------------------------------------------
// exhaustive

/// Every 16-bit value in each colour channel (red counts up, green steps
/// through them by 7919, blue counts down), at the alpha edges.
#[test]
fn every_channel_value() {
    for a in [0, 1, 32768, 33025, 65535] {
        let src = frame_of(|i| [i, i.wrapping_mul(7919), u16::MAX - i, a]);
        for mode in MODES {
            check(
                &format!("alpha {a}, mode {mode}"),
                &src,
                256,
                256,
                2048,
                mode,
            );
        }
    }
}

/// Every 16-bit alpha, under the grey edges.
#[test]
fn every_alpha() {
    for v in [0, 1, 257, 32768, 33025, 65535] {
        let src = frame_of(|i| [v, v, v, i]);
        for mode in MODES {
            check(
                &format!("grey {v}, mode {mode}"),
                &src,
                256,
                256,
                2048,
                mode,
            );
        }
    }
}

/// (grey, alpha) pairs where the sRGB path's last bit decides the byte:
/// a sample of the 5,359 pairs (of all 2^32) where `(u + 0.055) / 1.055`
/// computed as a multiplication by `1 / 1.055` changes the output. The
/// sweeps above put every value through the curve, but only at a few
/// alphas.
const SRGB_LAST_BIT: [(u16, u16); 24] = [
    (2859, 61697),
    (11926, 63205),
    (17795, 51852),
    (21455, 56296),
    (25417, 24172),
    (28810, 46706),
    (31636, 64852),
    (34519, 64674),
    (36960, 53928),
    (39404, 34782),
    (41660, 7041),
    (43691, 43759),
    (45762, 17018),
    (47741, 13216),
    (49711, 29389),
    (51407, 41263),
    (53348, 35438),
    (55168, 47808),
    (56748, 44324),
    (58288, 32815),
    (60027, 57538),
    (61460, 33967),
    (62861, 55615),
    (64268, 20742),
];

#[test]
fn srgb_last_bit_pairs() {
    let mut src = vec![0u8; SRGB_LAST_BIT.len() * 8];
    for (i, (v, a)) in SRGB_LAST_BIT.into_iter().enumerate() {
        put(&mut src, i * 8, [v, v, v, a]);
    }
    for mode in MODES {
        check(
            &format!("mode {mode}"),
            &src,
            SRGB_LAST_BIT.len(),
            1,
            src.len(),
            mode,
        );
    }
}

// ---------------------------------------------------------------------------
// layouts

fn noise(len: usize, seed: u64) -> Vec<u8> {
    let mut x = seed | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x.to_le_bytes()[3]
        })
        .collect()
}

/// Every width from 1 to 20 at 1 to 6 rows, with the line size tight,
/// padded (1, 7, 8 and 13 bytes) or short (a byte, a texel, all but one
/// texel, and under one texel), in each mode.
#[test]
fn every_small_layout() {
    for cx in 1..=20usize {
        for cy in 1..=6usize {
            let full = cx * 8;
            let mut linesizes = vec![full, full + 1, full + 7, full + 8, full + 13, full - 1, 7];
            if cx > 1 {
                linesizes.extend([full - 8, 8]);
            }
            for linesize in linesizes {
                let src = noise(linesize * cy, (cx * 131 + cy * 7 + linesize) as u64);
                for mode in MODES {
                    check(
                        &format!("{cx} x {cy}, line {linesize}, mode {mode}"),
                        &src,
                        cx,
                        cy,
                        linesize,
                        mode,
                    );
                }
            }
        }
    }
}

/// An alpha mode outside the three: nothing written on any side.
#[test]
fn unknown_alpha_modes() {
    let src = noise(5 * 3 * 8, 9);
    for mode in [-1, 3, 4, 100, i32::MIN, i32::MAX] {
        let out = check(&format!("mode {mode}"), &src, 5, 3, 40, mode);
        assert!(out.iter().all(|b| *b == CANARY), "mode {mode} wrote");
    }
}

// ---------------------------------------------------------------------------
// outside the C's contract: the shim alone

#[test]
fn shim_writes_nothing_outside_the_contract() {
    let src = noise(4 * 4 * 8, 3);
    let mut dst = vec![CANARY; 64];
    // SAFETY: the shim touches nothing for these: a null pointer, a
    // negative width, a size that overflows. The buffers are valid anyway.
    unsafe {
        gs_ffmpeg_image_rgba64be_rows(core::ptr::null_mut(), src.as_ptr(), 4, 4, 32, 0);
        gs_ffmpeg_image_rgba64be_rows(dst.as_mut_ptr(), core::ptr::null(), 4, 4, 32, 0);
        gs_ffmpeg_image_rgba64be_rows(dst.as_mut_ptr(), src.as_ptr(), -1, 4, 32, 0);
        gs_ffmpeg_image_rgba64be_rows(dst.as_mut_ptr(), src.as_ptr(), i32::MIN, 1, 32, 0);
        gs_ffmpeg_image_rgba64be_rows(
            dst.as_mut_ptr(),
            src.as_ptr(),
            i32::MAX,
            i32::MAX,
            usize::MAX,
            0,
        );
        gs_ffmpeg_image_rgba64be_rows(dst.as_mut_ptr(), src.as_ptr(), 4, -4, 32, 0);
        gs_ffmpeg_image_rgba64be_rows(dst.as_mut_ptr(), src.as_ptr(), 4, 0, 32, 0);
        gs_ffmpeg_image_rgba64be_rows(dst.as_mut_ptr(), src.as_ptr(), 0, 4, 32, 0);
    }
    assert!(dst.iter().all(|b| *b == CANARY));
}

// ---------------------------------------------------------------------------
// random frames

#[derive(Debug, Clone)]
struct Frame {
    cx: usize,
    cy: usize,
    linesize: usize,
    src: Vec<u8>,
    mode: i32,
}

fn frames() -> impl Strategy<Value = Frame> {
    (
        1usize..=64,
        1usize..=64,
        0usize..=64,
        any::<bool>(),
        -1i32..4,
    )
        .prop_flat_map(|(cx, cy, extra, short, mode)| {
            // A short line size is under cx * 8 (and at least 1); a padded
            // one is cx * 8 plus 0 to 64 bytes.
            let linesize = if short && cx * 8 > 1 {
                1 + extra % (cx * 8 - 1)
            } else {
                cx * 8 + extra
            };
            (
                Just(cx),
                Just(cy),
                Just(linesize),
                prop::collection::vec(any::<u8>(), linesize * cy),
                Just(mode),
            )
        })
        .prop_map(|(cx, cy, linesize, src, mode)| Frame {
            cx,
            cy,
            linesize,
            src,
            mode,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn random_frames(f in frames()) {
        check(
            &format!("{} x {}, line {}, mode {}", f.cx, f.cy, f.linesize, f.mode),
            &f.src,
            f.cx,
            f.cy,
            f.linesize,
            f.mode,
        );
    }
}
