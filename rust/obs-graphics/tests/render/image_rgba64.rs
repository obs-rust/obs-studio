//! Tier 1: `obs_graphics::image_rgba64`, the 16-bit RGBA rows of
//! `graphics-ffmpeg.c`.
//!
//! The `steps_*` cases port `test/cmocka/test_graphics_ffmpeg_rgba64_steps.c`
//! 1:1: its 10 cases under the same names, in the same order, with the same
//! values and expectations, the characterized-not-endorsed ones included (a
//! short line size packs the rows and reads the source as one run; an
//! unknown alpha mode writes nothing). No intentional difference.
//!
//! The `edge_*` cases are new: the row width and source length helpers, a
//! negative width, buffers shorter than the frame (the rows stop, nothing
//! panics), sizes at the ends of `i32` and `usize`, and proptests: the rows
//! against [`texel`] per texel, a padded frame against the tight one, the
//! premultiplied modes against straight alpha, and the alpha byte the same
//! in every mode.

use obs_graphics::image_rgba64::{
    DST_TEXEL_BYTES, SRC_TEXEL_BYTES, alpha_mode, rgba64be_rows, row_texels, src_len, texel,
};
use proptest::prelude::*;

use alpha_mode::{PREMULTIPLY, PREMULTIPLY_SRGB, STRAIGHT};

const CANARY: u8 = 0xa5;

// ---------------------------------------------------------------------------
// helpers

fn put_texel(src: &mut [u8], r: u16, g: u16, b: u16, a: u16) {
    for (c, v) in [r, g, b, a].into_iter().enumerate() {
        src[2 * c..2 * c + 2].copy_from_slice(&v.to_be_bytes());
    }
}

fn be(r: u16, g: u16, b: u16, a: u16) -> [u8; SRC_TEXEL_BYTES] {
    let mut t = [0; SRC_TEXEL_BYTES];
    put_texel(&mut t, r, g, b, a);
    t
}

/// A texel whose 8-bit value (straight) is its index: grey index * 257.
fn fill_indexed(src: &mut [u8], cx: usize, cy: usize, linesize: usize, pad: u8) {
    src[..linesize * cy].fill(pad);
    for y in 0..cy {
        for x in 0..cx {
            let v = u16::try_from((y * cx + x) * 257).unwrap();
            put_texel(&mut src[y * linesize + x * 8..], v, v, v, 65535);
        }
    }
}

fn check_indexed(what: &str, dst: &[u8], texels: usize) {
    for i in 0..texels {
        let v = u8::try_from(i).unwrap();
        assert_eq!(dst[i * 4..i * 4 + 4], [v, v, v, 255], "{what}: texel {i}");
    }
}

fn check_canary(what: &str, dst: &[u8], from: usize) {
    for (i, b) in dst.iter().enumerate().skip(from) {
        assert_eq!(*b, CANARY, "{what}: byte {i} written");
    }
}

fn to_i32(n: usize) -> i32 {
    i32::try_from(n).unwrap()
}

// ---------------------------------------------------------------------------
// test_graphics_ffmpeg_rgba64_steps.c, 1:1

/// One texel of grey v with alpha a, in each mode: the colour byte and the
/// alpha byte. 33025 is 128.5 in 8 bits, which rounds up to 129: dividing
/// by 65536 instead of 65535 would give 128.
struct Grey {
    mode: i32,
    v: u16,
    a: u16,
    colour: u8,
    alpha: u8,
}

