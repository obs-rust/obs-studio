//! Safe core of `libobs/graphics/libnsgif/gif.c.h`: libnsgif's state
//! machine and public API (`nsgif.h`).
//!
//! A [`Gif`] scans GIF data as it arrives ([`Gif::data_scan`],
//! [`Gif::data_complete`]), keeps the frame table and the positions into
//! the data, picks the next frame to show ([`Gif::frame_prepare`]) and
//! decodes a frame into the client's bitmap ([`Gif::frame_decode`]), with
//! the disposal of the frame before it. The blocks are parsed by
//! [`crate::gif_blocks`], the pixels written by [`crate::gif_frame`] on the
//! LZW decoder of [`crate::lzw`]. The client's bitmaps come through
//! [`Bitmaps`], the C's `nsgif_bitmap_cb_vt`.
//!
//! Every call that reads the GIF data takes it, as the C reads the buffer
//! of the last scan: the caller passes the same data to the scans, the
//! decodes and [`Gif::local_palette`].
//!
//! Characterized, not endorsed (`test_nsgif_decode`), and kept as the C has
//! them:
//! - with play once, one more [`Gif::frame_prepare`] after the last frame
//!   (infinite delay) shows frame 0 again before
//!   [`NsgifError::AnimationEnd`].

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use core::ffi::CStr;

use crate::gif_blocks::{
    self, ColourLayout, FrameExt, Image, MAX_COLOURS, NsgifError, Rect, Screen, disposal,
};
use crate::gif_frame::{self, Canvas, FrameArea};
use crate::lzw::{Decoder, LZW_CODE_MAX};

/// `NSGIF_INFINITE`: the delay of a frame shown for good.
pub const INFINITE: u32 = u32::MAX;

/// `nsgif_bitmap_fmt_t`, in the C's order.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitmapFmt {
    R8G8B8A8 = 0,
    B8G8R8A8,
    A8R8G8B8,
    A8B8G8R8,
    /// `RGBA8888`: a host-order format (`A8B8G8R8` on little endian).
    Rgba8888,
    /// `BGRA8888`: `A8R8G8B8` on little endian.
    Bgra8888,
    /// `ARGB8888`: `B8G8R8A8` on little endian.
    Argb8888,
    /// `ABGR8888`: `R8G8B8A8` on little endian.
    Abgr8888,
}

/// `nsgif_info_t`. The C layout: `nsgif_get_info` returns a pointer to it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub frame_count: u32,
    pub loop_max: i32,
    pub background: u32,
    pub global_palette: bool,
}

/// `nsgif_frame_info_t`. The C layout: `nsgif_get_frame_info` returns a
/// pointer to it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameInfo {
    pub display: bool,
    pub transparency: bool,
    pub local_palette: bool,
    pub interlaced: bool,
    pub disposal: u8,
    pub delay: u32,
    pub rect: Rect,
}

/// `nsgif_bitmap_cb_vt`: the client's bitmaps. `create`, `destroy` and
/// `pixels` are required (the C asserts them); the others are optional, as
/// NULL callbacks are in the C.
pub trait Bitmaps {
    /// `nsgif_bitmap_t *`.
    type Bitmap: Copy;

    /// `create`: a bitmap for `width` by `height` pixels; `None` is out of
    /// memory.
    fn create(&mut self, width: u32, height: u32) -> Option<Self::Bitmap>;

    /// `destroy`.
    fn destroy(&mut self, bitmap: Self::Bitmap);

    /// `get_buffer`: the bitmap's pixels, `max(width, rowspan) * height`
    /// of them for the image of `width` by `height`.
    fn pixels(&mut self, bitmap: Self::Bitmap, width: u32, height: u32, rowspan: u32)
    -> &mut [u32];

    /// `get_rowspan`; `None` is a NULL callback (the image width).
    fn rowspan(&mut self, bitmap: Self::Bitmap) -> Option<u32> {
        let _ = bitmap;
        None
    }

