//! Safe core of `libobs/graphics/graphics-ffmpeg-rgba64.c.h`: the 16-bit
//! RGBA rows of `graphics-ffmpeg.c`. A 16-bit PNG decodes to
//! `AV_PIX_FMT_RGBA64BE`, and `ffmpeg_image_reformat_frame()` turns it into
//! the 8-bit RGBA texture data of the Image Source, the Image Slide Show and
//! the other users of `gs_image_file`.
//!
//! - [`rgba64be_rows`] (`gs_ffmpeg_image_rgba64be_rows()`): every row of
//!   the frame, each texel's four big-endian 16-bit channels to 8 bits,
//!   straight, premultiplied in sRGB, or premultiplied ([`alpha_mode`]);
//! - [`texel`]: one texel, as each row converts it;
//! - [`row_texels`] and [`src_len`]: how many texels a row converts, and
//!   how many source bytes the rows read.
//!
//! `graphics-ffmpeg.c` keeps the format route, the allocation of the output
//! and the format it stores afterwards. These keep no state and call
//! nothing but `f32::powf` (the platform's `powf`, as the C calls it).
//!
//! The arithmetic is the C's, operation for operation, in `f32`: each
//! channel divided by 65535 (not multiplied by a reciprocal), the sRGB
//! curves of `srgb.h` on the colour channels only, the colour premultiplied
//! by alpha, and `(uint8_t)(f * 255 + 0.5)`. Nothing is fused into a
//! multiply-add; the C oracle is built with `-ffp-contract=off`.
//!
//! The rows keep the C's layout too: a row converts the smaller of `cx` and
//! `src_linesize / 8` texels; the destination advances 4 bytes per texel
//! written, with no row step of its own; the source skips what is left of
//! its line after the bytes it read. So a line size shorter than `cx * 8`
//! packs the rows in the output, leaves its end unwritten and reads the
//! source as one run of texels, and an alpha mode outside the three writes
//! nothing (characterized, not endorsed; FFmpeg never hands over such a
//! frame). The output is generic over [`DstByte`] so the C ABI can pass the
//! uninitialized buffer `bmalloc()` returns.
//!
//! The sizes come from the image file. Nothing here can panic on any input:
//! every read and write is bounds checked, and the rows stop at the first
//! texel that does not fit in `src` or `dst` (the C reads or writes out of
//! bounds there; `graphics-ffmpeg.c` always passes buffers of the sizes it
//! computes).

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use core::mem::MaybeUninit;

/// The alpha modes, `enum gs_image_alpha_mode`'s values
/// (`GS_FFMPEG_RGBA64_ALPHA_*` in `graphics-ffmpeg-rgba64.h`).
pub mod alpha_mode {
    /// `GS_IMAGE_ALPHA_STRAIGHT`: the channels to 8 bits as they are.
    pub const STRAIGHT: i32 = 0;
    /// `GS_IMAGE_ALPHA_PREMULTIPLY_SRGB`: the colour to linear light,
    /// premultiplied by alpha, back to sRGB.
    pub const PREMULTIPLY_SRGB: i32 = 1;
    /// `GS_IMAGE_ALPHA_PREMULTIPLY`: the colour premultiplied by alpha as
    /// it is stored.
    pub const PREMULTIPLY: i32 = 2;
}

/// Bytes of one big-endian 16-bit RGBA texel in the source.
pub const SRC_TEXEL_BYTES: usize = 8;

/// Bytes of one 8-bit RGBA texel in the output.
pub const DST_TEXEL_BYTES: usize = 4;

mod sealed {
    pub trait Sealed {}
    impl Sealed for u8 {}
    impl Sealed for core::mem::MaybeUninit<u8> {}
}

/// A byte of the output: `u8`, or `MaybeUninit<u8>` for a buffer that is
/// not initialized (the C ABI's `bmalloc()` buffer). Only written, never
/// read.
pub trait DstByte: sealed::Sealed {
    /// Stores `value`.
    fn put(&mut self, value: u8);
}

impl DstByte for u8 {
    fn put(&mut self, value: u8) {
        *self = value;
    }
}

impl DstByte for MaybeUninit<u8> {
    fn put(&mut self, value: u8) {
        self.write(value);
    }
}

