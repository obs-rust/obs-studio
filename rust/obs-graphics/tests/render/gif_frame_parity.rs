//! Tier 3: the shims against the original
//! `libobs/graphics/libnsgif/gif-frame.c.h`.
//!
//! The oracle (`rust/obs-c-oracle/oracle/gif_frame.c`) is the C file,
//! unmodified, with its three functions renamed and its LZW calls on the C
//! LZW oracle. So the Rust side is the Rust frame decode on the Rust LZW
//! decoder (#2466), and the C side is all C. Both get the same call through
//! their C signatures, each with a new LZW context, and are compared after
//! it: the result and the whole bitmap.
//!
//! Runs: the struct layouts; and 20 000 random cases each for the decode
//! and the disposal, on canvases of 1 to 24 by 1 to 24 pixels with row
//! spans up to 3 wider, frames inside, across and past every edge (zero
//! size too), interlaced or not, with and without a transparent index,
//! over code streams from the LZW emitter (with and without an EOI, cut at
//! random lengths, behind random bytes) and random bytes, with and without
//! complete data.
//!
//! Excluded, the intentional differences (C dereferences null or writes
//! past the bitmap): null pointers, and a bitmap shorter than
//! `max(width, rowspan) * height`. Those are tested on the shim alone
//! below.

use core::ffi::c_void;
use core::ptr;

use obs_c_oracle::gif_frame as c;
use obs_c_oracle::lzw as c_lzw;
use obs_graphics::ffi::gif_frame::{
    CCanvas, nsgif_frame_pixels_decode, nsgif_frame_recover, nsgif_frame_restore_bg,
};
use obs_graphics::ffi::lzw::{LzwCtx, lzw_context_create, lzw_context_destroy};
use obs_graphics::gif_blocks::{MAX_COLOURS, NsgifError, Rect};
use obs_graphics::gif_frame::FrameArea;
use proptest::prelude::*;

use super::lzw::Stream;

#[test]
fn layout() {
    let values = [
        size_of::<CCanvas>(),
        core::mem::offset_of!(CCanvas, width),
        core::mem::offset_of!(CCanvas, height),
        core::mem::offset_of!(CCanvas, rowspan),
        core::mem::offset_of!(CCanvas, background),
        size_of::<FrameArea>(),
        core::mem::offset_of!(FrameArea, rect),
        core::mem::offset_of!(FrameArea, interlaced),
        core::mem::offset_of!(FrameArea, display),
        core::mem::offset_of!(FrameArea, transparency),
        core::mem::offset_of!(FrameArea, transparency_index),
    ];
    for (which, (name, value)) in c::LAYOUT_NAMES.iter().zip(values).enumerate() {
        // SAFETY: a pure C function.
        assert_eq!(
            unsafe { c::oracle_gif_frame_layout(which) },
            value,
            "{name}"
        );
    }
}

#[derive(Debug, Clone)]
struct Case {
    width: u32,
    height: u32,
    rowspan: u32,
    background: u32,
    frame: FrameArea,
    data: Vec<u8>,
    data_pos: usize,
    complete: bool,
    colours: Vec<u32>,
    fill: u32,
}

fn frame_area() -> impl Strategy<Value = FrameArea> {
    (
        prop_oneof![0u32..30, any::<u16>().prop_map(u32::from)],
        prop_oneof![0u32..30, any::<u16>().prop_map(u32::from)],
        prop_oneof![0u32..30, any::<u16>().prop_map(u32::from)],
        prop_oneof![0u32..30, any::<u16>().prop_map(u32::from)],
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        prop_oneof![0u32..=255, 256u32..300, Just(u32::MAX)],
    )
        .prop_map(
            |(x, y, w, h, interlaced, display, transparency, transparency_index)| FrameArea {
                rect: Rect {
                    x0: x,
                    y0: y,
                    x1: x + w,
                    y1: y + h,
                },
                interlaced,
                display,
                transparency,
                transparency_index,
            },
        )
}