    /// `set_opaque`.
    fn set_opaque(&mut self, bitmap: Self::Bitmap, opaque: bool) {
        let _ = (bitmap, opaque);
    }

    /// `test_opaque`; `None` is a NULL callback (not opaque).
    fn test_opaque(&mut self, bitmap: Self::Bitmap) -> Option<bool> {
        let _ = bitmap;
        None
    }

    /// `modified`.
    fn modified(&mut self, bitmap: Self::Bitmap) {
        let _ = bitmap;
    }
}

/// `NSGIF_FRAME_INVALID`.
const FRAME_INVALID: u32 = u32::MAX;
/// `NSGIF_NO_TRANSPARENCY`.
const NO_TRANSPARENCY: u32 = 0xFFFF_FFFF;
/// `NSGIF_PROCESS_COLOURS`: the global table's first entry until the table
/// is read (no decoded entry has this value).
const PROCESS_COLOURS: u32 = 0xaa00_0000;
/// `NSGIF_TRAILER`.
const TRAILER: u8 = 0x3b;
/// `NSGIF_COLOUR_TABLE_MASK` and `NSGIF_COLOUR_TABLE_SIZE_MASK`.
const COLOUR_TABLE_MASK: u32 = 0x80;
const COLOUR_TABLE_SIZE_MASK: u32 = 0x07;
/// `NSGIF_FRAME_DELAY_MIN`, `NSGIF_FRAME_DELAY_DEFAULT`.
const FRAME_DELAY_MIN: u16 = 2;
const FRAME_DELAY_DEFAULT: u16 = 10;

/// `nsgif_frame`.
#[derive(Debug, Clone, Default)]
struct Frame {
    info: FrameInfo,
    frame_offset: usize,
    decoded: bool,
    opaque: bool,
    redraw_required: bool,
    lzw_data_length: u32,
    transparency_index: u32,
    colour_table_offset: u32,
    flags: u32,
}

impl Frame {
    /// A new frame holder (`nsgif__get_frame`); the C leaves the rest of the
    /// holder as it found it, which no path reads before setting.
    fn new(frame_offset: usize) -> Self {
        Self {
            info: FrameInfo {
                delay: 10,
                ..FrameInfo::default()
            },
            frame_offset,
            transparency_index: NO_TRANSPARENCY,
            ..Self::default()
        }
    }

    fn area(&self) -> FrameArea {
        FrameArea {
            rect: self.info.rect,
            interlaced: self.info.interlaced,
            display: self.info.display,
            transparency: self.info.transparency,
            transparency_index: self.transparency_index,
        }
    }
}

/// Which table `gif->colour_table` points to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Table {
    Global,
    Local,
}

/// `nsgif__bitmap_fmt_to_colour_layout`.
fn colour_layout(fmt: u32) -> ColourLayout {
    let le = crate::platform::host_is_little_endian();
    let fmt = match fmt {
        4 => {
            if le {
                3
            } else {
                0
            }
        }
        5 => {
            if le {
                2
            } else {
                1
            }
        }
        6 => {
            if le {
                1
            } else {
                2
            }
        }
        7 => {
            if le {
                0
            } else {
                3
            }
        }
        other => other,
    };
    match fmt {
        1 => ColourLayout {
            b: 0,
            g: 1,
            r: 2,
            a: 3,
        },
        2 => ColourLayout {
            a: 0,
            r: 1,
            g: 2,
            b: 3,
        },
        3 => ColourLayout {
            a: 0,
            b: 1,
            g: 2,
            r: 3,
        },
        _ => ColourLayout {
            r: 0,
            g: 1,
            b: 2,
            a: 3,
        },
    }
}

