//! Tier 1: the safe core of `libobs/graphics/libnsgif/gif-frame.c.h`.
//!
//! The `frame_*` cases mirror the frame cases of
//! `test/cmocka/test_nsgif_decode.c` one to one, with the same names,
//! streams and expected pixels. The cases after those pin the edges of the
//! safe API, and proptests check that no input panics and that nothing is
//! written outside the frame's clipped rectangle.

// Index loops mirror the cmocka cases' positions one to one.
#![allow(clippy::needless_range_loop)]

use obs_graphics::gif_blocks::{MAX_COLOURS, NsgifError, Rect};
use obs_graphics::gif_frame::{self, Canvas, FrameArea};
use obs_graphics::lzw::Decoder;
use proptest::prelude::*;

use super::lzw::Stream;

const SENTINEL: u32 = 0x5A5A_5A5A;

/// Palette entry i is 0xAB0000ii, so a pixel names its index.
fn palette() -> [u32; MAX_COLOURS] {
    core::array::from_fn(|i| 0xAB00_0000 | i as u32)
}

fn px(i: u32) -> u32 {
    0xAB00_0000 | i
}

/// The frame's image data: minimum code size 8, then the indices as root
/// codes after a clear, an EOI when `eoi`.
fn stream(idx: &[u8], eoi: bool, block: usize) -> Vec<u8> {
    let mut st = Stream::new(8);
    st.clear();
    for &i in idx {
        st.code(u32::from(i));
    }
    if eoi {
        st.eoi();
    }
    [vec![8], st.pack(block)].concat()
}

fn area(x: u32, y: u32, w: u32, h: u32) -> FrameArea {
    FrameArea {
        rect: Rect {
            x0: x,
            y0: y,
            x1: x + w,
            y1: y + h,
        },
        interlaced: false,
        display: true,
        transparency: false,
        transparency_index: 256,
    }
}

fn canvas(width: u32, height: u32, rowspan: u32) -> Canvas {
    Canvas {
        width,
        height,
        rowspan,
        background: 0,
    }
}

fn run(
    canvas: &Canvas,
    pixels: &mut [u32],
    frame: &FrameArea,
    data: &[u8],
    complete: bool,
) -> NsgifError {
    let mut lzw = Decoder::default();
    gif_frame::decode(
        &mut lzw,
        data,
        0,
        canvas,
        pixels,
        frame,
        &palette(),
        complete,
    )
}

#[test]
fn frame_simple_full_frame() {
    let idx: Vec<u8> = (1..=12).collect();
    let data = stream(&idx, true, 255);
    let c = canvas(4, 3, 4);
    let mut pixels = vec![SENTINEL; 64];

    assert_eq!(
        run(&c, &mut pixels, &area(0, 0, 4, 3), &data, false),
        NsgifError::Ok
    );
    for i in 0..12 {
        assert_eq!(pixels[i], px(i as u32 + 1));
    }
    assert_eq!(pixels[12], SENTINEL);

    pixels.fill(SENTINEL);
    assert_eq!(
        run(&c, &mut pixels, &area(0, 1, 4, 3), &data, false),
        NsgifError::Ok
    );
    assert_eq!(pixels[3], SENTINEL);
    for i in 0..8 {
        assert_eq!(pixels[4 + i], px(i as u32 + 1));
    }
    assert_eq!(pixels[12], SENTINEL);
}

#[test]
fn frame_complex_offset_and_clip() {
    let idx: Vec<u8> = (1..=12).collect();
    let data = stream(&idx, true, 255);
    let c = canvas(5, 4, 7);
    let mut pixels = vec![SENTINEL; 64];
    let at = |p: &[u32], x: usize, y: usize| p[y * 7 + x];

    assert_eq!(
        run(&c, &mut pixels, &area(3, 2, 4, 3), &data, false),
        NsgifError::Ok
    );
    assert_eq!(at(&pixels, 3, 2), px(1));
    assert_eq!(at(&pixels, 4, 2), px(2));
    assert_eq!(at(&pixels, 3, 3), px(5));
    assert_eq!(at(&pixels, 4, 3), px(6));
    assert_eq!(at(&pixels, 5, 2), SENTINEL);
    assert_eq!(at(&pixels, 2, 2), SENTINEL);
    assert_eq!(at(&pixels, 3, 1), SENTINEL);
    assert_eq!(at(&pixels, 3, 4), SENTINEL);

    // A full-width frame in a bitmap wider than the image: complex, rows by
    // the row span.
    let c = canvas(4, 3, 6);
    pixels.fill(SENTINEL);
    assert_eq!(
        run(&c, &mut pixels, &area(0, 0, 4, 3), &data, false),
        NsgifError::Ok
    );
    for r in 0..3 {
        for col in 0..4 {
            assert_eq!(pixels[r * 6 + col], px((r * 4 + col + 1) as u32));
        }
        assert_eq!(pixels[r * 6 + 4], SENTINEL);
    }

    let c = canvas(4, 3, 4);
    pixels.fill(SENTINEL);
    let mut a = area(0, 0, 4, 3);
    a.interlaced = true;
    assert_eq!(run(&c, &mut pixels, &a, &data, false), NsgifError::Ok);
    assert_eq!([pixels[0], pixels[4], pixels[8]], [px(1), px(9), px(5)]);
}

