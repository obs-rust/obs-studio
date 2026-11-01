//! Tier 1: `obs_graphics::image_route`, the pixel route of
//! `graphics-ffmpeg.c`.
//!
//! The `steps_*` cases port `test/cmocka/test_graphics_ffmpeg_route_steps.c`
//! 1:1: its 10 cases under the same names, in the same order, with the same
//! values and expectations, the characterized-not-endorsed one included (a
//! short line size leaves the end of each row unwritten). No intentional
//! difference.
//!
//! The `edge_*` cases are new: every format value from -1000 to 1000 in the
//! named modes, the YUVJ remap against the plan, the copy's source length,
//! buffers shorter than the image (the copy stops at a whole row, nothing
//! panics), sizes at the end of `usize`, and proptests: the copy against a
//! per-byte model over any geometry, and a padded source against the tight
//! one.

use obs_graphics::image_route::{
    Plan, Premultiply, Route, TEXEL_BYTES, alpha_mode, color_format, copy_rows, copy_src_len,
    gs_format, pix_fmt, plan, sws_format,
};
use proptest::prelude::*;

use alpha_mode::{PREMULTIPLY, PREMULTIPLY_SRGB, STRAIGHT};

const CANARY: u8 = 0xa5;

/// `AV_PIX_FMT_YUV420P10LE`: a format the route does not name.
const UNNAMED_PIX_FMT: i32 = 62;

const ALPHA_MODES: [i32; 3] = [STRAIGHT, PREMULTIPLY_SRGB, PREMULTIPLY];

/// What the swscale route and the premultiplied rows run for each mode.
const PREMULTIPLY_OF: [Premultiply; 3] =
    [Premultiply::None, Premultiply::Srgb, Premultiply::Linear];

const NAMED: [i32; 14] = [
    pix_fmt::YUV420P,
    pix_fmt::YUV422P,
    pix_fmt::YUV444P,
    pix_fmt::YUV411P,
    pix_fmt::YUVJ420P,
    pix_fmt::YUVJ422P,
    pix_fmt::YUVJ444P,
    pix_fmt::RGBA,
    pix_fmt::BGRA,
    pix_fmt::YUV440P,
    pix_fmt::YUVJ440P,
    pix_fmt::RGBA64BE,
    pix_fmt::BGR0,
    pix_fmt::YUVJ411P,
];

fn check_plan(
    fmt: i32,
    mode: i32,
    route: Route,
    premultiply: Premultiply,
    sws: i32,
    full_range: bool,
    out_format: i32,
) {
    assert_eq!(
        plan(fmt, mode),
        Plan {
            route,
            premultiply,
            sws_format: sws,
            full_range,
            out_format,
        },
        "format {fmt}, alpha mode {mode}"
    );
}

// ---------------------------------------------------------------------------
// test_graphics_ffmpeg_route_steps.c, 1:1

#[test]
fn steps_values() {
    assert_eq!(pix_fmt::YUV420P, 0);
    assert_eq!(pix_fmt::YUV422P, 4);
    assert_eq!(pix_fmt::YUV444P, 5);
    assert_eq!(pix_fmt::YUV411P, 7);
    assert_eq!(pix_fmt::YUVJ420P, 12);
    assert_eq!(pix_fmt::YUVJ422P, 13);
    assert_eq!(pix_fmt::YUVJ444P, 14);
    assert_eq!(pix_fmt::RGBA, 26);
    assert_eq!(pix_fmt::BGRA, 28);
    assert_eq!(pix_fmt::YUV440P, 31);
    assert_eq!(pix_fmt::YUVJ440P, 32);
    assert_eq!(pix_fmt::RGBA64BE, 104);
    assert_eq!(pix_fmt::BGR0, 121);
    assert_eq!(pix_fmt::YUVJ411P, 138);
    assert_eq!(gs_format::RGBA, 3);
    assert_eq!(gs_format::BGRX, 4);
    assert_eq!(gs_format::BGRA, 5);
    assert_eq!(gs_format::RGBA16, 7);
    assert_eq!(STRAIGHT, 0);
    assert_eq!(PREMULTIPLY_SRGB, 1);
    assert_eq!(PREMULTIPLY, 2);
}