/// `struct nsgif`.
pub struct Gif<B: Bitmaps> {
    bitmaps: B,
    info: Info,
    lzw: Option<Box<Decoder>>,
    frames: Vec<Frame>,
    frame: u32,
    decoded_frame: u32,
    frame_image: Option<B::Bitmap>,
    rowspan: u32,
    delay_min: u16,
    delay_default: u16,
    loop_count: i32,
    frame_count_partial: u32,
    data_complete: bool,
    buf_pos: usize,
    bg_index: u32,
    aspect_ratio: u32,
    colour_table_size: u32,
    colour_table: Table,
    colour_layout: ColourLayout,
    global_colour_table: [u32; MAX_COLOURS],
    local_colour_table: [u32; MAX_COLOURS],
    prev_frame: Option<Vec<u32>>,
    prev_index: u32,
}

impl<B: Bitmaps> Drop for Gif<B> {
    /// `nsgif_destroy`.
    fn drop(&mut self) {
        if let Some(image) = self.frame_image.take() {
            self.bitmaps.destroy(image);
        }
    }
}

impl<B: Bitmaps> Gif<B> {
    /// `nsgif_create`. `fmt` is a [`BitmapFmt`] value; any other value is
    /// [`BitmapFmt::R8G8B8A8`], as in the C.
    pub fn new(bitmaps: B, fmt: u32) -> Self {
        Self {
            bitmaps,
            info: Info::default(),
            lzw: None,
            frames: Vec::new(),
            frame: 0,
            decoded_frame: FRAME_INVALID,
            frame_image: None,
            rowspan: 0,
            delay_min: FRAME_DELAY_MIN,
            delay_default: FRAME_DELAY_DEFAULT,
            loop_count: 0,
            frame_count_partial: 0,
            data_complete: false,
            buf_pos: 0,
            bg_index: 0,
            aspect_ratio: 0,
            colour_table_size: 0,
            colour_table: Table::Global,
            colour_layout: colour_layout(fmt),
            global_colour_table: [0; MAX_COLOURS],
            local_colour_table: [0; MAX_COLOURS],
            prev_frame: None,
            prev_index: FRAME_INVALID,
        }
    }

    /// The client's bitmaps.
    pub fn bitmaps(&mut self) -> &mut B {
        &mut self.bitmaps
    }

    /// `nsgif_set_frame_delay_behaviour`.
    pub fn set_frame_delay_behaviour(&mut self, delay_min: u16, delay_default: u16) {
        self.delay_min = delay_min;
        self.delay_default = delay_default;
    }

    /// `nsgif__get_frame`: the holder of `idx`, made as needed.
    // The resize makes `idx` a valid index, so the index cannot panic.
    #[allow(clippy::indexing_slicing)]
    fn holder(&mut self, idx: usize) -> &mut Frame {
        if self.frames.len() <= idx {
            let offset = self.buf_pos;
            self.frames
                .resize_with(idx.wrapping_add(1), || Frame::new(offset));
        }
        &mut self.frames[idx]
    }

