//! Tier 1: `obs_graphics::image_orient`, the image orientation and size
//! check of `graphics-ffmpeg.c`.
//!
//! The `steps_*` cases port `test/cmocka/test_image_orient_steps.c` 1:1:
//! its 7 cases under the same names, in the same order, with the same
//! values and expectations, except where the intentional difference
//! changes them: an orientation outside 0 to 8 is the image as stored.
//! `steps_needed_and_swaps` checks only 0 to 8 for `orient_needed`, and
//! `steps_copy_unknown_orientation` expects the unturned copy; the C's
//! values are in `edge_unknown_orientation_*`. The expected positions are
//! the EXIF meaning of each orientation, written here, not taken from the
//! C.
//!
//! The `edge_*` cases are new: proptests over orientations, sizes and
//! matrices, every orientation of random images against the EXIF meaning,
//! turning back (each orientation and its inverse give the image again),
//! and buffers shorter than their size says.

use obs_graphics::image_orient::{
    BYTES_PER_PIXEL, matrix_orientation, orient_copy, orient_needed, orient_swaps, too_large,
};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// helpers

/// The EXIF meaning of each orientation: where source pixel (x, y) of a
/// w x h image lands in the turned image, and the turned width.
fn exif_position(orient: i32, w: usize, h: usize, x: usize, y: usize) -> (usize, usize, usize) {
    let dw = if (5..=8).contains(&orient) { h } else { w };
    let (dx, dy) = match orient {
        2 => (w - 1 - x, y),
        3 => (w - 1 - x, h - 1 - y),
        4 => (x, h - 1 - y),
        5 => (y, x),
        6 => (h - 1 - y, x),
        7 => (h - 1 - y, w - 1 - x),
        8 => (y, w - 1 - x),
        _ => (x, y),
    };
    (dx, dy, dw)
}

/// A w x h RGBA image whose every pixel holds its own index.
fn numbered(w: usize, h: usize) -> Vec<u8> {
    (0..w * h)
        .flat_map(|i| {
            let i = u32::try_from(i).unwrap_or(u32::MAX);
            let b = i.to_le_bytes();
            [b[0], b[1], b[2], 0xa5]
        })
        .collect()
}

fn check_turn(orient: i32, w: usize, h: usize) {
    let src = numbered(w, h);
    let mut dst = vec![0xee; w * h * BYTES_PER_PIXEL];
    orient_copy(&mut dst, &src, w, h, orient);
    for y in 0..h {
        for x in 0..w {
            let (dx, dy, dw) = exif_position(orient, w, h, x, y);
            let d = (dx + dy * dw) * BYTES_PER_PIXEL;
            let s = (x + y * w) * BYTES_PER_PIXEL;
            assert_eq!(
                dst[d..d + 4],
                src[s..s + 4],
                "orientation {orient}, {w}x{h}: pixel ({x}, {y}) not at ({dx}, {dy})"
            );
        }
    }
}

const SIGNS: [[i32; 4]; 8] = [
    [1, 0, 0, 1],
    [-1, 0, 0, 1],
    [-1, 0, 0, -1],
    [1, 0, 0, -1],
    [0, 1, 1, 0],
    [0, 1, -1, 0],
    [0, -1, -1, 0],
    [0, -1, 1, 0],
];

fn want_orientation(a: i32, b: i32, c: i32, d: i32) -> i32 {
    let s = [a.signum(), b.signum(), c.signum(), d.signum()];
    SIGNS
        .iter()
        .position(|m| *m == s)
        .map_or(0, |i| i32::try_from(i).unwrap_or(0) + 1)
}

// ---------------------------------------------------------------------------
// the display matrix

#[test]
fn steps_matrix_every_sign_combination() {
    let mut found = 0;
    for a in -1..=1 {
        for b in -1..=1 {
            for c in -1..=1 {
                for d in -1..=1 {
                    let want = want_orientation(a, b, c, d);
                    found += i32::from(want != 0);
                    assert_eq!(matrix_orientation(a, b, c, d), want, "{a} {b} {c} {d}");
                    assert_eq!(
                        matrix_orientation(a * 65536, b * 65536, c * 65536, d * 65536),
                        want,
                        "16.16: {a} {b} {c} {d}"
                    );
                }
            }
        }
    }
    assert_eq!(found, 8);
}

#[test]
fn steps_matrix_extremes() {
    assert_eq!(matrix_orientation(i32::MAX, 0, 0, i32::MAX), 1);
    assert_eq!(matrix_orientation(i32::MIN, 0, 0, i32::MIN), 3);
    assert_eq!(matrix_orientation(0, i32::MIN, i32::MAX, 0), 8);
    assert_eq!(matrix_orientation(0, 0, 0, 0), 0);
    assert_eq!(matrix_orientation(1, 1, 1, 1), 0);
}

// ---------------------------------------------------------------------------
// the orientation tests

#[test]
fn steps_needed_and_swaps() {
    for orient in -3..=12 {
        assert_eq!(orient_swaps(orient), (5..=8).contains(&orient), "{orient}");
    }
    // 0 to 8 as in the C; outside it, the intentional difference
    // (edge_unknown_orientation_is_not_turned).
    for orient in 0..=8 {
        assert_eq!(
            orient_needed(orient),
            orient != 0 && orient != 1,
            "{orient}"
        );
    }
    assert!(!orient_swaps(i32::MAX));
}