#[rustfmt::skip]
const GREYS: &[Grey] = &[
    Grey { mode: STRAIGHT, v: 0, a: 0, colour: 0, alpha: 0 },
    Grey { mode: STRAIGHT, v: 0, a: 32768, colour: 0, alpha: 128 },
    Grey { mode: STRAIGHT, v: 0, a: 33025, colour: 0, alpha: 129 },
    Grey { mode: STRAIGHT, v: 0, a: 65535, colour: 0, alpha: 255 },
    Grey { mode: STRAIGHT, v: 1, a: 0, colour: 0, alpha: 0 },
    Grey { mode: STRAIGHT, v: 1, a: 32768, colour: 0, alpha: 128 },
    Grey { mode: STRAIGHT, v: 1, a: 33025, colour: 0, alpha: 129 },
    Grey { mode: STRAIGHT, v: 1, a: 65535, colour: 0, alpha: 255 },
    Grey { mode: STRAIGHT, v: 257, a: 0, colour: 1, alpha: 0 },
    Grey { mode: STRAIGHT, v: 257, a: 32768, colour: 1, alpha: 128 },
    Grey { mode: STRAIGHT, v: 257, a: 33025, colour: 1, alpha: 129 },
    Grey { mode: STRAIGHT, v: 257, a: 65535, colour: 1, alpha: 255 },
    Grey { mode: STRAIGHT, v: 32767, a: 0, colour: 127, alpha: 0 },
    Grey { mode: STRAIGHT, v: 32767, a: 32768, colour: 127, alpha: 128 },
    Grey { mode: STRAIGHT, v: 32767, a: 33025, colour: 127, alpha: 129 },
    Grey { mode: STRAIGHT, v: 32767, a: 65535, colour: 127, alpha: 255 },
    Grey { mode: STRAIGHT, v: 32768, a: 0, colour: 128, alpha: 0 },
    Grey { mode: STRAIGHT, v: 32768, a: 32768, colour: 128, alpha: 128 },
    Grey { mode: STRAIGHT, v: 32768, a: 33025, colour: 128, alpha: 129 },
    Grey { mode: STRAIGHT, v: 32768, a: 65535, colour: 128, alpha: 255 },
    Grey { mode: STRAIGHT, v: 33025, a: 0, colour: 129, alpha: 0 },
    Grey { mode: STRAIGHT, v: 33025, a: 32768, colour: 129, alpha: 128 },
    Grey { mode: STRAIGHT, v: 33025, a: 33025, colour: 129, alpha: 129 },
    Grey { mode: STRAIGHT, v: 33025, a: 65535, colour: 129, alpha: 255 },
    Grey { mode: STRAIGHT, v: 65535, a: 0, colour: 255, alpha: 0 },
    Grey { mode: STRAIGHT, v: 65535, a: 32768, colour: 255, alpha: 128 },
    Grey { mode: STRAIGHT, v: 65535, a: 33025, colour: 255, alpha: 129 },
    Grey { mode: STRAIGHT, v: 65535, a: 65535, colour: 255, alpha: 255 },
    Grey { mode: PREMULTIPLY_SRGB, v: 0, a: 0, colour: 0, alpha: 0 },
    Grey { mode: PREMULTIPLY_SRGB, v: 0, a: 32768, colour: 0, alpha: 128 },
    Grey { mode: PREMULTIPLY_SRGB, v: 0, a: 33025, colour: 0, alpha: 129 },
    Grey { mode: PREMULTIPLY_SRGB, v: 0, a: 65535, colour: 0, alpha: 255 },
    Grey { mode: PREMULTIPLY_SRGB, v: 1, a: 0, colour: 0, alpha: 0 },
    Grey { mode: PREMULTIPLY_SRGB, v: 1, a: 32768, colour: 0, alpha: 128 },
    Grey { mode: PREMULTIPLY_SRGB, v: 1, a: 33025, colour: 0, alpha: 129 },
    Grey { mode: PREMULTIPLY_SRGB, v: 1, a: 65535, colour: 0, alpha: 255 },
    Grey { mode: PREMULTIPLY_SRGB, v: 257, a: 0, colour: 0, alpha: 0 },
    Grey { mode: PREMULTIPLY_SRGB, v: 257, a: 32768, colour: 1, alpha: 128 },
    Grey { mode: PREMULTIPLY_SRGB, v: 257, a: 33025, colour: 1, alpha: 129 },
    Grey { mode: PREMULTIPLY_SRGB, v: 257, a: 65535, colour: 1, alpha: 255 },
    Grey { mode: PREMULTIPLY_SRGB, v: 32767, a: 0, colour: 0, alpha: 0 },
    Grey { mode: PREMULTIPLY_SRGB, v: 32767, a: 32768, colour: 92, alpha: 128 },
    Grey { mode: PREMULTIPLY_SRGB, v: 32767, a: 33025, colour: 92, alpha: 129 },
    Grey { mode: PREMULTIPLY_SRGB, v: 32767, a: 65535, colour: 127, alpha: 255 },
    Grey { mode: PREMULTIPLY_SRGB, v: 32768, a: 0, colour: 0, alpha: 0 },
    Grey { mode: PREMULTIPLY_SRGB, v: 32768, a: 32768, colour: 92, alpha: 128 },
    Grey { mode: PREMULTIPLY_SRGB, v: 32768, a: 33025, colour: 92, alpha: 129 },
    Grey { mode: PREMULTIPLY_SRGB, v: 32768, a: 65535, colour: 128, alpha: 255 },
    Grey { mode: PREMULTIPLY_SRGB, v: 33025, a: 0, colour: 0, alpha: 0 },
    Grey { mode: PREMULTIPLY_SRGB, v: 33025, a: 32768, colour: 93, alpha: 128 },
    Grey { mode: PREMULTIPLY_SRGB, v: 33025, a: 33025, colour: 93, alpha: 129 },
    Grey { mode: PREMULTIPLY_SRGB, v: 33025, a: 65535, colour: 129, alpha: 255 },
    Grey { mode: PREMULTIPLY_SRGB, v: 65535, a: 0, colour: 0, alpha: 0 },
    Grey { mode: PREMULTIPLY_SRGB, v: 65535, a: 32768, colour: 188, alpha: 128 },
    Grey { mode: PREMULTIPLY_SRGB, v: 65535, a: 33025, colour: 188, alpha: 129 },
    Grey { mode: PREMULTIPLY_SRGB, v: 65535, a: 65535, colour: 255, alpha: 255 },
    Grey { mode: PREMULTIPLY, v: 0, a: 0, colour: 0, alpha: 0 },
    Grey { mode: PREMULTIPLY, v: 0, a: 32768, colour: 0, alpha: 128 },
    Grey { mode: PREMULTIPLY, v: 0, a: 33025, colour: 0, alpha: 129 },
    Grey { mode: PREMULTIPLY, v: 0, a: 65535, colour: 0, alpha: 255 },
    Grey { mode: PREMULTIPLY, v: 1, a: 0, colour: 0, alpha: 0 },
    Grey { mode: PREMULTIPLY, v: 1, a: 32768, colour: 0, alpha: 128 },
    Grey { mode: PREMULTIPLY, v: 1, a: 33025, colour: 0, alpha: 129 },
    Grey { mode: PREMULTIPLY, v: 1, a: 65535, colour: 0, alpha: 255 },
    Grey { mode: PREMULTIPLY, v: 257, a: 0, colour: 0, alpha: 0 },
    Grey { mode: PREMULTIPLY, v: 257, a: 32768, colour: 1, alpha: 128 },
    Grey { mode: PREMULTIPLY, v: 257, a: 33025, colour: 1, alpha: 129 },
    Grey { mode: PREMULTIPLY, v: 257, a: 65535, colour: 1, alpha: 255 },
    Grey { mode: PREMULTIPLY, v: 32767, a: 0, colour: 0, alpha: 0 },
    Grey { mode: PREMULTIPLY, v: 32767, a: 32768, colour: 64, alpha: 128 },
    Grey { mode: PREMULTIPLY, v: 32767, a: 33025, colour: 64, alpha: 129 },
    Grey { mode: PREMULTIPLY, v: 32767, a: 65535, colour: 127, alpha: 255 },
    Grey { mode: PREMULTIPLY, v: 32768, a: 0, colour: 0, alpha: 0 },
    Grey { mode: PREMULTIPLY, v: 32768, a: 32768, colour: 64, alpha: 128 },
    Grey { mode: PREMULTIPLY, v: 32768, a: 33025, colour: 64, alpha: 129 },
    Grey { mode: PREMULTIPLY, v: 32768, a: 65535, colour: 128, alpha: 255 },
    Grey { mode: PREMULTIPLY, v: 33025, a: 0, colour: 0, alpha: 0 },
    Grey { mode: PREMULTIPLY, v: 33025, a: 32768, colour: 64, alpha: 128 },
    Grey { mode: PREMULTIPLY, v: 33025, a: 33025, colour: 65, alpha: 129 },
    Grey { mode: PREMULTIPLY, v: 33025, a: 65535, colour: 129, alpha: 255 },
    Grey { mode: PREMULTIPLY, v: 65535, a: 0, colour: 0, alpha: 0 },
    Grey { mode: PREMULTIPLY, v: 65535, a: 32768, colour: 128, alpha: 128 },
    Grey { mode: PREMULTIPLY, v: 65535, a: 33025, colour: 129, alpha: 129 },
    Grey { mode: PREMULTIPLY, v: 65535, a: 65535, colour: 255, alpha: 255 },
];