    /// `nsgif_data_scan`: everything that has arrived of the GIF.
    pub fn data_scan(&mut self, data: &[u8]) -> NsgifError {
        if self.data_complete {
            return NsgifError::DataComplete;
        }
        let mut pos = self.buf_pos;

        if self.buf_pos == 0 {
            // Start over (the C leaves an earlier bitmap behind; there is
            // none before the first frame).
            if let Some(image) = self.frame_image.take() {
                self.bitmaps.destroy(image);
            }
            self.frames.clear();
            self.info.frame_count = 0;
            self.frame_count_partial = 0;
            self.decoded_frame = FRAME_INVALID;
            self.frame = FRAME_INVALID;

            let ret = gif_blocks::header(data, &mut pos, false);
            if ret != NsgifError::Ok {
                return ret;
            }
            let mut screen = Screen::default();
            let ret = gif_blocks::screen(data, &mut pos, &mut screen);
            if ret != NsgifError::Ok {
                return ret;
            }
            self.info.width = screen.width;
            self.info.height = screen.height;
            self.info.global_palette = screen.global_palette;
            self.colour_table_size = screen.colour_table_size;
            self.bg_index = screen.bg_index;
            self.aspect_ratio = screen.aspect_ratio;
            self.info.loop_max = screen.loop_max;

            self.buf_pos = pos;
            gif_blocks::screen_size_quirk(&mut self.info.width, &mut self.info.height);
            if let Some(first) = self.global_colour_table.get_mut(0) {
                *first = PROCESS_COLOURS;
            }

            // No frame data: the 13-byte header and a trailer.
            if data.len() == self.buf_pos.wrapping_add(1) && data.get(pos) == Some(&TRAILER) {
                return NsgifError::Ok;
            }
        }

        if self.global_colour_table.first() == Some(&PROCESS_COLOURS) {
            if self.info.global_palette {
                let mut used = 0;
                let ret = gif_blocks::colour_table(
                    &mut self.global_colour_table,
                    self.colour_layout,
                    self.colour_table_size as usize,
                    data.get(pos..).unwrap_or_default(),
                    &mut used,
                    true,
                );
                if ret != NsgifError::Ok {
                    return ret;
                }
                pos = pos.wrapping_add(used);
                self.buf_pos = pos;
            } else {
                // Black and white.
                gif_blocks::colour_table_decode(
                    &mut self.global_colour_table,
                    self.colour_layout,
                    2,
                    &[0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF],
                );
                self.colour_table_size = 2;
            }
            let index = if self.info.global_palette && self.bg_index < self.colour_table_size {
                self.bg_index as usize
            } else {
                0
            };
            self.info.background = self.global_colour_table.get(index).copied().unwrap_or(0);
        }

        if self.lzw.is_none() {
            self.lzw = Some(Box::default());
        }

        let mut ret;
        loop {
            let frames = self.info.frame_count;
            ret = self.process_frame(data, frames, false);
            if self.info.frame_count <= frames {
                break;
            }
        }
        if ret == NsgifError::EndOfData && self.info.frame_count > 0 {
            NsgifError::Ok
        } else {
            ret
        }
    }

    /// `nsgif__process_frame`.
    fn process_frame(&mut self, data: &[u8], frame_idx: u32, decode: bool) -> NsgifError {
        let idx = frame_idx as usize;
        let frame = self.holder(idx);
        let mut pos;
        if decode {
            pos = frame.frame_offset;
            if !frame.info.display {
                return NsgifError::Ok;
            }
            if frame_idx > self.frame_count_partial {
                return NsgifError::EndOfData;
            }
            if frame_idx == self.decoded_frame {
                return NsgifError::Ok;
            }
        } else {
            pos = self.buf_pos;
            if pos < data.len() && data.get(pos) == Some(&TRAILER) {
                return NsgifError::Ok;
            }
        }

        let ret = self.parse_frame(data, idx, &mut pos, decode);
        if !decode {
            // At the end of the data inside the frame, the next scan starts
            // the frame again, not where this one stopped (#2515).
            self.buf_pos = if ret == NsgifError::EndOfData {
                self.holder(idx).frame_offset
            } else {
                pos
            };
        }
        ret
    }

    /// The four parse steps of `nsgif__process_frame`, to its cleanup.
    fn parse_frame(
        &mut self,
        data: &[u8],
        idx: usize,
        pos: &mut usize,
        decode: bool,
    ) -> NsgifError {
        // nsgif__parse_frame_extensions: the fields are written back also
        // when a later extension fails.
        let frame = self.holder(idx);
        let mut ext = FrameExt {
            delay: frame.info.delay,
            disposal: frame.info.disposal,
            transparency: frame.info.transparency,
            transparency_index: frame.transparency_index,
            redraw_required: frame.redraw_required,
        };
        let ret = gif_blocks::extensions(data, pos, !decode, &mut ext, &mut self.info.loop_max);
        let frame = self.holder(idx);
        frame.info.delay = ext.delay;
        frame.info.disposal = ext.disposal;
        frame.info.transparency = ext.transparency;
        frame.transparency_index = ext.transparency_index;
        frame.redraw_required = ext.redraw_required;
        if ret != NsgifError::Ok {
            return ret;
        }

        // nsgif__parse_image_descriptor.
        let first_frame = self.info.frame_count == 0;
        let mut image = Image::default();
        let ret = gif_blocks::image_descriptor(
            data,
            pos,
            !decode,
            first_frame,
            &mut image,
            &mut self.info.width,
            &mut self.info.height,
        );
        if ret != NsgifError::Ok {
            return ret;
        }
        if !decode {
            let frame = self.holder(idx);
            frame.flags = image.flags;
            frame.info.rect = image.rect;
            frame.info.interlaced = image.interlaced;
        }

        let ret = self.parse_colour_table(data, idx, pos, decode);
        if ret != NsgifError::Ok {
            return ret;
        }
        self.parse_image_data(data, idx, pos, decode)
    }

