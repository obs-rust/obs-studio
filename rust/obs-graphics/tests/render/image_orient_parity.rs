//! Tier 3 parity: the shims of `obs_graphics::ffi::image_orient` (and the
//! safe core under them) against the original
//! `libobs/graphics/graphics-ffmpeg-orient.c.h`, compiled as
//! `oracle_gs_ffmpeg_image_*` (obs-c-oracle, oracle/image_orient.c).
//!
//! Every call runs the shim, the C and the safe core, and compares the
//! results exactly (the turned image byte for byte).
//!
//! Runs: every sign combination of the display matrix and the `i32` edges;
//! the orientation tests over -3 to 12 and the extremes; every orientation
//! 2 to 8 on every image size from 1 x 1 to 40 x 40; and proptests over
//! matrices, sizes up to 300 x 300 and the size check.
//!
//! An orientation outside 0 to 8 is the intentional difference (the image
//! as stored; the C writes every pixel to the first one). There the shim is
//! compared with the core only. `test_image_orient_steps` runs the C test
//! against the C; the real-libobs image tests run against the hybrid
//! libobs, where these shims replace the C.

use core::ffi::c_int;

use obs_c_oracle::image_orient::{
    oracle_gs_ffmpeg_image_matrix_orientation, oracle_gs_ffmpeg_image_orient_copy,
    oracle_gs_ffmpeg_image_orient_needed, oracle_gs_ffmpeg_image_orient_swaps,
    oracle_gs_ffmpeg_image_too_large,
};
use obs_graphics::ffi::image_orient::{
    gs_ffmpeg_image_matrix_orientation, gs_ffmpeg_image_orient_copy, gs_ffmpeg_image_orient_needed,
    gs_ffmpeg_image_orient_swaps, gs_ffmpeg_image_too_large,
};
use obs_graphics::image_orient::{self as orient_core, BYTES_PER_PIXEL};
use proptest::prelude::*;

const EDGES: [i32; 7] = [i32::MIN, -65536, -1, 0, 1, 65536, i32::MAX];

// ---------------------------------------------------------------------------
// one call on all three sides

fn check_matrix(a: i32, b: i32, c: i32, d: i32) {
    let shim = gs_ffmpeg_image_matrix_orientation(a, b, c, d);
    // SAFETY: plain values; the oracle keeps no state.
    let oracle = unsafe { oracle_gs_ffmpeg_image_matrix_orientation(a, b, c, d) };
    let core = orient_core::matrix_orientation(a, b, c, d);
    assert_eq!(shim, oracle, "shim vs C: {a} {b} {c} {d}");
    assert_eq!(shim, core, "shim vs core: {a} {b} {c} {d}");
}

fn check_tests(orient: c_int) {
    let needed = gs_ffmpeg_image_orient_needed(orient);
    let swaps = gs_ffmpeg_image_orient_swaps(orient);
    assert_eq!(needed, orient_core::orient_needed(orient), "{orient}");
    assert_eq!(swaps, orient_core::orient_swaps(orient), "{orient}");
    if (0..=8).contains(&orient) {
        // SAFETY: plain values.
        let (c_needed, c_swaps) = unsafe {
            (
                oracle_gs_ffmpeg_image_orient_needed(orient),
                oracle_gs_ffmpeg_image_orient_swaps(orient),
            )
        };
        assert_eq!(needed, c_needed, "shim vs C needed: {orient}");
        assert_eq!(swaps, c_swaps, "shim vs C swaps: {orient}");
    } else {
        // The intentional difference: not turned.
        assert!(!needed, "{orient}");
    }
}

fn image(w: usize, h: usize, seed: u32) -> Vec<u8> {
    let mut x = seed | 1;
    (0..w * h * BYTES_PER_PIXEL)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x.to_le_bytes()[0]
        })
        .collect()
}