#[test]
fn steps_copy_route() {
    for mode in ALPHA_MODES {
        check_plan(
            pix_fmt::BGR0,
            mode,
            Route::Copy,
            Premultiply::None,
            pix_fmt::BGR0,
            false,
            pix_fmt::BGR0,
        );
    }
    for fmt in [pix_fmt::RGBA, pix_fmt::BGRA] {
        check_plan(
            fmt,
            STRAIGHT,
            Route::Copy,
            Premultiply::None,
            fmt,
            false,
            fmt,
        );
    }
}

#[test]
fn steps_premultiply_rows_route() {
    for fmt in [pix_fmt::RGBA, pix_fmt::BGRA] {
        check_plan(
            fmt,
            PREMULTIPLY_SRGB,
            Route::PremultiplyRows,
            Premultiply::Srgb,
            fmt,
            false,
            fmt,
        );
        check_plan(
            fmt,
            PREMULTIPLY,
            Route::PremultiplyRows,
            Premultiply::Linear,
            fmt,
            false,
            fmt,
        );
    }
}

#[test]
fn steps_rgba64_route() {
    for mode in ALPHA_MODES {
        check_plan(
            pix_fmt::RGBA64BE,
            mode,
            Route::Rgba64,
            Premultiply::None,
            pix_fmt::RGBA64BE,
            false,
            pix_fmt::RGBA,
        );
    }
}

const SWS_PAIRS: [(i32, i32, bool); 12] = [
    (pix_fmt::YUVJ420P, pix_fmt::YUV420P, true),
    (pix_fmt::YUVJ422P, pix_fmt::YUV422P, true),
    (pix_fmt::YUVJ444P, pix_fmt::YUV444P, true),
    (pix_fmt::YUVJ440P, pix_fmt::YUV440P, true),
    (pix_fmt::YUVJ411P, pix_fmt::YUV411P, true),
    (pix_fmt::YUV420P, pix_fmt::YUV420P, false),
    (pix_fmt::YUV422P, pix_fmt::YUV422P, false),
    (pix_fmt::YUV444P, pix_fmt::YUV444P, false),
    (pix_fmt::YUV440P, pix_fmt::YUV440P, false),
    (pix_fmt::YUV411P, pix_fmt::YUV411P, false),
    (UNNAMED_PIX_FMT, UNNAMED_PIX_FMT, false),
    (-1, -1, false),
];

#[test]
fn steps_sws_route() {
    for (fmt, sws, full_range) in SWS_PAIRS {
        for (m, mode) in ALPHA_MODES.into_iter().enumerate() {
            check_plan(
                fmt,
                mode,
                Route::Sws,
                PREMULTIPLY_OF[m],
                sws,
                full_range,
                pix_fmt::BGRA,
            );
        }
    }
}

#[test]
fn steps_color_formats() {
    assert_eq!(color_format(pix_fmt::RGBA), gs_format::RGBA);
    assert_eq!(color_format(pix_fmt::BGRA), gs_format::BGRA);
    assert_eq!(color_format(pix_fmt::BGR0), gs_format::BGRX);
    assert_eq!(color_format(pix_fmt::RGBA64BE), gs_format::RGBA16);
    assert_eq!(color_format(pix_fmt::YUVJ420P), gs_format::BGRX);
    assert_eq!(color_format(pix_fmt::YUV420P), gs_format::BGRX);
    assert_eq!(color_format(UNNAMED_PIX_FMT), gs_format::BGRX);
    assert_eq!(color_format(-1), gs_format::BGRX);
}

const MAX_CX: usize = 9;
const MAX_CY: usize = 5;

fn src_byte(y: usize, x: usize) -> u8 {
    (y * 37 + x * 3 + 1) as u8
}

/// The C test's check_copy(): a cx x cy image from a source of the given
/// line size into a canary-filled buffer one image larger; each row has
/// min(cx * 4, linesize) bytes of its source line, the rest of the row and
/// everything after cx * cy * 4 the canary.
fn check_copy(cx: usize, cy: usize, linesize: usize) {
    let mut src = vec![0u8; linesize * cy];
    for y in 0..cy {
        for x in 0..linesize {
            src[y * linesize + x] = src_byte(y, x);
        }
    }
    let mut dst = vec![CANARY; 2 * MAX_CX * MAX_CY * 4];

    copy_rows(&mut dst, &src, cx, cy, linesize);

    let row = cx * 4;
    let copied = row.min(linesize);
    for (i, &b) in dst.iter().enumerate() {
        let (y, x) = (i / row, i % row);
        let expected = if y < cy && x < copied {
            src_byte(y, x)
        } else {
            CANARY
        };
        assert_eq!(b, expected, "{cx} x {cy}, line size {linesize}: byte {i}");
    }
}