fn check_greys(mode: i32) {
    for g in GREYS.iter().filter(|g| g.mode == mode) {
        let src = be(g.v, g.v, g.v, g.a);
        let mut dst = [CANARY; 4];
        rgba64be_rows(&mut dst, &src, 1, 1, src.len(), mode);
        assert_eq!(
            dst,
            [g.colour, g.colour, g.colour, g.alpha],
            "mode {mode}, grey {}, alpha {}",
            g.v,
            g.a
        );
    }
}

#[test]
fn steps_alpha_mode_values() {
    assert_eq!(STRAIGHT, 0);
    assert_eq!(PREMULTIPLY_SRGB, 1);
    assert_eq!(PREMULTIPLY, 2);
}

#[test]
fn steps_straight_values() {
    check_greys(STRAIGHT);
}

#[test]
fn steps_premultiply_srgb_values() {
    check_greys(PREMULTIPLY_SRGB);
}

#[test]
fn steps_premultiply_values() {
    check_greys(PREMULTIPLY);
}

/// The channels stay in order, and each is read big-endian.
#[test]
fn steps_channel_order_and_byte_order() {
    let mut dst = [0u8; 4];
    rgba64be_rows(&mut dst, &be(65535, 32768, 0, 65535), 1, 1, 8, STRAIGHT);
    assert_eq!(dst, [255, 128, 0, 255], "channel order");

    // 0x00FF is 255 (1 in 8 bits), 0xFF00 is 65280 (254), 0x1234 is 4660
    // (18).
    let from_be = [0x00, 0xFF, 0xFF, 0x00, 0x12, 0x34, 0xFF, 0xFF];
    rgba64be_rows(&mut dst, &from_be, 1, 1, 8, STRAIGHT);
    assert_eq!(dst, [1, 254, 18, 255], "byte order");
}

