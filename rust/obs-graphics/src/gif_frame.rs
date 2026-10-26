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
use crate::lzw::{Decoder, LzwResult};

/// `NSGIF_TRANSPARENT_COLOUR`.
const TRANSPARENT_COLOUR: u32 = 0;

/// `nsgif__error_from_lzw`. `LZW_BAD_PARAM` and `LZW_NO_COLOUR`, which the
/// C asserts against, are `NSGIF_OK` there without assertions (zeroed
/// entries of its table).
fn error_from_lzw(res: LzwResult) -> NsgifError {
    match res {
        LzwResult::Ok | LzwResult::BadParam | LzwResult::NoColour => NsgifError::Ok,
        LzwResult::NoMem => NsgifError::Oom,
        LzwResult::OkEod | LzwResult::NoData => NsgifError::EndOfData,
        LzwResult::EoiCode | LzwResult::BadIcode | LzwResult::BadCode => NsgifError::DataFrame,
    }
}

/// The result of an LZW call that ends a frame early: the end of the
/// frame's data (`LZW_OK_EOD`, `LZW_EOI_CODE`) is a success.
fn early_end(res: LzwResult) -> NsgifError {
    match res {
        LzwResult::OkEod | LzwResult::EoiCode => NsgifError::Ok,
        other => error_from_lzw(other),
    }
}

/// `gif__clip`: how far a frame passes the image in one dimension.
fn clip(frame_off: u32, frame_dim: u32, image_ext: u32) -> u32 {
    let frame_ext = frame_off.wrapping_add(frame_dim);
    if frame_ext <= image_ext {
        0
    } else {
        frame_ext.wrapping_sub(image_ext)
    }
}

/// `gif__jump_data`: skip decoded values of clipped columns.
fn jump_data(skip: &mut u32, available: &mut u32, pos: &mut usize) {
    let jump = (*skip).min(*available);
    *skip = skip.wrapping_sub(jump);
    *available = available.wrapping_sub(jump);
    *pos = pos.wrapping_add(jump as usize);
}

/// `nsgif__deinterlace`: the next row of the four interlace passes; `step`
/// starts at 24 (the first two passes both step by 8).
fn deinterlace(height: u32, y: &mut u32, step: &mut u8) -> bool {
    *y = y.wrapping_add(u32::from(*step & 0xf));
    if *y < height {
        return true;
    }
    // The passes after the current one, each tried in turn.
    let passes: &[(u32, u8)] = match *step {
        24 => &[(4, 8), (2, 4), (1, 2)],
        8 => &[(2, 4), (1, 2)],
        4 => &[(1, 2)],
        _ => &[],
    };
    for &(start, next) in passes {
        *y = start;
        *step = next;
        if *y < height {
            return true;
        }
    }
    false
}

/// `nsgif__next_row`.
fn next_row(interlace: bool, height: u32, y: &mut u32, step: &mut u8) -> bool {
    if interlace {
        deinterlace(height, y, step)
    } else {
        *y = y.wrapping_add(1);
        *y != height
    }
}

/// What both decode paths share.
struct Path<'a> {
    lzw: &'a mut Decoder,
    data: &'a [u8],
    data_pos: usize,
    canvas: &'a Canvas,
    colour_table: &'a [u32; MAX_COLOURS],
}