#[test]
fn steps_copy_tight() {
    for cx in 1..=MAX_CX {
        for cy in 1..=MAX_CY {
            check_copy(cx, cy, cx * 4);
        }
    }
}

#[test]
fn steps_copy_padded() {
    for cx in 1..=MAX_CX {
        for cy in 1..=MAX_CY {
            for pad in [1, 4, 8, 24] {
                check_copy(cx, cy, cx * 4 + pad);
            }
        }
    }
}

/// Characterized, not endorsed: a short line size leaves the end of each
/// row unwritten.
#[test]
fn steps_copy_short_linesize() {
    check_copy(4, 3, 12);
    check_copy(4, 3, 1);
    check_copy(9, 5, 31);
    check_copy(2, 5, 7);
}

#[test]
fn steps_copy_no_rows() {
    check_copy(4, 0, 16);
    check_copy(4, 0, 20);
    check_copy(4, 0, 3);
}

// ---------------------------------------------------------------------------
// edge cases

/// Every unnamed format goes through swscale as itself, in every named
/// mode, and comes out BGRX.
#[test]
fn edge_unnamed_formats() {
    for fmt in (-1000..1000).filter(|f| !NAMED.contains(f)) {
        assert_eq!(sws_format(fmt), (fmt, false), "format {fmt}");
        assert_eq!(color_format(fmt), gs_format::BGRX, "format {fmt}");
        for (m, mode) in ALPHA_MODES.into_iter().enumerate() {
            check_plan(
                fmt,
                mode,
                Route::Sws,
                PREMULTIPLY_OF[m],
                fmt,
                false,
                pix_fmt::BGRA,
            );
        }
    }
    for fmt in [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX] {
        assert_eq!(plan(fmt, STRAIGHT).route, Route::Sws);
        assert_eq!(color_format(fmt), gs_format::BGRX);
    }
}

/// The plan carries [`sws_format`] for every route, and the texture format
/// of every route's output is the one the C picks for it.
#[test]
fn edge_plan_carries_sws_format() {
    for fmt in NAMED.into_iter().chain([-1, UNNAMED_PIX_FMT]) {
        for mode in ALPHA_MODES {
            let p = plan(fmt, mode);
            assert_eq!(
                (p.sws_format, p.full_range),
                sws_format(fmt),
                "format {fmt}"
            );
            let expected = match p.route {
                Route::Copy | Route::PremultiplyRows => color_format(fmt),
                Route::Rgba64 => gs_format::RGBA,
                Route::Sws => gs_format::BGRA,
            };
            assert_eq!(color_format(p.out_format), expected, "format {fmt}");
        }
    }
}

#[test]
fn edge_route_codes() {
    assert_eq!(Route::Copy.code(), 0);
    assert_eq!(Route::PremultiplyRows.code(), 1);
    assert_eq!(Route::Rgba64.code(), 2);
    assert_eq!(Route::Sws.code(), 3);
    assert_eq!(Premultiply::None.code(), 0);
    assert_eq!(Premultiply::Srgb.code(), 1);
    assert_eq!(Premultiply::Linear.code(), 2);
}

#[test]
fn edge_copy_src_len() {
    assert_eq!(copy_src_len(4, 0, 16), Some(0));
    assert_eq!(copy_src_len(0, 5, 16), Some(0));
    assert_eq!(copy_src_len(4, 3, 0), Some(0));
    assert_eq!(copy_src_len(4, 3, 16), Some(48));
    assert_eq!(copy_src_len(4, 3, 20), Some(56));
    assert_eq!(copy_src_len(4, 3, 12), Some(36));
    assert_eq!(copy_src_len(1, 1, 7), Some(4));
    assert_eq!(copy_src_len(usize::MAX, 1, 8), Some(8));
    assert_eq!(copy_src_len(1, usize::MAX, 8), None);
    assert_eq!(copy_src_len(1, 2, usize::MAX), None);
    // cx * 4 wraps to 0 as the C's size_t does: nothing is read.
    assert_eq!(copy_src_len(usize::MAX / 4 + 1, 1, usize::MAX), Some(0));
}