/// GIF89a appendix E: the row of the i-th decoded row.
fn interlaced_row(height: u32, mut i: u32) -> u32 {
    for (start, step) in [(0, 8), (4, 8), (2, 4), (1, 2)] {
        let mut y = start;
        while y < height {
            if i == 0 {
                return y;
            }
            i -= 1;
            y += step;
        }
    }
    u32::MAX
}

#[test]
fn frame_interlaced_heights() {
    let idx: Vec<u8> = (0..64).collect();
    for h in [1u32, 2, 3, 4, 5, 6, 7, 8, 9, 15, 16, 17, 31, 64] {
        let data = stream(&idx[..h as usize], true, 255);
        let c = canvas(1, h, 1);
        let mut pixels = vec![SENTINEL; 128];
        let mut a = area(0, 0, 1, h);
        a.interlaced = true;
        assert_eq!(run(&c, &mut pixels, &a, &data, false), NsgifError::Ok);
        for i in 0..h {
            assert_eq!(
                pixels[interlaced_row(h, i) as usize],
                px(i),
                "height {h} row {i}"
            );
        }
        assert_eq!(pixels[h as usize], SENTINEL);
    }
}

#[test]
fn frame_transparency() {
    let data = stream(&[1, 7, 2, 7, 7, 3], true, 255);
    let c = canvas(3, 2, 3);
    for complex in [false, true] {
        let mut pixels = vec![SENTINEL; 16];
        let mut a = area(0, 0, 3, 2);
        a.interlaced = complex;
        a.transparency_index = 7;
        assert_eq!(run(&c, &mut pixels, &a, &data, false), NsgifError::Ok);
        assert_eq!(
            pixels[..6],
            [px(1), SENTINEL, px(2), SENTINEL, SENTINEL, px(3)]
        );

        pixels.fill(SENTINEL);
        a.transparency_index = 0x100;
        assert_eq!(run(&c, &mut pixels, &a, &data, false), NsgifError::Ok);
        assert_eq!(pixels[1], px(7));
    }
}

#[test]
fn frame_outside_the_image() {
    let c = canvas(4, 3, 4);
    let bad = [12u8, 0];
    for [x, y, w, h] in [
        [4, 0, 2, 2],
        [0, 3, 2, 2],
        [9, 9, 1, 1],
        [1, 1, 0, 2],
        [1, 1, 2, 0],
        [0, 3, 4, 1],
        [0, 0, 4, 0],
    ] {
        let mut pixels = vec![SENTINEL; 16];
        assert_eq!(
            run(&c, &mut pixels, &area(x, y, w, h), &bad, false),
            NsgifError::Ok
        );
        assert!(pixels.iter().all(|&p| p == SENTINEL));
    }
    for complex in [false, true] {
        let mut pixels = vec![SENTINEL; 16];
        let mut a = area(0, 0, 4, 3);
        a.interlaced = complex;
        assert_eq!(run(&c, &mut pixels, &a, &bad, false), NsgifError::DataFrame);
        assert_eq!(pixels[0], SENTINEL);
    }
}