#[test]
fn steps_one_by_one() {
    let mut src = [0u8; 8];
    let mut dst = [CANARY; 8];
    fill_indexed(&mut src, 1, 1, 8, 0);
    rgba64be_rows(&mut dst, &src, 1, 1, 8, STRAIGHT);
    check_indexed("1 x 1", &dst, 1);
    check_canary("past 1 x 1", &dst, 4);
}

/// Odd widths, tight and with padding (filled with 0xEE, which no texel of
/// the image has): every texel in place, the padding never read, nothing
/// written past cx * cy * 4.
#[test]
fn steps_odd_widths_and_padding() {
    let mut src = [0u8; 9 * 8 * 5 + 24 * 5];
    let mut dst = [0u8; 9 * 5 * 4 + 16];
    for cx in [1, 3, 5, 7, 9] {
        for pad in [0, 2, 8, 24] {
            for mode in 0..3 {
                let cy = 5;
                let linesize = cx * 8 + pad;
                fill_indexed(&mut src, cx, cy, linesize, 0xEE);
                dst.fill(CANARY);
                rgba64be_rows(&mut dst, &src, to_i32(cx), to_i32(cy), linesize, mode);

                let what = format!("{cx} x {cy}, padding {pad}, mode {mode}");
                // Alpha is 1, so every mode gives the straight values.
                check_indexed(&what, &dst, cx * cy);
                check_canary(&what, &dst, cx * cy * 4);
            }
        }
    }
}

