//! `libobs/graphics/libnsgif/gif-frame.c.h` shims against the C oracle.
//! The fixture is one 1920x1080 frame through `nsgif_frame_pixels_decode`,
//! on the simple path (full width, not interlaced) and on the complex path
//! (the same frame interlaced). The C side decodes on the C LZW oracle, the
//! Rust side on the Rust decoder, as the hybrid libobs does.

use core::ffi::c_void;
use core::hint::black_box;
use core::ptr;

use criterion::Criterion;
use obs_c_oracle::gif_frame as c;
use obs_c_oracle::lzw as c_lzw;
use obs_graphics::ffi::gif_frame::{CCanvas, nsgif_frame_pixels_decode};
use obs_graphics::ffi::lzw::{LzwCtx, lzw_context_create, lzw_context_destroy};
use obs_graphics::gif_blocks::Rect;
use obs_graphics::gif_frame::FrameArea;

use crate::pair;

const W: u32 = 1920;
const H: u32 = 1080;

pub fn benchmarks(criterion: &mut Criterion) {
    // The 1080p code stream of the LZW bench, with its minimum code size.
    let data = [vec![8u8], crate::lzw::stream()].concat();
    let colours: Vec<u32> = (0..256u32)
        .map(|i| 0xff00_0000 | (i * 0x0001_0101))
        .collect();
    let mut rust_px = vec![0u32; (W * H) as usize];
    let mut c_px = vec![0u32; (W * H) as usize];
    let mut rust_lzw: *mut LzwCtx = ptr::null_mut();
    let mut c_ctx: *mut c_void = ptr::null_mut();
    // SAFETY: valid out-pointers.
    unsafe {
        lzw_context_create(&mut rust_lzw);
        c_lzw::oracle_lzw_context_create(&mut c_ctx);
    }

    for (name, interlaced) in [
        ("gif_frame/decode_1080p_simple", false),
        ("gif_frame/decode_1080p_interlaced", true),
    ] {
        let frame = FrameArea {
            rect: Rect {
                x0: 0,
                y0: 0,
                x1: W,
                y1: H,
            },
            interlaced,
            display: true,
            transparency: false,
            transparency_index: 256,
        };
        pair(
            criterion,
            name,
            || {
                let canvas = CCanvas {
                    bitmap: rust_px.as_mut_ptr(),
                    width: W,
                    height: H,
                    rowspan: W,
                    background: 0,
                };
                // SAFETY: a live context, a W x H bitmap, 256 colours.
                let r = unsafe {
                    nsgif_frame_pixels_decode(
                        rust_lzw,
                        data.as_ptr(),
                        data.len(),
                        0,
                        &canvas,
                        &frame,
                        colours.as_ptr(),
                        true,
                    )
                };
                (r as i32, black_box(rust_px[(W * H - 1) as usize]))
            },
            || {
                let canvas = CCanvas {
                    bitmap: c_px.as_mut_ptr(),
                    width: W,
                    height: H,
                    rowspan: W,
                    background: 0,
                };
                // SAFETY: as above, through the C signature.
                let r = unsafe {
                    c::oracle_nsgif_frame_pixels_decode(
                        c_ctx,
                        data.as_ptr(),
                        data.len(),
                        0,
                        (&raw const canvas).cast(),
                        (&raw const frame).cast(),
                        colours.as_ptr(),
                        true,
                    )
                };
                (r, black_box(c_px[(W * H - 1) as usize]))
            },
        );
    }
    // SAFETY: from the create functions, not used afterwards.
    unsafe {
        lzw_context_destroy(rust_lzw);
        c_lzw::oracle_lzw_context_destroy(c_ctx);
    }
}