fn image_data() -> impl Strategy<Value = (Vec<u8>, usize)> {
    let stream = (
        prop_oneof![Just(8u32), 2u32..=8],
        proptest::collection::vec(any::<u8>(), 0..700),
        any::<bool>(),
        1usize..256,
        any::<proptest::sample::Index>(),
        any::<bool>(),
    )
        .prop_map(|(min, idx, eoi, block, cut, do_cut)| {
            let mut st = Stream::new(min);
            st.clear();
            for i in idx {
                st.code(u32::from(i) & ((1 << min) - 1));
            }
            if eoi {
                st.eoi();
            }
            let mut data = [vec![min as u8], st.pack(block)].concat();
            if do_cut {
                let len = cut.index(data.len()) + 1;
                data.truncate(len);
            }
            data
        });
    let data = prop_oneof![3 => stream, 1 => proptest::collection::vec(any::<u8>(), 1..60)];
    (proptest::collection::vec(any::<u8>(), 0..5), data).prop_map(|(prefix, data)| {
        let pos = prefix.len();
        ([prefix, data].concat(), pos)
    })
}

fn case() -> impl Strategy<Value = Case> {
    (
        1u32..24,
        1u32..24,
        0u32..4,
        any::<u32>(),
        frame_area(),
        image_data(),
        any::<bool>(),
        proptest::collection::vec(any::<u32>(), MAX_COLOURS),
        any::<u32>(),
        any::<bool>(),
    )
        .prop_map(
            |(
                width,
                height,
                extra,
                background,
                mut frame,
                (data, data_pos),
                complete,
                colours,
                fill,
                full,
            )| {
                // Often a full-width frame at x 0: the simple path without
                // extra row span, the complex one with it.
                let rowspan = width + extra;
                if full && !frame.interlaced {
                    frame.rect.x0 = 0;
                    frame.rect.x1 = width;
                }
                Case {
                    width,
                    height,
                    rowspan,
                    background,
                    frame,
                    data,
                    data_pos,
                    complete,
                    colours,
                    fill,
                }
            },
        )
}

fn pixels_of(case: &Case) -> Vec<u32> {
    vec![case.fill; (case.width.max(case.rowspan) * case.height) as usize]
}

fn c_canvas(case: &Case, bitmap: &mut [u32]) -> CCanvas {
    CCanvas {
        bitmap: bitmap.as_mut_ptr(),
        width: case.width,
        height: case.height,
        rowspan: case.rowspan,
        background: case.background,
    }
}

/// The Rust and the C decode of one case: (result, bitmap) for each.
fn decode_both(case: &Case) -> ((i32, Vec<u32>), (i32, Vec<u32>)) {
    let (mut rust_px, mut c_px) = (pixels_of(case), pixels_of(case));
    let rust_canvas = c_canvas(case, &mut rust_px);
    let oracle_canvas = c_canvas(case, &mut c_px);
    let mut rust_lzw: *mut LzwCtx = ptr::null_mut();
    let mut c_ctx: *mut c_void = ptr::null_mut();
    // SAFETY: valid out-pointers; live contexts; the canvases point to
    // bitmaps of max(width, rowspan) * height pixels; the data and the
    // colour table are live for the calls.
    let (r, cr) = unsafe {
        lzw_context_create(&mut rust_lzw);
        c_lzw::oracle_lzw_context_create(&mut c_ctx);
        c_lzw::oracle_lzw_context_fill(c_ctx, 0);
        let r = nsgif_frame_pixels_decode(
            rust_lzw,
            case.data.as_ptr(),
            case.data.len(),
            case.data_pos,
            &rust_canvas,
            &case.frame,
            case.colours.as_ptr(),
            case.complete,
        );
        let cr = c::oracle_nsgif_frame_pixels_decode(
            c_ctx,
            case.data.as_ptr(),
            case.data.len(),
            case.data_pos,
            (&raw const oracle_canvas).cast(),
            (&raw const case.frame).cast(),
            case.colours.as_ptr(),
            case.complete,
        );
        lzw_context_destroy(rust_lzw);
        c_lzw::oracle_lzw_context_destroy(c_ctx);
        (r, cr)
    };
    ((r as i32, rust_px), (cr, c_px))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn decode(case in case()) {
        let ((r, rust_px), (cr, c_px)) = decode_both(&case);
        prop_assert_eq!(r, cr);
        prop_assert_eq!(rust_px, c_px);
    }

    #[test]
    fn restore_and_recover(case in case(), whole in any::<bool>(), saved_fill in any::<u32>()) {
        let (mut rust_px, mut c_px) = (pixels_of(&case), pixels_of(&case));
        let rust_canvas = c_canvas(&case, &mut rust_px);
        let oracle_canvas = c_canvas(&case, &mut c_px);
        let frame: *const FrameArea = if whole { ptr::null() } else { &case.frame };
        // SAFETY: as in decode_both; frame null (whole image) or live.
        unsafe {
            nsgif_frame_restore_bg(&rust_canvas, frame);
            c::oracle_nsgif_frame_restore_bg((&raw const oracle_canvas).cast(), frame.cast());
        }
        prop_assert_eq!(&rust_px, &c_px);

        let saved: Vec<u32> = (0..case.width * case.height).map(|i| saved_fill ^ i).collect();
        // SAFETY: as above; `saved` has width * height pixels.
        unsafe {
            nsgif_frame_recover(&rust_canvas, saved.as_ptr());
            c::oracle_nsgif_frame_recover((&raw const oracle_canvas).cast(), saved.as_ptr());
        }
        prop_assert_eq!(rust_px, c_px);
    }
}