/// Characterized, not endorsed: a source line size shorter than cx * 8
/// bytes converts only line size / 8 texels per row, and the destination
/// advances by what was written, so the rows pack together and the end of
/// the output is left unwritten. The source is not stepped to the next line
/// either, so every row reads on from where the last one stopped: the
/// texels come from the source as one run of 8 bytes each, whatever the
/// line size's remainder. FFmpeg never hands over such a frame.
fn check_short_linesize(linesize: usize) {
    let (cx, cy) = (4, 3);
    let per_row = linesize / 8;
    let mut src = [0xEEu8; 3 * 31];
    let mut dst = [CANARY; 4 * 3 * 4];
    for k in 0..per_row * cy {
        let v = u16::try_from(k * 257).unwrap();
        put_texel(&mut src[k * 8..], v, v, v, 65535);
    }
    rgba64be_rows(&mut dst, &src, cx, to_i32(cy), linesize, STRAIGHT);

    let what = format!("line size {linesize}");
    check_indexed(&what, &dst, per_row * cy);
    check_canary(&what, &dst, per_row * cy * 4);
}

#[test]
fn steps_short_linesize_packs_rows() {
    check_short_linesize(16); // 2 texels
    check_short_linesize(20); // 2 texels and 4 bytes, not skipped
    check_short_linesize(31); // 3 texels and 7 bytes, not skipped
    check_short_linesize(7); // under one texel: nothing
}

/// Characterized, not endorsed: an alpha mode outside the three writes
/// nothing.
#[test]
fn steps_unknown_alpha_mode_writes_nothing() {
    let mut src = [0u8; 2 * 2 * 8];
    fill_indexed(&mut src, 2, 2, 16, 0);
    for mode in [-1, 3, 100] {
        let mut dst = [CANARY; 2 * 2 * 4];
        rgba64be_rows(&mut dst, &src, 2, 2, 16, mode);
        check_canary(&format!("mode {mode}"), &dst, 0);
    }
}

/// No rows or no columns: nothing is written.
#[test]
fn steps_empty_image_writes_nothing() {
    let mut src = [0u8; 48];
    fill_indexed(&mut src, 2, 3, 16, 0);
    for mode in 0..3 {
        let mut dst = [CANARY; 16];
        rgba64be_rows(&mut dst, &src, 2, 0, 16, mode);
        rgba64be_rows(&mut dst, &src, 0, 2, 16, mode);
        check_canary(&format!("mode {mode}"), &dst, 0);
    }
}

// ---------------------------------------------------------------------------
// edge cases

#[test]
fn edge_row_texels() {
    assert_eq!(row_texels(4, 32), 4);
    assert_eq!(row_texels(4, 40), 4, "padding is not converted");
    assert_eq!(row_texels(4, 31), 3, "a short line: whole texels only");
    assert_eq!(row_texels(4, 7), 0);
    assert_eq!(row_texels(0, 64), 0);
    // The C's size_t: a negative width is huge, so the line size decides.
    assert_eq!(row_texels(-1, 64), 8);
    assert_eq!(row_texels(i32::MIN, 24), 3);
    assert_eq!(
        row_texels(i32::MAX, usize::MAX),
        usize::try_from(i32::MAX).unwrap()
    );
}