/// Buffers shorter than the image: the copy stops at the last whole row
/// that fits both, writes nothing past it and never panics.
#[test]
fn edge_short_buffers() {
    let (cx, cy) = (3usize, 4usize);
    for linesize in [8usize, 12, 16] {
        let src_full: Vec<u8> = (0..linesize * cy).map(|i| i as u8).collect();
        let copied = (cx * 4).min(linesize);
        for src_len in 0..=src_full.len() {
            for dst_len in 0..=cx * cy * 4 + 4 {
                let mut dst = vec![CANARY; dst_len];
                let rows = copy_rows(&mut dst, &src_full[..src_len], cx, cy, linesize);
                assert!(rows <= cy);
                for y in 0..rows {
                    assert_eq!(
                        dst[y * cx * 4..y * cx * 4 + copied],
                        src_full[y * linesize..y * linesize + copied],
                        "line size {linesize}, src {src_len}, dst {dst_len}, row {y}"
                    );
                }
                let ends = (rows < cy).then(|| {
                    let src_fits = rows * linesize + copied <= src_len;
                    let dst_fits = rows * cx * 4 + copied <= dst_len;
                    !(src_fits && dst_fits)
                });
                assert_ne!(ends, Some(false), "stopped early");
                for (i, &b) in dst.iter().enumerate() {
                    let (y, x) = (i / (cx * 4), i % (cx * 4));
                    if y >= rows || x >= copied {
                        assert_eq!(b, CANARY, "byte {i}");
                    }
                }
            }
        }
    }
}

#[test]
fn edge_huge_sizes() {
    let mut dst = [CANARY; 64];
    let src = [1u8; 64];
    for (cx, cy, ls) in [
        (usize::MAX, 1, 16),
        (1, usize::MAX, 4),
        (usize::MAX, usize::MAX, usize::MAX),
        (usize::MAX / 4 + 1, 2, 8),
        (0, usize::MAX, 0),
        (4, 2, usize::MAX),
    ] {
        let rows = copy_rows(&mut dst, &src, cx, cy, ls);
        assert!(rows <= 64);
    }
}

fn model(cx: usize, cy: usize, linesize: usize, src: &[u8]) -> Vec<u8> {
    let row = cx * TEXEL_BYTES;
    let mut out = vec![CANARY; row * cy];
    for y in 0..cy {
        for x in 0..row.min(linesize) {
            out[y * row + x] = src[y * linesize + x];
        }
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn edge_copy_matches_model(
        cx in 0usize..24,
        cy in 0usize..16,
        ls in 0usize..128,
        seed in any::<u64>(),
    ) {
        let src: Vec<u8> = (0..ls * cy + 8).map(|i| (seed.wrapping_mul(i as u64 + 1) >> 13) as u8).collect();
        let mut dst = vec![CANARY; cx * cy * 4];
        let rows = copy_rows(&mut dst, &src, cx, cy, ls);
        let expected_rows = if cx * 4 == 0 || ls == 0 { rows } else { cy };
        prop_assert_eq!(rows, expected_rows);
        prop_assert_eq!(dst, model(cx, cy, ls, &src));
        prop_assert!(copy_src_len(cx, cy, ls).unwrap() <= src.len());
    }

    #[test]
    fn edge_padding_never_copied(
        cx in 1usize..24,
        cy in 1usize..16,
        pad in 1usize..64,
        fill in any::<u8>(),
        seed in any::<u64>(),
    ) {
        let row = cx * 4;
        let tight: Vec<u8> = (0..row * cy).map(|i| (seed.wrapping_mul(i as u64 + 7) >> 17) as u8).collect();
        let ls = row + pad;
        let mut padded = vec![fill; ls * cy];
        for y in 0..cy {
            padded[y * ls..y * ls + row].copy_from_slice(&tight[y * row..(y + 1) * row]);
        }
        let mut a = vec![CANARY; row * cy];
        let mut b = vec![CANARY; row * cy];
        copy_rows(&mut a, &tight, cx, cy, row);
        copy_rows(&mut b, &padded, cx, cy, ls);
        prop_assert_eq!(&a, &tight);
        prop_assert_eq!(a, b);
    }
}
