//! C ABI shims for the exported functions in `libobs/obs-avc.h`.
//!
//! The shims convert to the safe core in [`crate::avc`] and copy its results
//! into `bmalloc` memory, which the callers `bfree` as they did with the C
//! `DARRAY` and serializer buffers.

use core::ffi::c_int;

use super::nal::obs_nal_find_startcode;
pub use super::packet::encoder_packet;
use super::packet::{bmalloc_concat, bytes, parse_packet, write_split};
use crate::avc::{self, AvcHeader};

/// Returns true if the first slice in the packet is an IDR slice.
///
/// # Safety
///
/// A non-NULL `data` must be readable for `size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_avc_keyframe(data: *const u8, size: usize) -> bool {
    // SAFETY: forwarded from the caller.
    avc::keyframe(unsafe { bytes(data, size) })
}

/// Same as [`obs_nal_find_startcode`].
///
/// # Safety
///
/// As [`obs_nal_find_startcode`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_avc_find_startcode(p: *const u8, end: *const u8) -> *const u8 {
    // SAFETY: forwarded from the caller.
    unsafe { obs_nal_find_startcode(p, end) }
}

/// Copies `*src` to `*avc_packet` with its data converted to AVCC.
///
/// The new data is `bmalloc`ed with a `long` reference count of 1 in front,
/// as the C serializer wrote it; `data` points just past it.
///
/// # Safety
///
/// `src` is a readable packet whose `data` is readable for `size` bytes;
/// `avc_packet` is writable. They may be the same packet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_parse_avc_packet(
    avc_packet: *mut encoder_packet,
    src: *const encoder_packet,
) {
    // SAFETY: forwarded from the caller.
    unsafe { parse_packet(avc_packet, src, avc::to_avcc) };
}

/// Returns the packet's priority raised to the highest NAL priority in it.
///
/// # Safety
///
/// `packet` is readable and its `data` readable for `size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_parse_avc_packet_priority(packet: *const encoder_packet) -> c_int {
    // SAFETY: the caller guarantees `packet` and its data are readable.
    let (data, priority) = unsafe { (bytes((*packet).data, (*packet).size), (*packet).priority) };
    avc::packet_priority(data, priority)
}

/// Builds an `AVCDecoderConfigurationRecord` from an Annex B header, or
/// copies a header that is not Annex B. Returns the size written to
/// `*header`, or 0 (leaving `*header` untouched).
///
/// # Safety
///
/// `data` is readable for `size` bytes; `header` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_parse_avc_header(
    header: *mut *mut u8,
    data: *const u8,
    size: usize,
) -> usize {
    // SAFETY: the caller guarantees `size` readable bytes.
    let data = unsafe { bytes(data, size) };
    let out: &[u8] = match avc::parse_header(data) {
        AvcHeader::None => return 0,
        AvcHeader::Copy => data,
        AvcHeader::Record(ref record) => {
            let p = bmalloc_concat(&[record]);
            // SAFETY: the caller guarantees `header` is writable.
            unsafe { *header = p };
            return record.len();
        }
    };
    // C: *header = bmemdup(data, size);
    // SAFETY: the caller guarantees `header` is writable.
    unsafe { *header = bmalloc_concat(&[out]) };
    out.len()
}

/// Splits an Annex B packet into its SPS/PPS units, its SEI units, and the
/// rest, each output `bmalloc`ed (or NULL with size 0 when empty).
///
/// # Safety
///
/// `packet` is readable for `size` bytes; every output pointer is
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_extract_avc_headers(
    packet: *const u8,
    size: usize,
    new_packet_data: *mut *mut u8,
    new_packet_size: *mut usize,
    header_data: *mut *mut u8,
    header_size: *mut usize,
    sei_data: *mut *mut u8,
    sei_size: *mut usize,
) {
    // SAFETY: the caller guarantees `size` readable bytes.
    let split = avc::extract_headers(unsafe { bytes(packet, size) });
    // SAFETY: the caller guarantees every output pointer is writable.
    unsafe {
        write_split(
            &split,
            new_packet_data,
            new_packet_size,
            header_data,
            header_size,
            sei_data,
            sei_size,
        );
    }
}