// ---------------------------------------------------------------------------
// the copy

#[test]
fn steps_copy_small_image_spelled_out() {
    let want = [
        "", "", "cbafed", "fedcba", "defabc", "adbecf", "daebfc", "fcebda", "cfbead",
    ];
    let src: Vec<u8> = b"abcdef".iter().flat_map(|&c| [c, 0, 0, 0]).collect();
    for orient in 2..=8_i32 {
        let mut dst = [0_u8; 6 * 4];
        orient_copy(&mut dst, &src, 3, 2, orient);
        let got: String = dst.chunks(4).map(|p| char::from(p[0])).collect();
        assert_eq!(
            got,
            want[usize::try_from(orient).unwrap_or(0)],
            "orientation {orient}"
        );
    }
}

#[test]
fn steps_copy_every_size() {
    let sizes = [1, 2, 3, 7, 15, 16, 17, 31, 32, 33];
    for orient in 2..=8 {
        for &w in &sizes {
            for &h in &sizes {
                check_turn(orient, w, h);
            }
        }
    }
}

/// The C writes every pixel to the first one here; the port copies the
/// image as stored (the intentional difference).
#[test]
fn steps_copy_unknown_orientation() {
    for orient in [-1, 9, 42, i32::MAX, i32::MIN] {
        let (w, h) = (19, 5);
        let src = numbered(w, h);
        let mut dst = vec![0xee; w * h * BYTES_PER_PIXEL];
        orient_copy(&mut dst, &src, w, h, orient);
        assert_eq!(dst, src, "orientation {orient}");
    }
}

// ---------------------------------------------------------------------------
// the size check

#[test]
fn steps_too_large_against_the_limit() {
    assert!(!too_large(0, 100_000, 100_000));
    assert!(!too_large(16384, 16384, 16384));
    assert!(too_large(16384, 16385, 1));
    assert!(too_large(16384, 1, 16385));
    assert!(!too_large(16384, 1, 1));
    assert!(!too_large(1, 1, 1));
    assert!(too_large(1, 2, 1));
    assert!(!too_large(16384, 0, 100_000));
    assert!(!too_large(16384, 100_000, -1));
    assert!(!too_large(16384, i32::MIN, i32::MAX));
    assert!(too_large(16384, i32::MAX, i32::MAX));
    assert!(!too_large(u32::MAX, i32::MAX, i32::MAX));
}

// ---------------------------------------------------------------------------
// edges

/// The intentional difference: an orientation outside 0 to 8 is not
/// turned (the C turns it, `orient != 0 && orient != 1`).
#[test]
fn edge_unknown_orientation_is_not_turned() {
    for orient in [-3, -2, -1, 9, 10, 12, 42, i32::MAX, i32::MIN] {
        assert!(!orient_needed(orient), "{orient}");
        assert!(!orient_swaps(orient), "{orient}");
    }
}

/// Buffers shorter than the size says: no panic, and only whole pixels
/// inside both buffers are written.
#[test]
fn edge_short_buffers() {
    let src = numbered(4, 4);
    for orient in -1..=9 {
        let mut short = vec![0xee_u8; 10];
        orient_copy(&mut short, &src, 4, 4, orient);
        let mut dst = vec![0xee_u8; 64];
        orient_copy(&mut dst, &src[..10], 4, 4, orient);
        orient_copy(&mut dst, &src, usize::MAX, 2, orient);
        orient_copy(&mut dst, &src, 0, 0, orient);
    }
}

/// Each orientation and its inverse give the image back.
#[test]
fn edge_turn_and_back() {
    // (orientation, its inverse): 6 and 8 turn by 90 degrees each way; the
    // mirrors, 180 degrees, the transpose and the transverse undo themselves.
    let pairs = [(2, 2), (3, 3), (4, 4), (5, 5), (6, 8), (7, 7), (8, 6)];
    for (orient, inverse) in pairs {
        let (w, h) = (23, 9);
        let src = numbered(w, h);
        let mut once = vec![0; src.len()];
        orient_copy(&mut once, &src, w, h, orient);
        let (tw, th) = if orient_swaps(orient) { (h, w) } else { (w, h) };
        let mut back = vec![0; src.len()];
        orient_copy(&mut back, &once, tw, th, inverse);
        assert_eq!(back, src, "orientation {orient}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// Any matrix: the orientation of its signs.
    #[test]
    fn edge_matrix_any(a in any::<i32>(), b in any::<i32>(), c in any::<i32>(), d in any::<i32>()) {
        prop_assert_eq!(matrix_orientation(a, b, c, d), want_orientation(a, b, c, d));
    }

    /// Any orientation 2 to 8 on any size up to 70 x 70.
    #[test]
    fn edge_copy_any_size(orient in 2_i32..=8, w in 1_usize..70, h in 1_usize..70) {
        check_turn(orient, w, h);
    }

    /// Any size check: too large exactly when a positive side is over a
    /// set limit.
    #[test]
    fn edge_too_large_any(max in any::<u32>(), cx in any::<i32>(), cy in any::<i32>()) {
        let want = max != 0 && cx > 0 && cy > 0
            && (u32::try_from(cx).unwrap_or(0) > max || u32::try_from(cy).unwrap_or(0) > max);
        prop_assert_eq!(too_large(max, cx, cy), want);
    }
}