impl Path<'_> {
    /// The minimum code size at `data_pos`, read once the frame is known
    /// to cover pixels, as the C reads `data[0]`.
    fn minimum_code_size(&self) -> Option<u8> {
        self.data.get(self.data_pos).copied()
    }

    /// `nsgif__decode_simple`: whole rows through `lzw_decode_map`.
    fn simple(
        self,
        pixels: &mut [u32],
        mut height: u32,
        offset_y: u32,
        transparency_index: u32,
    ) -> NsgifError {
        let canvas = self.canvas;
        if offset_y >= canvas.height {
            return NsgifError::Ok;
        }
        height = height.wrapping_sub(clip(offset_y, height, canvas.height));
        if height == 0 {
            return NsgifError::Ok;
        }

        let Some(min) = self.minimum_code_size() else {
            return NsgifError::EndOfData;
        };
        let res = self.lzw.init_map(
            self.data,
            min,
            transparency_index,
            self.data_pos.wrapping_add(1),
        );
        if res != LzwResult::Ok {
            return error_from_lzw(res);
        }

        let mut at = offset_y.wrapping_mul(canvas.width) as usize;
        let mut left = canvas.width.wrapping_mul(height);
        let mut ret = NsgifError::Ok;
        while left > 0 {
            let out = pixels.get_mut(at..).unwrap_or_default();
            let len = out.len().min(left as usize);
            let Some(out) = out.get_mut(..len).filter(|o| !o.is_empty()) else {
                // Past the pixel slice (the C would write past the bitmap).
                break;
            };
            let (res, written) = self.lzw.decode_map(self.data, Some(self.colour_table), out);
            left = left.wrapping_sub(written);
            at = at.wrapping_add(written as usize);
            if res != LzwResult::Ok {
                ret = early_end(res);
                break;
            }
        }
        if left == 0 { NsgifError::Ok } else { ret }
    }

    /// `nsgif__decode_complex`: row by row through `lzw_decode`, clipped
    /// to the image and in the interlaced order.
    #[allow(clippy::too_many_arguments)]
    fn complex(
        self,
        pixels: &mut [u32],
        mut width: u32,
        mut height: u32,
        offset_x: u32,
        offset_y: u32,
        interlace: bool,
        transparency_index: u32,
    ) -> NsgifError {
        let canvas = self.canvas;
        let clip_x = clip(offset_x, width, canvas.width);
        let clip_y = clip(offset_y, height, canvas.height);
        if offset_x >= canvas.width || offset_y >= canvas.height {
            return NsgifError::Ok;
        }
        width = width.wrapping_sub(clip_x);
        height = height.wrapping_sub(clip_y);
        if width == 0 || height == 0 {
            return NsgifError::Ok;
        }

        let Some(min) = self.minimum_code_size() else {
            return NsgifError::EndOfData;
        };
        let mut res = self.lzw.init(self.data, min, self.data_pos.wrapping_add(1));
        if res != LzwResult::Ok {
            return error_from_lzw(res);
        }

        let (mut available, mut uncompressed, mut skip) = (0u32, 0usize, 0u32);
        let (mut y, mut step) = (0u32, 24u8);
        let transparent = (transparency_index <= 0xFF).then_some(transparency_index);
        loop {
            let mut at = offset_x
                .wrapping_add(y.wrapping_add(offset_y).wrapping_mul(canvas.rowspan))
                as usize;
            let mut x = width;
            while x > 0 {
                while available == 0 {
                    if res != LzwResult::Ok {
                        // Unexpected end of the frame: try to recover.
                        return early_end(res);
                    }
                    let (r, written) = self.lzw.decode(self.data);
                    res = r;
                    available = written;
                    uncompressed = 0;
                    if available == 0 {
                        return NsgifError::Ok;
                    }
                    jump_data(&mut skip, &mut available, &mut uncompressed);
                }

                let row = x.min(available);
                x = x.wrapping_sub(row);
                available = available.wrapping_sub(row);
                let values = self.lzw.output().get(uncompressed..).unwrap_or_default();
                let values = values.get(..row as usize).unwrap_or(values);
                let targets = pixels.get_mut(at..).unwrap_or_default();
                let n = values.len().min(targets.len());
                let (values, targets) = (
                    values.get(..n).unwrap_or_default(),
                    targets.get_mut(..n).unwrap_or_default(),
                );
                let colours = self.colour_table;
                // One branch per run of values, as the C.
                match transparent {
                    None => {
                        for (pixel, &value) in targets.iter_mut().zip(values) {
                            *pixel = colours.get(usize::from(value)).copied().unwrap_or(0);
                        }
                    }
                    Some(index) => {
                        for (pixel, &value) in targets.iter_mut().zip(values) {
                            if u32::from(value) != index {
                                *pixel = colours.get(usize::from(value)).copied().unwrap_or(0);
                            }
                        }
                    }
                }
                at = at.wrapping_add(row as usize);
                uncompressed = uncompressed.wrapping_add(row as usize);
            }

            skip = clip_x;
            jump_data(&mut skip, &mut available, &mut uncompressed);
            if !next_row(interlace, height, &mut y, &mut step) {
                return NsgifError::Ok;
            }
        }
    }
}

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
    let width = frame.rect.x1.wrapping_sub(frame.rect.x0);
    let height = frame.rect.y1.wrapping_sub(frame.rect.y0);
    let (offset_x, offset_y) = (frame.rect.x0, frame.rect.y0);
    let path = Path {
        lzw,
        data,
        data_pos,
        canvas,
        colour_table,
    };

    let ret =
        if !frame.interlaced && offset_x == 0 && width == canvas.width && width == canvas.rowspan {
            path.simple(pixels, height, offset_y, frame.transparency_index)
        } else {
            path.complex(
                pixels,
                width,
                height,
                offset_x,
                offset_y,
                frame.interlaced,
                frame.transparency_index,
            )
        };

    // All the data there is: make do.
    if data_complete && ret == NsgifError::EndOfData {
        NsgifError::Ok
    } else {
        ret
    }
}

/// `nsgif_frame_restore_bg`: the whole image to transparent (`None`), or
/// the frame's rectangle, clipped to the image, to transparent or to the
/// background colour.
pub fn restore_bg(canvas: &Canvas, pixels: &mut [u32], frame: Option<&FrameArea>) {
    let Some(frame) = frame else {
        let whole = (canvas.width as usize).saturating_mul(canvas.height as usize);
        for p in pixels.iter_mut().take(whole) {
            *p = TRANSPARENT_COLOUR;
        }
        return;
    };

    let (offset_x, offset_y) = (frame.rect.x0, frame.rect.y0);
    if !frame.display || offset_x >= canvas.width || offset_y >= canvas.height {
        return;
    }
    let mut width = frame.rect.x1.wrapping_sub(offset_x);
    let mut height = frame.rect.y1.wrapping_sub(offset_y);
    width = width.wrapping_sub(clip(offset_x, width, canvas.width));
    height = height.wrapping_sub(clip(offset_y, height, canvas.height));

    let colour = if frame.transparency {
        TRANSPARENT_COLOUR
    } else {
        canvas.background
    };
    for y in 0..height {
        // Rows by the image width, as the C (not the row span).
        let start =
            offset_x.wrapping_add(offset_y.wrapping_add(y).wrapping_mul(canvas.width)) as usize;
        if let Some(row) = pixels.get_mut(start..) {
            for p in row.iter_mut().take(width as usize) {
                *p = colour;
            }
        }
    }
}

/// `nsgif_frame_recover`: the saved frame's `width * height` pixels back.
pub fn recover(canvas: &Canvas, pixels: &mut [u32], prev_frame: &[u32]) {
    let whole = (canvas.width as usize).saturating_mul(canvas.height as usize);
    for (p, &saved) in pixels.iter_mut().zip(prev_frame).take(whole) {
        *p = saved;
    }
}