#[test]
fn frame_end_of_data() {
    let idx = [1u8, 2, 3, 4, 5];
    let c = canvas(4, 2, 4);
    for complex in [false, true] {
        let mut a = area(0, 0, 4, 2);
        a.interlaced = complex;

        let mut pixels = vec![SENTINEL; 16];
        assert_eq!(
            run(&c, &mut pixels, &a, &stream(&idx, true, 255), false),
            NsgifError::Ok
        );
        for i in 0..5 {
            assert_eq!(pixels[i], px(i as u32 + 1));
        }
        assert_eq!(pixels[5], SENTINEL);

        pixels.fill(SENTINEL);
        assert_eq!(
            run(&c, &mut pixels, &a, &stream(&idx, false, 255), false),
            NsgifError::Ok
        );
        assert_eq!(pixels[4], px(5));
        assert_eq!(pixels[5], SENTINEL);

        let mut cut = stream(&idx, false, 255);
        cut.pop();
        pixels.fill(SENTINEL);
        assert_eq!(run(&c, &mut pixels, &a, &cut, false), NsgifError::EndOfData);
        pixels.fill(SENTINEL);
        assert_eq!(run(&c, &mut pixels, &a, &cut, true), NsgifError::Ok);
    }
}

#[test]
fn frame_lzw_results() {
    let c = canvas(4, 2, 4);
    for complex in [false, true] {
        let mut a = area(0, 0, 4, 2);
        a.interlaced = complex;

        let mut st = Stream::new(8);
        st.clear();
        st.code(1);
        st.code(2);
        st.put(300, st.width);
        let data = [vec![8], st.pack(255)].concat();
        let mut pixels = vec![SENTINEL; 16];
        assert_eq!(
            run(&c, &mut pixels, &a, &data, false),
            NsgifError::DataFrame
        );
        assert_eq!(pixels[..2], [px(1), px(2)]);
        assert_eq!(run(&c, &mut pixels, &a, &data, true), NsgifError::DataFrame);

        let mut st = Stream::new(8);
        st.put(300, st.width);
        let data = [vec![8], st.pack(255)].concat();
        pixels.fill(SENTINEL);
        assert_eq!(
            run(&c, &mut pixels, &a, &data, false),
            NsgifError::DataFrame
        );
        assert_eq!(pixels[0], SENTINEL);
    }
}

#[test]
fn frame_restore_bg() {
    let c = Canvas {
        width: 5,
        height: 4,
        rowspan: 7,
        background: 0xFF11_2233,
    };
    let mut pixels = vec![SENTINEL; 64];

    gif_frame::restore_bg(&c, &mut pixels, None);
    assert!(pixels[..20].iter().all(|&p| p == 0));
    assert_eq!(pixels[20], SENTINEL);

    pixels.fill(SENTINEL);
    let mut a = area(3, 2, 4, 3);
    gif_frame::restore_bg(&c, &mut pixels, Some(&a));
    for i in [2 * 5 + 3, 2 * 5 + 4, 3 * 5 + 3, 3 * 5 + 4] {
        assert_eq!(pixels[i], 0xFF11_2233);
    }
    for i in [2 * 5 + 5, 4 * 5 + 3, 2 * 5 + 2] {
        assert_eq!(pixels[i], SENTINEL);
    }

    pixels.fill(SENTINEL);
    a.transparency = true;
    gif_frame::restore_bg(&c, &mut pixels, Some(&a));
    assert_eq!(pixels[2 * 5 + 3], 0);
    assert_eq!(pixels[3 * 5 + 4], 0);
    assert_eq!(pixels[2 * 5 + 5], SENTINEL);

    for [x, y, w, h] in [[5, 0, 1, 1], [0, 4, 1, 1]] {
        pixels.fill(SENTINEL);
        gif_frame::restore_bg(&c, &mut pixels, Some(&area(x, y, w, h)));
        assert!(pixels[..28].iter().all(|&p| p == SENTINEL));
    }
    pixels.fill(SENTINEL);
    let mut hidden = area(0, 0, 2, 2);
    hidden.display = false;
    gif_frame::restore_bg(&c, &mut pixels, Some(&hidden));
    assert_eq!(pixels[0], SENTINEL);
}

#[test]
fn frame_recover() {
    let saved: Vec<u32> = (0..28).map(|i| 0xC000_0000 | i).collect();
    let c = canvas(5, 4, 7);
    let mut pixels = vec![SENTINEL; 64];
    gif_frame::recover(&c, &mut pixels, &saved);
    for i in 0..20 {
        assert_eq!(pixels[i], 0xC000_0000 | i as u32);
    }
    assert_eq!(pixels[20], SENTINEL);
}