    /// `nsgif__parse_colour_table`.
    fn parse_colour_table(
        &mut self,
        data: &[u8],
        idx: usize,
        pos: &mut usize,
        decode: bool,
    ) -> NsgifError {
        let flags = self.holder(idx).flags;
        if flags & COLOUR_TABLE_MASK == 0 {
            self.colour_table = Table::Global;
            return NsgifError::Ok;
        }
        if !decode {
            self.holder(idx).colour_table_offset = *pos as u32;
        }
        let mut used = 0;
        let ret = gif_blocks::colour_table(
            &mut self.local_colour_table,
            self.colour_layout,
            2usize << (flags & COLOUR_TABLE_SIZE_MASK),
            data.get(*pos..).unwrap_or_default(),
            &mut used,
            decode,
        );
        if ret != NsgifError::Ok {
            return ret;
        }
        *pos = pos.wrapping_add(used);
        if decode {
            self.colour_table = Table::Local;
        } else {
            self.holder(idx).info.local_palette = true;
        }
        NsgifError::Ok
    }

    /// `nsgif__parse_image_data`.
    fn parse_image_data(
        &mut self,
        data: &[u8],
        idx: usize,
        pos: &mut usize,
        decode: bool,
    ) -> NsgifError {
        let rest = data.get(*pos..).unwrap_or_default();
        let frame_idx = idx as u32;
        if !decode {
            self.frame_count_partial = frame_idx.wrapping_add(1);
        }

        // A trailer, or a minimum code size and a trailer, is no image
        // data but not an error.
        match rest {
            [] => return NsgifError::EndOfData,
            [first] => {
                return if *first == TRAILER {
                    NsgifError::Ok
                } else {
                    NsgifError::EndOfData
                };
            }
            [first, second] => {
                return if *second == TRAILER || *first == TRAILER {
                    NsgifError::Ok
                } else {
                    NsgifError::EndOfData
                };
            }
            [first, ..] if *first == TRAILER => return NsgifError::Ok,
            _ => {}
        }

        let minimum_code_size = rest.first().copied().unwrap_or(0);
        if minimum_code_size >= LZW_CODE_MAX {
            return NsgifError::DataFrame;
        }

        if decode {
            return self.update_bitmap(data, idx, *pos);
        }

        // Skip the minimum code size, then the sub-blocks.
        let mut at = 1usize;
        let mut len = rest.len().wrapping_sub(1);
        let mut block_size = 0usize;
        while block_size != 1 {
            if len < 1 {
                return NsgifError::EndOfData;
            }
            block_size = usize::from(rest.get(at).copied().unwrap_or(0)).wrapping_add(1);
            if block_size > len {
                let frame = self.holder(idx);
                frame.lzw_data_length = frame.lzw_data_length.wrapping_add(len as u32);
                return NsgifError::EndOfData;
            }
            len = len.wrapping_sub(block_size);
            at = at.wrapping_add(block_size);
            let frame = self.holder(idx);
            frame.lzw_data_length = frame.lzw_data_length.wrapping_add(block_size as u32);
        }

        *pos = pos.wrapping_add(at);
        self.info.frame_count = frame_idx.wrapping_add(1);
        self.holder(idx).info.display = true;
        NsgifError::Ok
    }

