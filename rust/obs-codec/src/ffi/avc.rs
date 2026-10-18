//! C ABI shims for the exported functions in `libobs/obs-avc.h`.
//!
//! The shims convert to the safe core in [`crate::avc`]; the packet
//! plumbing is shared with the HEVC shims in [`super::packet`].

use core::ffi::c_int;

use super::nal::obs_nal_find_startcode;
use super::packet::{bmalloc_concat, bytes, encoder_packet, parse_packet, write_split_units};
use crate::avc::{self, AvcHeader};
use crate::nal::SplitUnits;

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

/// Copies `*src` to `*avc_packet` with its data converted to AVCC, behind
/// a `long` reference count.
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
    unsafe {
        parse_packet(avc_packet, src, |data, keyframe, priority, write| {
            avc::write_avcc(data, keyframe, priority, write)
        })
    };
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
    let parsed = avc::parse_header(data);
    let out: &[u8] = match parsed {
        AvcHeader::None => return 0,
        // C: *header = bmemdup(data, size);
        AvcHeader::Copy => data,
        AvcHeader::Record(ref record) => record,
    };
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
    let units = SplitUnits::new(unsafe { bytes(packet, size) }, avc::bucket);
    // SAFETY: the caller guarantees every output pointer is writable.
    unsafe {
        write_split_units(
            &units,
            new_packet_data,
            new_packet_size,
            header_data,
            header_size,
            sei_data,
            sei_size,
        );
    }
}
