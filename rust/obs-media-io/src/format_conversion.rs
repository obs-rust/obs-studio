//! Port of `libobs/media-io/format-conversion.c`: packing conversions
//! between UYVX, planar and semi-planar YUV, and packed 32-bit pixels.
//!
//! The C does the UYVX functions with SSE2 integer operations. Every lane
//! works out to plain integer math (picking bytes out of each pixel, and
//! `(a + b + c + d) >> 2` for the chroma of a 2x2 block), so this is a
//! scalar port, exact byte for byte. Offsets use the C's `u32` arithmetic,
//! wrapping included, and each loop keeps the C's order of reads and writes,
//! which decides the result where output rows overlap.
//!
//! The planes are [`Source`]s and [`Sink`]s addressed by byte offset. Byte
//! slices implement both and panic on an out-of-bounds access, where the C
//! reads or writes out of bounds; the C ABI shims implement them over raw
//! pointers. As in the C, a `uint16_t` or `uint32_t` value is stored in
//! native byte order.

use core::ops::Range;

/// An input plane, read by byte offset.
pub trait Source {
    /// The `N` bytes at `offset`.
    fn read<const N: usize>(&self, offset: usize) -> [u8; N];

    /// The `len` bytes at `offset`, as a slice.
    ///
    /// The decompress conversions read a row at a time through this and then
    /// index it, instead of calling [`read`](Source::read) once per pixel
    /// (three calls per output pixel, each one copying and bounds-checking a
    /// fixed-size array, was 1.5-3.2x the C: #3111).
    fn bytes(&self, offset: usize, len: usize) -> &[u8];
}

/// An output plane, written by byte offset.
pub trait Sink {
    /// Writes `bytes` at `offset`.
    fn write<const N: usize>(&mut self, offset: usize, bytes: [u8; N]);

    /// The `len` bytes at `offset`, as a mutable slice.
    ///
    /// The compress conversions borrow each output row through this and then
    /// index it, instead of calling [`write`](Sink::write) once per access:
    /// the per-access cost, not the lane arithmetic, was what kept them 2-4x
    /// over the C (#3004).
    fn bytes_mut(&mut self, offset: usize, len: usize) -> &mut [u8];
}

impl Source for [u8] {
    fn read<const N: usize>(&self, offset: usize) -> [u8; N] {
        let mut bytes = [0; N];
        bytes.copy_from_slice(&self[offset..offset + N]);
        bytes
    }

    fn bytes(&self, offset: usize, len: usize) -> &[u8] {
        &self[offset..offset + len]
    }
}

impl Sink for [u8] {
    fn write<const N: usize>(&mut self, offset: usize, bytes: [u8; N]) {
        self[offset..offset + N].copy_from_slice(&bytes);
    }

    fn bytes_mut(&mut self, offset: usize, len: usize) -> &mut [u8] {
        &mut self[offset..offset + len]
    }
}

/// The offset of each channel in a UYVX pixel.
const UYVX_U: usize = 0;
const UYVX_Y: usize = 1;
const UYVX_V: usize = 2;

/// The two 32-bit pixels `luma[at]` and `luma[at + 1]`, each with `pixel`'s
/// value, as the two adjacent `uint32_t` stores the C makes.
fn pair(luma: &[u8], at: usize, pixel: impl Fn(u8) -> u32) -> [u8; 8] {
    let (first, second) = (
        pixel(luma[at]).to_ne_bytes(),
        pixel(luma[at + 1]).to_ne_bytes(),
    );
    let mut bytes = [0u8; 8];
    bytes[..4].copy_from_slice(&first);
    bytes[4..].copy_from_slice(&second);
    bytes
}

/// `base + add` for a pointer advanced by a `u32` offset.
fn at(base: usize, add: u32) -> usize {
    base.wrapping_add(add as usize)
}

/// The bytes one block of four UYVX pixels occupies, and reads, in a row.
const BLOCK_BYTES: usize = 16;

/// The four bytes of channel `c` in the four UYVX pixels at `at` in `row`.
fn channel(row: &[u8], at: usize, c: usize) -> [u8; 4] {
    [row[at + c], row[at + 4 + c], row[at + 8 + c], row[at + 12 + c]]
}