    /// `nsgif__bitmap_get`: the bitmap, made on first use, and the row
    /// span.
    fn bitmap_get(&mut self) -> Option<B::Bitmap> {
        if self.frame_image.is_none() {
            self.frame_image = self.bitmaps.create(self.info.width, self.info.height);
        }
        let image = self.frame_image?;
        self.rowspan = self.info.width;
        if let Some(rowspan) = self.bitmaps.rowspan(image) {
            self.rowspan = rowspan;
        }
        Some(image)
    }

    fn canvas(&self) -> Canvas {
        Canvas {
            width: self.info.width,
            height: self.info.height,
            rowspan: self.rowspan,
            background: self.info.background,
        }
    }

    /// `nsgif__restore_bg`.
    fn restore_bg(&mut self, image: B::Bitmap, frame: Option<FrameArea>) {
        let canvas = self.canvas();
        let pixels = self
            .bitmaps
            .pixels(image, canvas.width, canvas.height, canvas.rowspan);
        gif_frame::restore_bg(&canvas, pixels, frame.as_ref());
    }

    /// `nsgif__record_frame`: save the bitmap for a later restore.
    fn record_frame(&mut self) {
        if self.decoded_frame == FRAME_INVALID || self.decoded_frame == self.prev_index {
            return;
        }
        let Some(image) = self.bitmap_get() else {
            return;
        };
        let canvas = self.canvas();
        let size = (canvas.width as usize).saturating_mul(canvas.height as usize);
        let saved = self.prev_frame.get_or_insert_with(|| vec![0; size]);
        let pixels = self
            .bitmaps
            .pixels(image, canvas.width, canvas.height, canvas.rowspan);
        for (s, &p) in saved.iter_mut().zip(pixels.iter()).take(size) {
            *s = p;
        }
        self.prev_index = self.decoded_frame;
    }

    /// `nsgif__update_bitmap`: dispose of the previous frame, decode this
    /// one, tell the client.
    fn update_bitmap(&mut self, data: &[u8], idx: usize, data_pos: usize) -> NsgifError {
        let frame_idx = idx as u32;
        self.decoded_frame = frame_idx;

        let Some(image) = self.bitmap_get() else {
            return NsgifError::Oom;
        };

        if frame_idx == 0 || self.decoded_frame == FRAME_INVALID {
            self.restore_bg(image, None);
        } else {
            let prev = self.holder(idx.wrapping_sub(1));
            let (prev_disposal, prev_area) = (prev.info.disposal, prev.area());
            if prev_disposal == disposal::RESTORE_BG {
                self.restore_bg(image, Some(prev_area));
            } else if prev_disposal == disposal::RESTORE_PREV {
                // nsgif__recover_frame (it cannot fail).
                let canvas = self.canvas();
                let pixels =
                    self.bitmaps
                        .pixels(image, canvas.width, canvas.height, canvas.rowspan);
                gif_frame::recover(
                    &canvas,
                    pixels,
                    self.prev_frame.as_deref().unwrap_or_default(),
                );
            }
        }

        if self.holder(idx).info.disposal == disposal::RESTORE_PREV {
            self.record_frame();
        }

        // nsgif__decode.
        let canvas = self.canvas();
        let area = self.holder(idx).area();
        let lzw = self.lzw.get_or_insert_with(Box::default);
        let table = match self.colour_table {
            Table::Global => &self.global_colour_table,
            Table::Local => &self.local_colour_table,
        };
        let pixels = self
            .bitmaps
            .pixels(image, canvas.width, canvas.height, canvas.rowspan);
        let ret = gif_frame::decode(
            lzw,
            data,
            data_pos,
            &canvas,
            pixels,
            &area,
            table,
            self.data_complete,
        );

        self.bitmaps.modified(image);
        let frame = self.holder(idx);
        if !frame.decoded {
            let opaque = self.bitmaps.test_opaque(image).unwrap_or(false);
            let frame = self.holder(idx);
            frame.opaque = opaque;
            frame.decoded = true;
        }
        let opaque = self.holder(idx).opaque;
        self.bitmaps.set_opaque(image, opaque);
        ret
    }