// ---------------------------------------------------------------------------
// Edges of the safe API.

#[test]
fn edge_short_pixel_slice_writes_what_fits() {
    // The C trusts the bitmap size; here a short slice takes what fits.
    let idx: Vec<u8> = (1..=12).collect();
    let data = stream(&idx, true, 255);
    let c = canvas(4, 3, 4);
    for complex in [false, true] {
        let mut a = area(0, 0, 4, 3);
        a.interlaced = complex;
        let mut pixels = vec![SENTINEL; 5];
        assert_eq!(run(&c, &mut pixels, &a, &data, false), NsgifError::Ok);
        assert_eq!(pixels[0], px(1));
    }
    let mut pixels = vec![SENTINEL; 5];
    gif_frame::restore_bg(&c, &mut pixels, None);
    assert_eq!(pixels, [0; 5]);
    gif_frame::recover(&c, &mut pixels, &[7; 3]);
    assert_eq!(pixels, [7, 7, 7, 0, 0]);
}

#[test]
fn edge_data_position_past_the_data() {
    let c = canvas(4, 3, 4);
    let mut pixels = vec![SENTINEL; 16];
    let mut lzw = Decoder::default();
    assert_eq!(
        gif_frame::decode(
            &mut lzw,
            &[8, 0],
            5,
            &c,
            &mut pixels,
            &area(0, 0, 4, 3),
            &palette(),
            false
        ),
        NsgifError::EndOfData
    );
    assert_eq!(pixels[0], SENTINEL);
}

// ---------------------------------------------------------------------------
// Properties.

fn clipped_contains(c: &Canvas, a: &FrameArea, x: u32, y: u32) -> bool {
    x >= a.rect.x0 && y >= a.rect.y0 && x < a.rect.x1.min(c.width) && y < a.rect.y1.min(c.height)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn prop_decode_writes_only_inside_the_clipped_frame(
        width in 1u32..12, height in 1u32..12, extra in 0u32..3,
        x in 0u32..14, y in 0u32..14, w in 0u32..14, h in 0u32..14,
        interlaced in any::<bool>(),
        idx in proptest::collection::vec(any::<u8>(), 0..300),
        eoi in any::<bool>(),
        noise in proptest::collection::vec(any::<u8>(), 0..40),
        use_noise in any::<bool>(),
        complete in any::<bool>(),
    ) {
        let c = Canvas { width, height, rowspan: width + extra, background: 0 };
        let mut a = area(x, y, w, h);
        a.interlaced = interlaced;
        let data = if use_noise { noise } else { stream(&idx, eoi, 7) };
        let mut pixels = vec![SENTINEL; ((width + extra) * height) as usize];
        let mut lzw = Decoder::default();
        let _ = gif_frame::decode(&mut lzw, &data, 0, &c, &mut pixels, &a, &palette(), complete);
        let simple = !interlaced && x == 0 && w == width && extra == 0;
        for (i, &p) in pixels.iter().enumerate() {
            let (px_x, px_y) = ((i as u32) % (width + extra), (i as u32) / (width + extra));
            if p != SENTINEL {
                if simple {
                    // The simple path writes whole rows from row y on.
                    prop_assert!(px_y >= y);
                } else {
                    prop_assert!(clipped_contains(&c, &a, px_x, px_y), "({px_x}, {px_y}) outside");
                }
            }
        }
    }

    #[test]
    fn prop_restore_writes_only_the_clipped_frame(
        width in 1u32..12, height in 1u32..12,
        x in 0u32..14, y in 0u32..14, w in 0u32..14, h in 0u32..14,
        transparency in any::<bool>(), display in any::<bool>(),
    ) {
        let c = Canvas { width, height, rowspan: width, background: 0xFF00_00FF };
        let mut a = area(x, y, w, h);
        a.transparency = transparency;
        a.display = display;
        let mut pixels = vec![SENTINEL; (width * height) as usize];
        gif_frame::restore_bg(&c, &mut pixels, Some(&a));
        for (i, &p) in pixels.iter().enumerate() {
            let (px_x, px_y) = ((i as u32) % width, (i as u32) / width);
            let inside = display && clipped_contains(&c, &a, px_x, px_y);
            let restored = if transparency { 0 } else { 0xFF00_00FF };
            prop_assert_eq!(p, if inside { restored } else { SENTINEL });
        }
    }
}