#[test]
fn edge_src_len() {
    assert_eq!(
        src_len(4, 3, 40),
        Some(2 * 40 + 32),
        "padded: the last row's texels"
    );
    assert_eq!(src_len(4, 3, 32), Some(96));
    assert_eq!(
        src_len(4, 3, 20),
        Some(2 * 20 + 16),
        "short: a bound, the rows read 48"
    );
    assert_eq!(src_len(4, 0, 40), Some(0));
    assert_eq!(src_len(4, -5, 40), Some(0));
    assert_eq!(src_len(0, 3, 40), Some(0));
    assert_eq!(src_len(4, 3, 7), Some(0));
    assert_eq!(src_len(i32::MAX, i32::MAX, usize::MAX), None);
}

/// The C's size_t width: -1 is the widest image, so the line size gives
/// the row.
#[test]
fn edge_negative_width_takes_the_line_size() {
    let mut src = [0u8; 2 * 16];
    fill_indexed(&mut src, 2, 2, 16, 0);
    let mut dst = [CANARY; 16];
    assert_eq!(rgba64be_rows(&mut dst, &src, -1, 2, 16, STRAIGHT), 16);
    check_indexed("cx -1", &dst, 4);
}

#[test]
fn edge_short_dst_stops_at_the_last_whole_texel() {
    let mut src = [0u8; 3 * 3 * 8];
    fill_indexed(&mut src, 3, 3, 24, 0);
    for len in 0..36 {
        let mut dst = vec![CANARY; len];
        let written = rgba64be_rows(&mut dst, &src, 3, 3, 24, STRAIGHT);
        assert_eq!(written, len / 4 * 4, "dst of {len}");
        check_indexed(&format!("dst of {len}"), &dst, len / 4);
        check_canary(&format!("dst of {len}"), &dst, written);
    }
}

#[test]
fn edge_short_src_stops_at_the_last_whole_texel() {
    let mut full = [0u8; 3 * 40];
    fill_indexed(&mut full, 3, 3, 40, 0xEE);
    for len in 0..full.len() {
        let mut dst = [CANARY; 36];
        let written = rgba64be_rows(&mut dst, &full[..len], 3, 3, 40, STRAIGHT);
        let texels = (0..9)
            .filter(|i| (i / 3) * 40 + (i % 3) * 8 + 8 <= len)
            .count();
        assert_eq!(written, texels * 4, "src of {len}");
        check_indexed(&format!("src of {len}"), &dst, texels);
        check_canary(&format!("src of {len}"), &dst, written);
    }
}

#[test]
fn edge_extreme_sizes_do_not_panic() {
    let src = [0x80u8; 64];
    let mut dst = [CANARY; 32];
    for cx in [i32::MIN, -1, 0, 1, i32::MAX] {
        for cy in [i32::MIN, -1, 0, 1, i32::MAX] {
            for linesize in [0, 7, 8, 64, usize::MAX >> 1, usize::MAX] {
                for mode in -1..4 {
                    let written = rgba64be_rows(&mut dst, &src, cx, cy, linesize, mode);
                    assert!(written <= dst.len());
                    assert_eq!(written % DST_TEXEL_BYTES, 0);
                }
            }
        }
    }
}

#[test]
fn edge_texel_unknown_mode() {
    for mode in [-1, 3, i32::MIN, i32::MAX] {
        assert_eq!(texel(be(1, 2, 3, 4), mode), None);
    }
}

#[test]
fn edge_texel_ends() {
    for mode in 0..3 {
        assert_eq!(texel(be(0, 0, 0, 0), mode), Some([0, 0, 0, 0]));
        assert_eq!(texel(be(65535, 65535, 65535, 65535), mode), Some([255; 4]));
        // Transparent: the premultiplied colour is black.
        let clear = texel(be(65535, 32768, 1, 0), mode).unwrap();
        if mode == STRAIGHT {
            assert_eq!(clear, [255, 128, 0, 0]);
        } else {
            assert_eq!(clear, [0, 0, 0, 0]);
        }
    }
}