// ---------------------------------------------------------------------------
// The intentional differences, on the shim alone.

#[test]
fn shim_null_pointers() {
    let mut px = [7u32; 4];
    let canvas = CCanvas {
        bitmap: px.as_mut_ptr(),
        width: 2,
        height: 2,
        rowspan: 2,
        background: 1,
    };
    let frame = FrameArea {
        rect: Rect {
            x0: 0,
            y0: 0,
            x1: 2,
            y1: 2,
        },
        display: true,
        ..FrameArea::default()
    };
    let colours = [0u32; MAX_COLOURS];
    let data = [8u8, 0];
    let mut lzw: *mut LzwCtx = ptr::null_mut();
    // SAFETY: every non-null pointer is valid; the nulls are the test.
    unsafe {
        lzw_context_create(&mut lzw);
        assert_eq!(
            nsgif_frame_pixels_decode(
                ptr::null_mut(),
                data.as_ptr(),
                2,
                0,
                &canvas,
                &frame,
                colours.as_ptr(),
                false
            ),
            NsgifError::Data
        );
        assert_eq!(
            nsgif_frame_pixels_decode(
                lzw,
                data.as_ptr(),
                2,
                0,
                ptr::null(),
                &frame,
                colours.as_ptr(),
                false
            ),
            NsgifError::Data
        );
        assert_eq!(
            nsgif_frame_pixels_decode(
                lzw,
                data.as_ptr(),
                2,
                0,
                &canvas,
                ptr::null(),
                colours.as_ptr(),
                false
            ),
            NsgifError::Data
        );
        assert_eq!(
            nsgif_frame_pixels_decode(
                lzw,
                data.as_ptr(),
                2,
                0,
                &canvas,
                &frame,
                ptr::null(),
                false
            ),
            NsgifError::Data
        );
        // A null data pointer is empty data: the end of the data.
        assert_eq!(
            nsgif_frame_pixels_decode(
                lzw,
                ptr::null(),
                2,
                0,
                &canvas,
                &frame,
                colours.as_ptr(),
                false
            ),
            NsgifError::EndOfData
        );
        nsgif_frame_restore_bg(ptr::null(), &frame);
        nsgif_frame_recover(ptr::null(), px.as_ptr());
        nsgif_frame_recover(&canvas, ptr::null());
        let empty = CCanvas {
            bitmap: ptr::null_mut(),
            ..canvas
        };
        nsgif_frame_restore_bg(&empty, ptr::null());
        lzw_context_destroy(lzw);
    }
    assert_eq!(px, [7; 4]);
}