/// `(size_t)cx * 4 * 2`: the row's bytes in the source, wrapping as the
/// C's `size_t` does (a negative `cx` converts to a huge value first).
fn src_row_bytes(cx: i32, src_linesize: usize) -> usize {
    // `as` sign-extends, which is the C's int to size_t conversion.
    let width = (cx as isize).cast_unsigned();
    width
        .wrapping_mul(DST_TEXEL_BYTES)
        .wrapping_mul(2)
        .min(src_linesize)
}

/// The texels a row converts: the smaller of `cx * 8` bytes (as the C's
/// `size_t`, so a negative `cx` is a huge width) and `src_linesize`, in
/// 8-byte texels.
#[must_use]
pub fn row_texels(cx: i32, src_linesize: usize) -> usize {
    src_row_bytes(cx, src_linesize) >> 3
}

/// The source bytes [`rgba64be_rows`] reads for a `cy`-row frame: every
/// line but the last, then the last row's texels. 0 when it reads nothing
/// (no rows, or no texels in a row); `None` when the size overflows.
#[must_use]
pub fn src_len(cx: i32, cy: i32, src_linesize: usize) -> Option<usize> {
    let texels = row_texels(cx, src_linesize);
    let rows = usize::try_from(cy).unwrap_or(0);
    if rows == 0 || texels == 0 {
        return Some(0);
    }
    rows.checked_sub(1)?
        .checked_mul(src_linesize)?
        .checked_add(texels.checked_mul(SRC_TEXEL_BYTES)?)
}

/// `gs_srgb_nonlinear_to_linear()`.
fn srgb_nonlinear_to_linear(u: f32) -> f32 {
    if u <= 0.04045 {
        u / 12.92
    } else {
        ((u + 0.055) / 1.055).powf(2.4)
    }
}

/// `gs_srgb_linear_to_nonlinear()`. `1.0 / 2.4` is the C's `1.0f / 2.4f`,
/// an `f32` division.
fn srgb_linear_to_nonlinear(u: f32) -> f32 {
    if u <= 0.003_130_8 {
        12.92 * u
    } else {
        (1.055 * u.powf(1.0 / 2.4)) - 0.055
    }
}

/// `gs_float_to_u8()`: `(uint8_t)(f * 255.0f + 0.5f)`. Every value here is
/// in 0 to 255.5, where the C's conversion is defined and `as` truncates
/// the same way.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // the C's own conversion
fn float_to_u8(f: f32) -> u8 {
    (f * 255.0 + 0.5) as u8
}

/// The four channels of a big-endian texel, each divided by 65535.
fn to_float(be: [u8; SRC_TEXEL_BYTES]) -> [f32; 4] {
    let [r0, r1, g0, g1, b0, b1, a0, a1] = be;
    [[r0, r1], [g0, g1], [b0, b1], [a0, a1]].map(|c| f32::from(u16::from_be_bytes(c)) / 65535.0)
}

/// One texel in the alpha mode `MODE`: the C's loop body. An unknown mode
/// is never passed (the rows return before).
fn convert<const MODE: i32>(be: [u8; SRC_TEXEL_BYTES]) -> [u8; DST_TEXEL_BYTES] {
    let [mut r, mut g, mut b, a] = to_float(be);
    if MODE == alpha_mode::PREMULTIPLY_SRGB {
        r = srgb_nonlinear_to_linear(r);
        g = srgb_nonlinear_to_linear(g);
        b = srgb_nonlinear_to_linear(b);
    }
    if MODE == alpha_mode::PREMULTIPLY_SRGB || MODE == alpha_mode::PREMULTIPLY {
        r *= a;
        g *= a;
        b *= a;
    }
    if MODE == alpha_mode::PREMULTIPLY_SRGB {
        r = srgb_linear_to_nonlinear(r);
        g = srgb_linear_to_nonlinear(g);
        b = srgb_linear_to_nonlinear(b);
    }
    [
        float_to_u8(r),
        float_to_u8(g),
        float_to_u8(b),
        float_to_u8(a),
    ]
}

