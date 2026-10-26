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
//! - when a scan stops at the end of the data inside a frame, the next scan
//!   resumes where the frame's parse stopped, not at the frame's start, and
//!   reads the rest of the frame as the next block (a bad frame from then
//!   on);
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

use crate::gif_blocks::{MAX_COLOURS, NsgifError, Rect};

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

/// `struct nsgif`.
pub struct Gif<B: Bitmaps> {
    bitmaps: B,
    info: Info,
}

impl<B: Bitmaps> Gif<B> {
    /// `nsgif_create`. `fmt` is a [`BitmapFmt`] value; any other value is
    /// [`BitmapFmt::R8G8B8A8`], as in the C.
    pub fn new(bitmaps: B, fmt: u32) -> Self {
        let _ = fmt;
        Self {
            bitmaps,
            info: Info::default(),
        }
    }

    /// The client's bitmaps.
    pub fn bitmaps(&mut self) -> &mut B {
        &mut self.bitmaps
    }

    /// `nsgif_set_frame_delay_behaviour`.
    pub fn set_frame_delay_behaviour(&mut self, delay_min: u16, delay_default: u16) {
        let _ = (delay_min, delay_default);
        todo!("set_frame_delay_behaviour")
    }

    /// `nsgif_data_scan`: everything that has arrived of the GIF.
    pub fn data_scan(&mut self, data: &[u8]) -> NsgifError {
        let _ = data;
        todo!("data_scan")
    }

    /// `nsgif_data_complete`.
    pub fn data_complete(&mut self) {
        todo!("data_complete")
    }

    /// `nsgif_reset`.
    pub fn reset(&mut self) -> NsgifError {
        todo!("reset")
    }

    /// `nsgif_frame_prepare`: the area to redraw, the delay and the frame.
    pub fn frame_prepare(&mut self) -> Result<(Rect, u32, u32), NsgifError> {
        todo!("frame_prepare")
    }

    /// `nsgif_frame_decode`: decode `frame` (and the frames before it as
    /// needed) into the bitmap, which it returns (`None` before a bitmap
    /// exists, as the C returns NULL).
    pub fn frame_decode(
        &mut self,
        data: &[u8],
        frame: u32,
    ) -> Result<Option<B::Bitmap>, NsgifError> {
        let _ = (data, frame);
        todo!("frame_decode")
    }

    /// `nsgif_get_info`.
    pub fn info(&self) -> &Info {
        &self.info
    }

    /// `nsgif_get_frame_info`.
    pub fn frame_info(&self, frame: u32) -> Option<&FrameInfo> {
        let _ = frame;
        todo!("frame_info")
    }

    /// `nsgif_global_palette`: the whole table, and its entry count.
    pub fn global_palette(&self, table: &mut [u32; MAX_COLOURS]) -> usize {
        let _ = table;
        todo!("global_palette")
    }

    /// `nsgif_local_palette`: the frame's local table into `table` and its
    /// entry count, or `None` without one.
    pub fn local_palette(
        &self,
        data: &[u8],
        frame: u32,
        table: &mut [u32; MAX_COLOURS],
    ) -> Option<usize> {
        let _ = (data, frame, table);
        todo!("local_palette")
    }
}

/// `nsgif_strerror`.
pub fn strerror(err: u32) -> &'static CStr {
    let _ = err;
    todo!("strerror")
}

/// `nsgif_str_disposal`.
pub fn str_disposal(disposal: u32) -> &'static CStr {
    let _ = disposal;
    todo!("str_disposal")
}