fn frame(cx: usize, cy: usize, pad: usize, texels: &[[u16; 4]], pad_byte: u8) -> Vec<u8> {
    let linesize = cx * 8 + pad;
    let mut src = vec![pad_byte; linesize * cy];
    for y in 0..cy {
        for x in 0..cx {
            let [r, g, b, a] = texels[y * cx + x];
            put_texel(&mut src[y * linesize + x * 8..], r, g, b, a);
        }
    }
    src
}

fn texels(max: usize) -> impl Strategy<Value = Vec<[u16; 4]>> {
    prop::collection::vec(any::<[u16; 4]>(), max..=max)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// The rows convert texel by texel with `texel`, in order.
    #[test]
    fn edge_rows_are_texels(
        (cx, cy, t) in (1usize..=24, 1usize..=8).prop_flat_map(|(cx, cy)| (Just(cx), Just(cy), texels(cx * cy))),
        pad in 0usize..24,
        mode in -1i32..4,
    ) {
        let src = frame(cx, cy, pad, &t, 0xEE);
        let mut dst = vec![CANARY; cx * cy * 4];
        let written = rgba64be_rows(&mut dst, &src, to_i32(cx), to_i32(cy), cx * 8 + pad, mode);
        if (0..3).contains(&mode) {
            prop_assert_eq!(written, cx * cy * 4);
            for (i, rgba) in t.iter().enumerate() {
                let want = texel(be(rgba[0], rgba[1], rgba[2], rgba[3]), mode).unwrap();
                prop_assert_eq!(&dst[i * 4..i * 4 + 4], &want[..], "texel {}", i);
            }
        } else {
            prop_assert_eq!(written, 0);
            prop_assert!(dst.iter().all(|b| *b == CANARY));
        }
    }

    /// Padding changes nothing: a padded frame gives the tight frame's
    /// output, whatever the padding holds.
    #[test]
    fn edge_padding_is_skipped(
        (cx, cy, t) in (1usize..=16, 1usize..=6).prop_flat_map(|(cx, cy)| (Just(cx), Just(cy), texels(cx * cy))),
        pad in 1usize..40,
        pad_byte in any::<u8>(),
        mode in 0i32..3,
    ) {
        let mut tight = vec![0u8; cx * cy * 4];
        let mut padded = vec![0u8; cx * cy * 4];
        rgba64be_rows(&mut tight, &frame(cx, cy, 0, &t, 0), to_i32(cx), to_i32(cy), cx * 8, mode);
        rgba64be_rows(&mut padded, &frame(cx, cy, pad, &t, pad_byte), to_i32(cx), to_i32(cy), cx * 8 + pad, mode);
        prop_assert_eq!(tight, padded);
    }

    /// Opaque, premultiplying changes nothing; transparent, the colour is
    /// black; the alpha byte is the same in every mode; premultiplied as
    /// stored, no colour is above its alpha.
    #[test]
    fn edge_modes_agree(r in any::<u16>(), g in any::<u16>(), b in any::<u16>(), a in any::<u16>()) {
        let straight = texel(be(r, g, b, a), STRAIGHT).unwrap();
        let srgb = texel(be(r, g, b, a), PREMULTIPLY_SRGB).unwrap();
        let pre = texel(be(r, g, b, a), PREMULTIPLY).unwrap();
        prop_assert_eq!(straight[3], srgb[3]);
        prop_assert_eq!(straight[3], pre[3]);
        for c in 0..3 {
            prop_assert!(pre[c] <= pre[3], "colour {} of {:?}", c, pre);
        }
        let opaque = texel(be(r, g, b, 65535), STRAIGHT).unwrap();
        prop_assert_eq!(texel(be(r, g, b, 65535), PREMULTIPLY).unwrap(), opaque);
        prop_assert_eq!(texel(be(r, g, b, 0), PREMULTIPLY).unwrap(), [0, 0, 0, 0]);
        prop_assert_eq!(texel(be(r, g, b, 0), PREMULTIPLY_SRGB).unwrap(), [0, 0, 0, 0]);
    }
}