/// Channel `c` averaged over pixels 0-1 and 2-3 of the two rows at `at`,
/// `(a + b + c + d) >> 2`.
fn average(line1: &[u8], line2: &[u8], at: usize, c: usize) -> [u8; 2] {
    let avg = |p: usize| {
        let sum = u16::from(line1[at + p + c])
            + u16::from(line1[at + p + 4 + c])
            + u16::from(line2[at + p + c])
            + u16::from(line2[at + p + 4 + c]);
        (sum >> 2) as u8
    };
    [avg(0), avg(8)]
}

/// Packs UYVX rows `start_y..end_y`, two at a time, into planar I420:
/// full-size luma in `output[0]`, and U and V averaged over each 2x2 block
/// in `output[1]` and `output[2]`.
///
/// Each UYVX pixel is 4 bytes, U Y V X. The width is
/// `min(in_linesize, out_linesize[0])` pixels, converted 4 at a time. Luma
/// rows are `out_linesize[0]` apart and both chroma planes'
/// `out_linesize[1]` apart: `out_linesize[2]` is not used.
///
/// The row pair and the chroma rows are borrowed once per `y` and written by
/// index, instead of four [`Sink::write`] calls per block: that per-access
/// cost, not the lane arithmetic, is what kept this 2-4x over the C (#3004).
pub fn compress_uyvx_to_i420<S, D>(
    input: &S,
    in_linesize: u32,
    start_y: u32,
    end_y: u32,
    output: [&mut D; 3],
    out_linesize: &[u32],
) where
    S: Source + ?Sized,
    D: Sink + ?Sized,
{
    let [lum, u, v] = output;
    let (lum_linesize, chroma_linesize) = (out_linesize[0], out_linesize[1]);
    let width = in_linesize.min(lum_linesize);
    let blocks = width.div_ceil(4) as usize;
    if blocks == 0 {
        return;
    }
    let bytes = blocks * 4;
    let lum_stride = lum_linesize as usize;
    let mut y = start_y;
    while y < end_y {
        let in_pos = y.wrapping_mul(in_linesize) as usize;
        let read = blocks * BLOCK_BYTES;
        let line1 = input.bytes(in_pos, read);
        let line2 = input.bytes(at(in_pos, in_linesize), read);
        let lum_pos = y.wrapping_mul(lum_linesize) as usize;
        // One slice covers the row pair, so a write past `lum_linesize` (a
        // width that is not a multiple of 4) still lands where the C's does.
        let luma = lum.bytes_mut(lum_pos, lum_stride + bytes);
        let chroma_pos = (y >> 1).wrapping_mul(chroma_linesize) as usize;
        let u_row = u.bytes_mut(chroma_pos, blocks * 2);
        let v_row = v.bytes_mut(chroma_pos, blocks * 2);
        for i in 0..blocks {
            let (pixel, block) = (i * 4, i * BLOCK_BYTES);
            let luma_at = pixel;
            luma[luma_at..luma_at + 4].copy_from_slice(&channel(line1, block, UYVX_Y));
            luma[lum_stride + luma_at..lum_stride + luma_at + 4]
                .copy_from_slice(&channel(line2, block, UYVX_Y));
            let [u01, u23] = average(line1, line2, block, UYVX_U);
            let [v01, v23] = average(line1, line2, block, UYVX_V);
            u_row[i * 2..i * 2 + 2].copy_from_slice(&[u01, u23]);
            v_row[i * 2..i * 2 + 2].copy_from_slice(&[v01, v23]);
        }
        y = y.wrapping_add(2);
    }
}