    /// `nsgif_data_complete`: no more data; the first frame with image data
    /// that the scan could not finish becomes displayable.
    pub fn data_complete(&mut self) {
        if !self.data_complete {
            let (start, end) = (self.info.frame_count, self.frame_count_partial);
            for f in start..end {
                let Some(frame) = self.frames.get_mut(f as usize) else {
                    break;
                };
                if frame.lzw_data_length > 0 {
                    frame.info.display = true;
                    if f == 0 {
                        frame.info.transparency = true;
                    }
                    self.info.frame_count = f.wrapping_add(1);
                    break;
                }
            }
        }
        self.data_complete = true;
    }

    /// `nsgif__frame_next`.
    fn frame_next(&self, partial: bool, frame: u32) -> u32 {
        let frames = if partial {
            self.frame_count_partial
        } else {
            self.info.frame_count
        };
        if frames == 0 {
            return FRAME_INVALID;
        }
        let next = frame.wrapping_add(1);
        if next >= frames { 0 } else { next }
    }

    /// `nsgif__next_displayable_frame`.
    fn next_displayable_frame(&self, frame: &mut u32, mut delay: Option<&mut u32>) -> NsgifError {
        let mut next = *frame;
        loop {
            next = self.frame_next(false, next);
            if next <= *frame && *frame != FRAME_INVALID && !self.data_complete {
                return NsgifError::EndOfData;
            } else if next == *frame || next == FRAME_INVALID {
                return NsgifError::FrameDisplay;
            }
            let info = self
                .frames
                .get(next as usize)
                .map(|f| f.info)
                .unwrap_or_default();
            if let Some(d) = delay.as_deref_mut() {
                *d = d.wrapping_add(info.delay);
            }
            if info.display {
                break;
            }
        }
        *frame = next;
        NsgifError::Ok
    }

    /// `nsgif__animation_complete`.
    fn animation_complete(count: i32, max: i32) -> bool {
        max != 0 && count >= max
    }

    /// `nsgif_reset`.
    pub fn reset(&mut self) -> NsgifError {
        self.loop_count = 0;
        self.frame = FRAME_INVALID;
        NsgifError::Ok
    }

    /// `nsgif_frame_prepare`: the area to redraw, the delay and the frame.
    pub fn frame_prepare(&mut self) -> Result<(Rect, u32, u32), NsgifError> {
        let mut rect = Rect::default();
        let mut delay = 0u32;
        let mut frame = self.frame;

        if self.frame != FRAME_INVALID
            && self.frame < self.info.frame_count
            && let Some(current) = self.frames.get(self.frame as usize)
            && current.info.display
        {
            rect = current.info.rect;
        }

        if Self::animation_complete(self.loop_count, self.info.loop_max) {
            return Err(NsgifError::AnimationEnd);
        }

        let ret = self.next_displayable_frame(&mut frame, Some(&mut delay));
        if ret != NsgifError::Ok {
            return Err(ret);
        }

        if self.frame != FRAME_INVALID && frame < self.frame {
            self.loop_count = self.loop_count.wrapping_add(1);
        }

        if self.data_complete {
            // The last frame has an infinite delay.
            if self.info.frame_count == 1 {
                delay = INFINITE;
            } else if self.info.loop_max != 0 {
                let mut frame_next = frame;
                let ret = self.next_displayable_frame(&mut frame_next, None);
                if ret != NsgifError::Ok {
                    return Err(ret);
                }
                if self.data_complete
                    && frame_next < frame
                    && Self::animation_complete(self.loop_count.wrapping_add(1), self.info.loop_max)
                {
                    delay = INFINITE;
                }
            }
        }

        self.frame = frame;
        let frame_rect = self
            .frames
            .get(frame as usize)
            .map(|f| f.info.rect)
            .unwrap_or_default();
        redraw_rect_extend(&frame_rect, &mut rect);

        if delay < u32::from(self.delay_min) {
            delay = u32::from(self.delay_default);
        }
        Ok((rect, delay, self.frame))
    }

