//! Safe core of `libobs/graphics/graphics-ffmpeg-route.c.h`: the pixel route
//! of `graphics-ffmpeg.c`. `ffmpeg_image_reformat_frame()` turns every
//! decoded image (PNG, JPEG, WebP, BMP, ...) into the texture data of the
//! Image Source, the Image Slide Show and the other users of `gs_image_file`:
//!
//! - [`plan`] (`gs_ffmpeg_image_plan()`): which way the frame goes (a
//!   straight copy, the premultiplied rows, the 16-bit rows or swscale),
//!   which premultiply runs, the format swscale is given and whether it is
//!   full range ([`sws_format`]), and the pixel format afterwards;
//! - [`color_format`] (`gs_ffmpeg_image_color_format()`): the texture format
//!   of that pixel format;
//! - [`copy_rows`] (`gs_ffmpeg_image_copy_rows()`): the straight copy, with
//!   [`copy_src_len`] the source bytes it reads.
//!
//! `graphics-ffmpeg.c` keeps every FFmpeg and swscale call and their logs,
//! every allocation, the premultiply loops, the swscale output copy and the
//! format stores. These keep no state and call nothing.
//!
//! The copy keeps the C's layout: each of the `cy` rows gets the smaller of
//! `cx * 4` and `src_linesize` bytes of its source line, `cx * 4` bytes
//! apart. So a line size shorter than `cx * 4` leaves the end of each row
//! unwritten (characterized, not endorsed; FFmpeg never hands over such a
//! frame). The output is generic over [`DstByte`] so the C ABI can pass the
//! uninitialized buffer `bmalloc()` returns.
//!
//! The sizes come from the image file. Nothing here can panic on any input:
//! every read and write is bounds checked, and the copy stops at the first
//! row that does not fit in `src` or `dst` (the C reads or writes out of
//! bounds there; `graphics-ffmpeg.c` always passes buffers of the sizes it
//! computes).

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

pub use crate::image_rgba64::DstByte;

/// The pixel formats the route names, `enum AVPixelFormat`'s values
/// (`GS_FFMPEG_IMAGE_PIX_FMT_*` in `graphics-ffmpeg-route.h`).
pub mod pix_fmt {
    /// `AV_PIX_FMT_YUV420P`.
    pub const YUV420P: i32 = 0;
    /// `AV_PIX_FMT_YUV422P`.
    pub const YUV422P: i32 = 4;
    /// `AV_PIX_FMT_YUV444P`.
    pub const YUV444P: i32 = 5;
    /// `AV_PIX_FMT_YUV411P`.
    pub const YUV411P: i32 = 7;
    /// `AV_PIX_FMT_YUVJ420P`, full range (JPEG).
    pub const YUVJ420P: i32 = 12;
    /// `AV_PIX_FMT_YUVJ422P`, full range.
    pub const YUVJ422P: i32 = 13;
    /// `AV_PIX_FMT_YUVJ444P`, full range.
    pub const YUVJ444P: i32 = 14;
    /// `AV_PIX_FMT_RGBA`.
    pub const RGBA: i32 = 26;
    /// `AV_PIX_FMT_BGRA`.
    pub const BGRA: i32 = 28;
    /// `AV_PIX_FMT_YUV440P`.
    pub const YUV440P: i32 = 31;
    /// `AV_PIX_FMT_YUVJ440P`, full range.
    pub const YUVJ440P: i32 = 32;
    /// `AV_PIX_FMT_RGBA64BE`, 16 bits a channel (16-bit PNGs).
    pub const RGBA64BE: i32 = 104;
    /// `AV_PIX_FMT_BGR0`.
    pub const BGR0: i32 = 121;
    /// `AV_PIX_FMT_YUVJ411P`, full range.
    pub const YUVJ411P: i32 = 138;
}

/// The texture formats, `enum gs_color_format`'s values
/// (`GS_FFMPEG_IMAGE_GS_*`).
pub mod gs_format {
    /// `GS_RGBA`.
    pub const RGBA: i32 = 3;
    /// `GS_BGRX`.
    pub const BGRX: i32 = 4;
    /// `GS_BGRA`.
    pub const BGRA: i32 = 5;
    /// `GS_RGBA16`.
    pub const RGBA16: i32 = 7;
}