fn check_copy(w: usize, h: usize, orient: c_int, seed: u32) {
    let src = image(w, h, seed);
    let len = src.len();
    let mut shim = vec![0xee_u8; len];
    // SAFETY: two buffers of w * h * 4 bytes, not overlapping.
    unsafe { gs_ffmpeg_image_orient_copy(shim.as_mut_ptr(), src.as_ptr(), w, h, orient) };
    let mut core = vec![0xee_u8; len];
    orient_core::orient_copy(&mut core, &src, w, h, orient);
    assert_eq!(shim, core, "shim vs core: {w}x{h} orientation {orient}");
    if (2..=8).contains(&orient) {
        let mut c = vec![0xee_u8; len];
        // SAFETY: as above; the C reads and writes exactly that much.
        unsafe { oracle_gs_ffmpeg_image_orient_copy(c.as_mut_ptr(), src.as_ptr(), w, h, orient) };
        assert_eq!(shim, c, "shim vs C: {w}x{h} orientation {orient}");
    } else {
        assert_eq!(shim, src, "as stored: {w}x{h} orientation {orient}");
    }
}

fn check_too_large(max: u32, cx: c_int, cy: c_int) {
    let shim = gs_ffmpeg_image_too_large(max, cx, cy);
    // SAFETY: plain values.
    let c = unsafe { oracle_gs_ffmpeg_image_too_large(max, cx, cy) };
    assert_eq!(shim, c, "shim vs C: {max} {cx} {cy}");
    assert_eq!(
        shim,
        orient_core::too_large(max, cx, cy),
        "shim vs core: {max} {cx} {cy}"
    );
}

// ---------------------------------------------------------------------------
// exhaustive over the edges

#[test]
fn parity_matrix_exhaustive() {
    for a in EDGES {
        for b in EDGES {
            for c in EDGES {
                for d in EDGES {
                    check_matrix(a, b, c, d);
                }
            }
        }
    }
}

#[test]
fn parity_orientation_tests() {
    for orient in (-3..=12).chain([c_int::MIN, c_int::MAX]) {
        check_tests(orient);
    }
}

#[test]
fn parity_copy_every_small_size() {
    for orient in -1..=9 {
        for w in 1..=40 {
            for h in 1..=40 {
                check_copy(w, h, orient, u32::try_from(w * 64 + h).unwrap_or(1));
            }
        }
    }
}

#[test]
fn parity_copy_null_and_overflow() {
    let mut dst = [0xee_u8; 16];
    let src = [1_u8; 16];
    // SAFETY: the shim checks the null pointers and the size first.
    unsafe {
        gs_ffmpeg_image_orient_copy(std::ptr::null_mut(), src.as_ptr(), 2, 2, 6);
        gs_ffmpeg_image_orient_copy(dst.as_mut_ptr(), std::ptr::null(), 2, 2, 6);
        gs_ffmpeg_image_orient_copy(dst.as_mut_ptr(), src.as_ptr(), usize::MAX, 2, 6);
        gs_ffmpeg_image_orient_copy(dst.as_mut_ptr(), src.as_ptr(), 0, 0, 6);
    }
    assert_eq!(dst, [0xee_u8; 16]);
}

#[test]
fn parity_too_large_exhaustive() {
    let maxes = [0, 1, 2, 4096, 16384, 0x7fff_ffff, 0x8000_0000, u32::MAX];
    let dims = [
        c_int::MIN,
        -1,
        0,
        1,
        2,
        4095,
        4096,
        4097,
        16384,
        16385,
        c_int::MAX,
    ];
    for max in maxes {
        for cx in dims {
            for cy in dims {
                check_too_large(max, cx, cy);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(1024)))]

    #[test]
    fn parity_matrix_any(a in any::<i32>(), b in any::<i32>(), c in any::<i32>(), d in any::<i32>()) {
        check_matrix(a, b, c, d);
    }

    #[test]
    fn parity_copy_any(w in 1_usize..300, h in 1_usize..300, orient in -2_i32..11, seed in any::<u32>()) {
        check_copy(w, h, orient, seed);
    }

    #[test]
    fn parity_too_large_any(max in any::<u32>(), cx in any::<i32>(), cy in any::<i32>()) {
        check_too_large(max, cx, cy);
    }
}