/// One texel, as the rows convert it: the four big-endian 16-bit channels
/// of `be` (R, G, B, A) to 8-bit RGBA for `mode`, or `None` for an alpha
/// mode outside the three, which converts nothing.
#[must_use]
pub fn texel(be: [u8; SRC_TEXEL_BYTES], mode: i32) -> Option<[u8; DST_TEXEL_BYTES]> {
    match mode {
        alpha_mode::STRAIGHT => Some(convert::<{ alpha_mode::STRAIGHT }>(be)),
        alpha_mode::PREMULTIPLY_SRGB => Some(convert::<{ alpha_mode::PREMULTIPLY_SRGB }>(be)),
        alpha_mode::PREMULTIPLY => Some(convert::<{ alpha_mode::PREMULTIPLY }>(be)),
        _ => None,
    }
}

/// `gs_ffmpeg_image_rgba64be_rows()`: converts the `cy` rows of the
/// big-endian 16-bit RGBA frame `src`, `src_linesize` bytes apart, into
/// 8-bit RGBA in `dst`, for the alpha `mode`. Returns the bytes written,
/// always from the start of `dst`.
///
/// Each row converts [`row_texels`] texels. The output advances 4 bytes
/// per texel; the source skips the rest of its line after each row.
/// Nothing is read or written for an alpha mode outside the three. The rows
/// stop at the first texel that does not fit in `src` or `dst`.
pub fn rgba64be_rows<D: DstByte>(
    dst: &mut [D],
    src: &[u8],
    cx: i32,
    cy: i32,
    src_linesize: usize,
    mode: i32,
) -> usize {
    match mode {
        alpha_mode::STRAIGHT => rows::<D, { alpha_mode::STRAIGHT }>(dst, src, cx, cy, src_linesize),
        alpha_mode::PREMULTIPLY_SRGB => {
            rows::<D, { alpha_mode::PREMULTIPLY_SRGB }>(dst, src, cx, cy, src_linesize)
        }
        alpha_mode::PREMULTIPLY => {
            rows::<D, { alpha_mode::PREMULTIPLY }>(dst, src, cx, cy, src_linesize)
        }
        // Nothing read or written.
        _ => 0,
    }
}

/// [`rgba64be_rows`] in the alpha mode `MODE`.
fn rows<D: DstByte, const MODE: i32>(
    dst: &mut [D],
    src: &[u8],
    cx: i32,
    cy: i32,
    src_linesize: usize,
) -> usize {
    let row_bytes = src_row_bytes(cx, src_linesize);
    let texels = row_bytes >> 3;
    if texels == 0 {
        // Every row reads and writes nothing (and skips its whole line).
        return 0;
    }
    // Never negative: `row_bytes` is at most the line size.
    let skip = src_linesize.wrapping_sub(row_bytes);

    let mut s = 0usize;
    let mut d = 0usize;
    for _ in 0..cy.max(0) {
        // The texels of this row that fit in both buffers: all of them but
        // at the end of a buffer that is too short.
        let fit = texels
            .min(src.len().saturating_sub(s) / SRC_TEXEL_BYTES)
            .min(dst.len().saturating_sub(d) / DST_TEXEL_BYTES);
        let (Some(row_src), Some(row_dst)) = (
            src.get(s..)
                .and_then(|r| r.get(..fit.wrapping_mul(SRC_TEXEL_BYTES))),
            dst.get_mut(d..)
                .and_then(|r| r.get_mut(..fit.wrapping_mul(DST_TEXEL_BYTES))),
        ) else {
            return d;
        };
        // `chunks_exact` rather than `as_chunks`: the same texels, and about
        // 1.4 times faster here (port_comparison, image_rgba64/straight).
        #[expect(
            clippy::chunks_exact_to_as_chunks,
            reason = "as_chunks is slower in this loop"
        )]
        let texels_in_row = row_src
            .chunks_exact(SRC_TEXEL_BYTES)
            .zip(row_dst.chunks_exact_mut(DST_TEXEL_BYTES));
        for (texel, out) in texels_in_row {
            let Ok(texel) = <[u8; SRC_TEXEL_BYTES]>::try_from(texel) else {
                return d;
            };
            for (byte, value) in out.iter_mut().zip(convert::<MODE>(texel)) {
                byte.put(value);
            }
        }
        // In bounds: `fit` texels were just taken from both buffers.
        s = s.wrapping_add(fit.wrapping_mul(SRC_TEXEL_BYTES));
        d = d.wrapping_add(fit.wrapping_mul(DST_TEXEL_BYTES));
        if fit < texels {
            return d;
        }
        let Some(next) = s.checked_add(skip) else {
            return d;
        };
        s = next;
    }
    d
}
