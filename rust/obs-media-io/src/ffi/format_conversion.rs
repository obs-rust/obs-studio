//! C ABI shims for the exported functions in
//! `libobs/media-io/format-conversion.h`.
//!
//! The planes cross as raw pointers, which the safe core reads and writes
//! through [`RawSource`] and [`RawSink`] at the offsets the C would. Nothing
//! is checked, as in the C. Each shim reads only the `output`, `input` and
//! linesize entries the C reads, and reads them up front.

use core::ptr;

use crate::format_conversion::{self, Sink, Source};

/// A caller's input plane.
struct RawSource(*const u8);

impl Source for RawSource {
    fn read<const N: usize>(&self, offset: usize) -> [u8; N] {
        // SAFETY: only the shims below build a `RawSource`, from a plane
        // their caller guarantees is readable at every offset the
        // conversion reads.
        unsafe { ptr::read_unaligned(self.0.add(offset).cast::<[u8; N]>()) }
    }

    fn bytes(&self, offset: usize, len: usize) -> &[u8] {
        // SAFETY: as `read`: the caller guarantees the plane is readable at
        // every offset the conversion reads, so `len` bytes from `offset`
        // are in one allocation, and the slice lives only for the call.
        unsafe { core::slice::from_raw_parts(self.0.add(offset), len) }
    }
}

/// A caller's output plane.
struct RawSink(*mut u8);

impl Sink for RawSink {
    fn write<const N: usize>(&mut self, offset: usize, bytes: [u8; N]) {
        // SAFETY: only the shims below build a `RawSink`, from a plane their
        // caller guarantees is writable at every offset the conversion
        // writes.
        unsafe { ptr::write_unaligned(self.0.add(offset).cast::<[u8; N]>(), bytes) }
    }
}

/// Packs UYVX rows `start_y..end_y` into planar I420.
///
/// # Safety
///
/// `output` must point to 3 plane pointers and `out_linesize` to at least
/// 2 linesizes. `input` and the planes must be readable and writable at
/// every offset the conversion touches (see
/// [`format_conversion::compress_uyvx_to_i420`]).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compress_uyvx_to_i420(
    input: *const u8,
    in_linesize: u32,
    start_y: u32,
    end_y: u32,
    output: *const *mut u8,
    out_linesize: *const u32,
) {
    // SAFETY: the caller guarantees both arrays' lengths.
    let ([lum, u, v], linesizes) = unsafe {
        (
            ptr::read(output.cast::<[*mut u8; 3]>()),
            ptr::read(out_linesize.cast::<[u32; 2]>()),
        )
    };
    format_conversion::compress_uyvx_to_i420(
        &RawSource(input),
        in_linesize,
        start_y,
        end_y,
        [&mut RawSink(lum), &mut RawSink(u), &mut RawSink(v)],
        &linesizes,
    );
}

/// Packs UYVX rows `start_y..end_y` into NV12.
///
/// # Safety
///
/// `output` must point to 2 plane pointers and `out_linesize` to at least
/// 2 linesizes. `input` and the planes must be readable and writable at
/// every offset the conversion touches (see
/// [`format_conversion::compress_uyvx_to_nv12`]).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compress_uyvx_to_nv12(
    input: *const u8,
    in_linesize: u32,
    start_y: u32,
    end_y: u32,
    output: *const *mut u8,
    out_linesize: *const u32,
) {
    // SAFETY: the caller guarantees both arrays' lengths.
    let ([lum, chroma], linesizes) = unsafe {
        (
            ptr::read(output.cast::<[*mut u8; 2]>()),
            ptr::read(out_linesize.cast::<[u32; 2]>()),
        )
    };
    format_conversion::compress_uyvx_to_nv12(
        &RawSource(input),
        in_linesize,
        start_y,
        end_y,
        [&mut RawSink(lum), &mut RawSink(chroma)],
        &linesizes,
    );
}

/// Splits UYVX rows `start_y..end_y` into full-size Y, U and V planes.
///
/// # Safety
///
/// `output` must point to 3 plane pointers and `out_linesize` to at least
/// 1 linesize. `input` and the planes must be readable and writable at
/// every offset the conversion touches (see
/// [`format_conversion::convert_uyvx_to_i444`]).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn convert_uyvx_to_i444(
    input: *const u8,
    in_linesize: u32,
    start_y: u32,
    end_y: u32,
    output: *const *mut u8,
    out_linesize: *const u32,
) {
    // SAFETY: the caller guarantees both arrays' lengths.
    let ([lum, u, v], linesize) = unsafe {
        (
            ptr::read(output.cast::<[*mut u8; 3]>()),
            ptr::read(out_linesize),
        )
    };
    format_conversion::convert_uyvx_to_i444(
        &RawSource(input),
        in_linesize,
        start_y,
        end_y,
        [&mut RawSink(lum), &mut RawSink(u), &mut RawSink(v)],
        linesize,
    );
}

/// Expands planar I420 into packed 32-bit pixels.
///
/// # Safety
///
/// `input` and `in_linesize` must each point to 3 entries. The planes and
/// `output` must be readable and writable at every offset the conversion
/// touches (see [`format_conversion::decompress_420`]).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn decompress_420(
    input: *const *const u8,
    in_linesize: *const u32,
    start_y: u32,
    end_y: u32,
    output: *mut u8,
    out_linesize: u32,
) {
    // SAFETY: the caller guarantees both arrays' lengths.
    let ([y, u, v], linesizes) = unsafe {
        (
            ptr::read(input.cast::<[*const u8; 3]>()),
            ptr::read(in_linesize.cast::<[u32; 3]>()),
        )
    };
    format_conversion::decompress_420(
        [&RawSource(y), &RawSource(u), &RawSource(v)],
        &linesizes,
        start_y,
        end_y,
        &mut RawSink(output),
        out_linesize,
    );
}

/// Expands NV12 into packed 32-bit pixels.
///
/// # Safety
///
/// `input` and `in_linesize` must each point to 2 entries. The planes and
/// `output` must be readable and writable at every offset the conversion
/// touches (see [`format_conversion::decompress_nv12`]).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn decompress_nv12(
    input: *const *const u8,
    in_linesize: *const u32,
    start_y: u32,
    end_y: u32,
    output: *mut u8,
    out_linesize: u32,
) {
    // SAFETY: the caller guarantees both arrays' lengths.
    let ([y, uv], linesizes) = unsafe {
        (
            ptr::read(input.cast::<[*const u8; 2]>()),
            ptr::read(in_linesize.cast::<[u32; 2]>()),
        )
    };
    format_conversion::decompress_nv12(
        [&RawSource(y), &RawSource(uv)],
        &linesizes,
        start_y,
        end_y,
        &mut RawSink(output),
        out_linesize,
    );
}

/// Expands packed 4:2:2 rows `start_y..end_y` into packed 32-bit pixels.
///
/// # Safety
///
/// `input` and `output` must be readable and writable at every offset the
/// conversion touches (see [`format_conversion::decompress_422`]).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn decompress_422(
    input: *const u8,
    in_linesize: u32,
    start_y: u32,
    end_y: u32,
    output: *mut u8,
    out_linesize: u32,
    leading_lum: bool,
) {
    format_conversion::decompress_422(
        &RawSource(input),
        in_linesize,
        start_y,
        end_y,
        &mut RawSink(output),
        out_linesize,
        leading_lum,
    );
}