    /// `nsgif_frame_decode`: decode `frame` (and the frames before it as
    /// needed) into the bitmap, which it returns (`None` before a bitmap
    /// exists, as the C returns NULL).
    pub fn frame_decode(
        &mut self,
        data: &[u8],
        frame: u32,
    ) -> Result<Option<B::Bitmap>, NsgifError> {
        if frame >= self.info.frame_count {
            return Err(NsgifError::BadFrame);
        }
        if self.decoded_frame == frame {
            return Ok(self.frame_image);
        }
        let start = if self.decoded_frame >= frame || self.decoded_frame == FRAME_INVALID {
            0
        } else {
            self.frame_next(false, self.decoded_frame)
        };
        for f in start..=frame {
            let ret = self.process_frame(data, f, true);
            if ret != NsgifError::Ok {
                return Err(ret);
            }
        }
        Ok(self.frame_image)
    }

    /// `nsgif_get_info`.
    pub fn info(&self) -> &Info {
        &self.info
    }

    /// `nsgif_get_frame_info`.
    pub fn frame_info(&self, frame: u32) -> Option<&FrameInfo> {
        if frame >= self.info.frame_count {
            return None;
        }
        self.frames.get(frame as usize).map(|f| &f.info)
    }

    /// `nsgif_global_palette`: the whole table, and its entry count.
    pub fn global_palette(&self, table: &mut [u32; MAX_COLOURS]) -> usize {
        *table = self.global_colour_table;
        self.colour_table_size as usize
    }

    /// `nsgif_local_palette`: the frame's local table into `table` and its
    /// entry count, or `None` without one.
    pub fn local_palette(
        &self,
        data: &[u8],
        frame: u32,
        table: &mut [u32; MAX_COLOURS],
    ) -> Option<usize> {
        if frame >= self.frame_count_partial {
            return None;
        }
        let f = self.frames.get(frame as usize)?;
        if !f.info.local_palette {
            return None;
        }
        let entries = 2usize << (f.flags & COLOUR_TABLE_SIZE_MASK);
        let at = f.colour_table_offset as usize;
        gif_blocks::colour_table_decode(
            table,
            self.colour_layout,
            entries,
            data.get(at..).unwrap_or_default(),
        );
        Some(entries)
    }
}

/// `nsgif__redraw_rect_extend`.
fn redraw_rect_extend(frame: &Rect, redraw: &mut Rect) {
    if redraw.x1 == 0 || redraw.y1 == 0 {
        *redraw = *frame;
    } else {
        redraw.x0 = redraw.x0.min(frame.x0);
        redraw.x1 = redraw.x1.max(frame.x1);
        redraw.y0 = redraw.y0.min(frame.y0);
        redraw.y1 = redraw.y1.max(frame.y1);
    }
}

/// `nsgif_strerror`.
pub fn strerror(err: u32) -> &'static CStr {
    match err {
        0 => c"Success",
        1 => c"Out of memory",
        2 => c"Invalid source data",
        3 => c"Requested frame does not exist",
        4 => c"Invalid frame data",
        5 => c"Unexpected end of GIF source data",
        6 => c"Can't add data to completed GIF",
        7 => c"Frame can't be displayed",
        8 => c"Animation complete",
        _ => c"Unknown error",
    }
}

/// `nsgif_str_disposal`.
pub fn str_disposal(disposal: u32) -> &'static CStr {
    match disposal {
        1 => c"None",
        2 => c"Restore background",
        3 => c"Restore previous",
        4 => c"Restore quirk",
        _ => c"Unspecified",
    }
}