/// Packs UYVX rows `start_y..end_y`, two at a time, into NV12: full-size
/// luma in `output[0]`, and U and V averaged over each 2x2 block and
/// interleaved in `output[1]`.
///
/// The width is `min(in_linesize, out_linesize[0])` pixels, converted 4 at
/// a time. Luma rows are `out_linesize[0]` apart, chroma rows
/// `out_linesize[1]`.
pub fn compress_uyvx_to_nv12<S, D>(
    input: &S,
    in_linesize: u32,
    start_y: u32,
    end_y: u32,
    output: [&mut D; 2],
    out_linesize: &[u32],
) where
    S: Source + ?Sized,
    D: Sink + ?Sized,
{
    let [lum, chroma] = output;
    let (lum_linesize, chroma_linesize) = (out_linesize[0], out_linesize[1]);
    let width = in_linesize.min(lum_linesize);
    let blocks = width.div_ceil(4) as usize;
    if blocks == 0 {
        return;
    }
    let bytes = blocks * 4;
    let lum_stride = lum_linesize as usize;
    let mut y = start_y;
    while y < end_y {
        let in_pos = y.wrapping_mul(in_linesize) as usize;
        let read = blocks * BLOCK_BYTES;
        let line1 = input.bytes(in_pos, read);
        let line2 = input.bytes(at(in_pos, in_linesize), read);
        let lum_pos = y.wrapping_mul(lum_linesize) as usize;
        let luma = lum.bytes_mut(lum_pos, lum_stride + bytes);
        let chroma_pos = (y >> 1).wrapping_mul(chroma_linesize) as usize;
        let chroma_row = chroma.bytes_mut(chroma_pos, blocks * 4);
        for i in 0..blocks {
            let (pixel, block) = (i * 4, i * BLOCK_BYTES);
            luma[pixel..pixel + 4].copy_from_slice(&channel(line1, block, UYVX_Y));
            luma[lum_stride + pixel..lum_stride + pixel + 4]
                .copy_from_slice(&channel(line2, block, UYVX_Y));
            let [u01, u23] = average(line1, line2, block, UYVX_U);
            let [v01, v23] = average(line1, line2, block, UYVX_V);
            chroma_row[pixel..pixel + 4].copy_from_slice(&[u01, v01, u23, v23]);
        }
        y = y.wrapping_add(2);
    }
}

/// Splits UYVX rows `start_y..end_y`, two at a time, into full-size Y, U
/// and V planes in `output[0..3]`.
///
/// The width is `min(in_linesize, out_linesize)` pixels, converted 4 at a
/// time. All three planes' rows are `out_linesize` apart: the C reads only
/// `out_linesize[0]`.
pub fn convert_uyvx_to_i444<S, D>(
    input: &S,
    in_linesize: u32,
    start_y: u32,
    end_y: u32,
    output: [&mut D; 3],
    out_linesize: u32,
) where
    S: Source + ?Sized,
    D: Sink + ?Sized,
{
    let [lum, u, v] = output;
    let width = in_linesize.min(out_linesize);
    let blocks = width.div_ceil(4) as usize;
    if blocks == 0 {
        return;
    }
    let bytes = blocks * 4;
    let stride = out_linesize as usize;
    let mut y = start_y;
    while y < end_y {
        let in_pos = y.wrapping_mul(in_linesize) as usize;
        let read = blocks * BLOCK_BYTES;
        let line1 = input.bytes(in_pos, read);
        let line2 = input.bytes(at(in_pos, in_linesize), read);
        let pos = y.wrapping_mul(out_linesize) as usize;
        for (plane, c) in [(&mut *lum, UYVX_Y), (&mut *u, UYVX_U), (&mut *v, UYVX_V)] {
            let rows = plane.bytes_mut(pos, stride + bytes);
            for i in 0..blocks {
                let (pixel, block) = (i * 4, i * BLOCK_BYTES);
                rows[pixel..pixel + 4].copy_from_slice(&channel(line1, block, c));
                rows[stride + pixel..stride + pixel + 4].copy_from_slice(&channel(line2, block, c));
            }
        }
        y = y.wrapping_add(2);
    }
}

/// Expands planar I420 into one `Y << 16 | U << 8 | V` value per pixel.
///
/// Rows go in pairs from `start_y / 2` to `end_y / 2`, both rounded down.
/// The width is `in_linesize[0]` rounded down to even, whatever
/// `out_linesize` is.
pub fn decompress_420<S, D>(
    input: [&S; 3],
    in_linesize: &[u32],
    start_y: u32,
    end_y: u32,
    output: &mut D,
    out_linesize: u32,
) where
    S: Source + ?Sized,
    D: Sink + ?Sized,
{
    let [lum, u, v] = input;
    let width_d2 = in_linesize[0] / 2;
    for y in start_y / 2..end_y / 2 {
        let chroma0 = y.wrapping_mul(in_linesize[1]) as usize;
        let chroma1 = y.wrapping_mul(in_linesize[2]) as usize;
        let lum0 = y.wrapping_mul(2).wrapping_mul(in_linesize[0]) as usize;
        let lum1 = at(lum0, in_linesize[0]);
        let output0 = y.wrapping_mul(2).wrapping_mul(out_linesize) as usize;
        let output1 = at(output0, out_linesize);
        if width_d2 == 0 {
            continue;
        }

        // One slice per row, then a per-two-pixel write of the C's two
        // adjacent `uint32_t` stores, in the C's order (#3111).
        let width_d2 = width_d2 as usize;
        let luma0 = lum.bytes(lum0, width_d2 * 2);
        let luma1 = lum.bytes(lum1, width_d2 * 2);
        let u_row = u.bytes(chroma0, width_d2);
        let v_row = v.bytes(chroma1, width_d2);

        for x in 0..width_d2 {
            let out = u32::from(u_row[x]) << 8 | u32::from(v_row[x]);
            let pixel = |l| u32::from(l) << 16 | out;
            output.write(output0 + 8 * x, pair(luma0, 2 * x, pixel));
            output.write(output1 + 8 * x, pair(luma1, 2 * x, pixel));
        }
    }
}

