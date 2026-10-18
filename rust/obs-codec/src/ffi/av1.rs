//! C ABI shims for the exported functions in `libobs/obs-av1.h`.
//!
//! The shims convert to the safe core in [`crate::av1`] and copy its results
//! into `bmalloc` memory, which the callers `bfree`. As in `obs-av1.c`, the
//! outputs are the only allocations: the core gives slices of the input and
//! small stack buffers, which go straight into the `bmalloc` buffers (#1304).

use super::packet::{bmalloc_concat, bmalloc_concat_iter, bytes};
use crate::av1::{self, METADATA_TYPE_ITUT_T35};

/// Returns true if the first frame (header) OBU in the packet is a shown
/// key frame.
///
/// # Safety
///
/// A non-NULL `data` must be readable for `size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_av1_keyframe(data: *const u8, size: usize) -> bool {
    // SAFETY: forwarded from the caller.
    av1::keyframe(unsafe { bytes(data, size) })
}

/// Copies the packet, and its sequence header and metadata OBUs, to new
/// `bmalloc` buffers (NULL with size 0 when empty).
///
/// # Safety
///
/// `packet` is readable for `size` bytes; every output pointer is
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_extract_av1_headers(
    packet: *const u8,
    size: usize,
    new_packet_data: *mut *mut u8,
    new_packet_size: *mut usize,
    header_data: *mut *mut u8,
    header_size: *mut usize,
) {
    // SAFETY: the caller guarantees `size` readable bytes.
    let data = unsafe { bytes(packet, size) };
    let header_len = av1::header_obus(data).map(<[u8]>::len).sum();
    // SAFETY: the caller guarantees every output pointer is writable.
    unsafe {
        // The OBUs cover every byte, so the packet is a copy of `data`.
        *new_packet_data = bmalloc_concat(&[data]);
        *new_packet_size = data.len();
        *header_data = bmalloc_concat_iter(header_len, av1::header_obus(data));
        *header_size = header_len;
    }
}

/// Builds a metadata OBU of `metadata_type` around `source_buffer`.
///
/// # Safety
///
/// `source_buffer` is readable for `source_bufsize` bytes; `out_buffer`
/// and `outbuf_size` are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn metadata_obu(
    source_buffer: *const u8,
    source_bufsize: usize,
    out_buffer: *mut *mut u8,
    outbuf_size: *mut usize,
    metadata_type: u8,
) {
    // SAFETY: the caller guarantees `source_bufsize` readable bytes.
    let payload = unsafe { bytes(source_buffer, source_bufsize) };
    let (prefix, n) = av1::metadata_obu_prefix(payload.len(), metadata_type);
    let parts: [&[u8]; 3] = [&prefix[..n], payload, &[av1::METADATA_TRAILING_BITS]];
    // SAFETY: the caller guarantees both outputs are writable.
    unsafe {
        *outbuf_size = n + payload.len() + 1;
        *out_buffer = bmalloc_concat(&parts);
    }
}

/// Builds an ITU-T T.35 metadata OBU around `itut_t35_buffer`.
///
/// # Safety
///
/// As [`metadata_obu`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn metadata_obu_itu_t35(
    itut_t35_buffer: *const u8,
    itut_bufsize: usize,
    out_buffer: *mut *mut u8,
    outbuf_size: *mut usize,
) {
    // SAFETY: forwarded from the caller.
    unsafe {
        metadata_obu(
            itut_t35_buffer,
            itut_bufsize,
            out_buffer,
            outbuf_size,
            METADATA_TYPE_ITUT_T35,
        );
    }
}
