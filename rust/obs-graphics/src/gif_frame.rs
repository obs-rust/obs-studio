//! Safe core of `libobs/graphics/libnsgif/gif-frame.c.h`: libnsgif's frame
//! pixel decode and disposal.
//!
//! [`decode`] writes the pixels of one frame into a [`Canvas`]: a
//! full-width frame at x 0, not interlaced, on a canvas whose row span is
//! its width goes through [`Decoder::decode_map`] (the C's simple path);
//! any other frame through [`Decoder::decode`] and the colour table, row by
//! row, clipped to the image and in the interlaced order (the complex
//! path). [`restore_bg`] and [`recover`] are the disposal steps before a
//! frame. `gif.c` keeps `struct nsgif`, the bitmap callbacks, the saved
//! frame's allocation and the order of the steps.
//!
//! Characterized, not endorsed (`test_nsgif_decode`), and kept as the C has
//! them: [`restore_bg`] and [`recover`] work on `width * height` pixels from
//! the start of the bitmap and step rows by the image width, not the row
//! span; a frame that starts past the image or is clipped to nothing is a
//! success that reads no data.
//!
//! Every write goes through a bounds check: a pixel the C would write
//! outside the canvas slice is not written.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::gif_blocks::{MAX_COLOURS, NsgifError, Rect};
use crate::lzw::Decoder;

/// `struct nsgif_canvas` without the bitmap pointer: the image size, the
/// row span and the background colour. The pixels are a separate slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub rowspan: u32,
    pub background: u32,
}

/// `struct nsgif_frame_area`: the frame fields the decode and the restore
/// read.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameArea {
    pub rect: Rect,
    pub interlaced: bool,
    pub display: bool,
    pub transparency: bool,
    /// The transparent colour index; above 0xFF for none.
    pub transparency_index: u32,
}

/// `nsgif_frame_pixels_decode`: decode one frame's image data, at
/// `data_pos` in `data` (the minimum code size), into `pixels`.
#[allow(clippy::too_many_arguments)]
pub fn decode(
    lzw: &mut Decoder,
    data: &[u8],
    data_pos: usize,
    canvas: &Canvas,
    pixels: &mut [u32],
    frame: &FrameArea,
    colour_table: &[u32; MAX_COLOURS],
    data_complete: bool,
) -> NsgifError {
    let _ = (
        lzw,
        data,
        data_pos,
        canvas,
        pixels,
        frame,
        colour_table,
        data_complete,
    );
    todo!("decode")
}

/// `nsgif_frame_restore_bg`: the whole image to transparent (`None`), or
/// the frame's rectangle, clipped to the image, to transparent or to the
/// background colour.
pub fn restore_bg(canvas: &Canvas, pixels: &mut [u32], frame: Option<&FrameArea>) {
    let _ = (canvas, pixels, frame);
    todo!("restore_bg")
}

/// `nsgif_frame_recover`: the saved frame's `width * height` pixels back.
pub fn recover(canvas: &Canvas, pixels: &mut [u32], prev_frame: &[u32]) {
    let _ = (canvas, pixels, prev_frame);
    todo!("recover")
}