/// The alpha modes, `enum gs_image_alpha_mode`'s values
/// (`GS_FFMPEG_IMAGE_ALPHA_*`).
pub mod alpha_mode {
    /// `GS_IMAGE_ALPHA_STRAIGHT`.
    pub const STRAIGHT: i32 = 0;
    /// `GS_IMAGE_ALPHA_PREMULTIPLY_SRGB`.
    pub const PREMULTIPLY_SRGB: i32 = 1;
    /// `GS_IMAGE_ALPHA_PREMULTIPLY`.
    pub const PREMULTIPLY: i32 = 2;
}

/// Bytes of one 8-bit texel.
pub const TEXEL_BYTES: usize = 4;

/// Which way a decoded frame goes (`GS_FFMPEG_IMAGE_ROUTE_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// Copied as it is ([`copy_rows`]).
    Copy,
    /// The rows premultiplied into the output (8-bit RGBA or BGRA).
    PremultiplyRows,
    /// The 16-bit rows (`gs_ffmpeg_image_rgba64be_rows()`).
    Rgba64,
    /// swscale to BGRA, then premultiplied over the whole output.
    Sws,
}

impl Route {
    /// The `GS_FFMPEG_IMAGE_ROUTE_*` value.
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::Copy => 0,
            Self::PremultiplyRows => 1,
            Self::Rgba64 => 2,
            Self::Sws => 3,
        }
    }
}

/// What the premultiplied rows and the swscale route run
/// (`GS_FFMPEG_IMAGE_PREMULTIPLY_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Premultiply {
    /// Nothing.
    None,
    /// `gs_premultiply_xyza_srgb_*`: in linear light, back to sRGB.
    Srgb,
    /// `gs_premultiply_xyza_*`: as stored.
    Linear,
}

impl Premultiply {
    /// The `GS_FFMPEG_IMAGE_PREMULTIPLY_*` value.
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Srgb => 1,
            Self::Linear => 2,
        }
    }
}

/// `struct gs_ffmpeg_image_plan`: the route of one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Which way the frame goes.
    pub route: Route,
    /// What [`Route::PremultiplyRows`] and [`Route::Sws`] run.
    pub premultiply: Premultiply,
    /// The pixel format swscale is given ([`sws_format`]).
    pub sws_format: i32,
    /// Whether swscale is told the input is full range.
    pub full_range: bool,
    /// The pixel format of the output.
    pub out_format: i32,
}

/// `ffmpeg_image_sws_format()`: the pixel format swscale is given for
/// `pix_fmt`, and whether it is full range. The five full-range YUVJ
/// formats are their YUV twins, full range (swscale logs "deprecated pixel
/// format used" for a YUVJ format, #2334); any other format is itself.
#[must_use]
pub fn sws_format(pix_fmt: i32) -> (i32, bool) {
    let _ = pix_fmt;
    todo!()
}

/// `gs_ffmpeg_image_plan()`: the route of a frame of `pix_fmt` loaded for
/// `alpha_mode`.
#[must_use]
pub fn plan(pix_fmt: i32, alpha_mode: i32) -> Plan {
    let _ = (pix_fmt, alpha_mode);
    todo!()
}

/// `gs_ffmpeg_image_color_format()` (`convert_format()`): the texture
/// format of the output pixel format `pix_fmt`.
#[must_use]
pub fn color_format(pix_fmt: i32) -> i32 {
    let _ = pix_fmt;
    todo!()
}

/// The source bytes [`copy_rows`] reads for a `cy`-row image: every line
/// but the last, then the last row's bytes. 0 when it reads nothing (no
/// rows, or no bytes in a row); `None` when the size overflows.
#[must_use]
pub fn copy_src_len(cx: usize, cy: usize, src_linesize: usize) -> Option<usize> {
    let _ = (cx, cy, src_linesize);
    todo!()
}

/// `gs_ffmpeg_image_copy_rows()`: copies the `cy` rows of `src`,
/// `src_linesize` bytes apart, into `dst`, `cx * 4` bytes apart: the
/// smaller of the two line sizes from each row. Stops at the first row
/// that does not fit in `src` or `dst`. Returns the rows copied.
pub fn copy_rows<D: DstByte>(
    dst: &mut [D],
    src: &[u8],
    cx: usize,
    cy: usize,
    src_linesize: usize,
) -> usize {
    let _ = (dst, src, cx, cy, src_linesize);
    todo!()
}