/// Expands NV12 into one `Y | uv << 8` value per pixel, where `uv` is the
/// interleaved U V pair read as a native-endian `u16`.
///
/// Rows go in pairs from `start_y / 2` to `end_y / 2`, both rounded down.
/// The width is `min(in_linesize[0], out_linesize)` rounded down to even.
pub fn decompress_nv12<S, D>(
    input: [&S; 2],
    in_linesize: &[u32],
    start_y: u32,
    end_y: u32,
    output: &mut D,
    out_linesize: u32,
) where
    S: Source + ?Sized,
    D: Sink + ?Sized,
{
    let [lum, uv] = input;
    let width_d2 = in_linesize[0].min(out_linesize) / 2;
    for y in start_y / 2..end_y / 2 {
        let chroma = y.wrapping_mul(in_linesize[1]) as usize;
        let lum0 = y.wrapping_mul(2).wrapping_mul(in_linesize[0]) as usize;
        let lum1 = at(lum0, in_linesize[0]);
        let output0 = y.wrapping_mul(2).wrapping_mul(out_linesize) as usize;
        let output1 = at(output0, out_linesize);
        if width_d2 == 0 {
            continue;
        }

        let width_d2 = width_d2 as usize;
        let luma0 = lum.bytes(lum0, width_d2 * 2);
        let luma1 = lum.bytes(lum1, width_d2 * 2);
        let chroma_row = uv.bytes(chroma, width_d2 * 2);

        for x in 0..width_d2 {
            let uv_pair = u16::from_ne_bytes([chroma_row[2 * x], chroma_row[2 * x + 1]]);
            let out = u32::from(uv_pair) << 8;
            output.write(output0 + 8 * x, pair(luma0, 2 * x, |l| u32::from(l) | out));
            output.write(output1 + 8 * x, pair(luma1, 2 * x, |l| u32::from(l) | out));
        }
    }
}

/// Expands packed 4:2:2 rows `start_y..end_y` into one 32-bit value per
/// pixel: each input `u32` is copied, then repeated with its other luma byte
/// in place of the first (`leading_lum`, YUYV) or second (UYVY) one.
///
/// Each row reads `min(in_linesize, out_linesize) / 4` values, the row's
/// bytes (2 per pixel), and writes twice as many, 4 bytes per pixel. Until
/// #3031 the C and this port read and wrote twice that, so each row spilled
/// into the next one.
pub fn decompress_422<S, D>(
    input: &S,
    in_linesize: u32,
    start_y: u32,
    end_y: u32,
    output: &mut D,
    out_linesize: u32,
    leading_lum: bool,
) where
    S: Source + ?Sized,
    D: Sink + ?Sized,
{
    let width_d2 = in_linesize.min(out_linesize) / 4;
    for y in start_y..end_y {
        let input32 = y.wrapping_mul(in_linesize) as usize;
        let output32 = y.wrapping_mul(out_linesize) as usize;
        if width_d2 == 0 {
            continue;
        }

        let width_d2 = width_d2 as usize;
        let row = input.bytes(input32, width_d2 * 4);
        for i in 0..width_d2 {
            let at = 4 * i;
            let dw = u32::from_ne_bytes([row[at], row[at + 1], row[at + 2], row[at + 3]]);
            let dup = if leading_lum {
                (dw & 0xFFFF_FF00) | ((dw >> 16) & 0xFF)
            } else {
                (dw & 0xFFFF_00FF) | ((dw >> 16) & 0xFF00)
            };
            let (low, high) = (dw.to_ne_bytes(), dup.to_ne_bytes());
            let mut bytes = [0u8; 8];
            bytes[..4].copy_from_slice(&low);
            bytes[4..].copy_from_slice(&high);
            output.write(output32 + 8 * i, bytes);
        }
    }
}
