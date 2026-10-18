//! C ABI shims for the exported functions in `libobs/obs-hevc.h`.
//!
//! The shims convert to the safe core in [`crate::hevc`]; the packet
//! plumbing is shared with the AVC shims in [`super::packet`].

use core::ffi::c_int;

use super::packet::{bytes, encoder_packet, parse_packet, write_split_units};
use crate::hevc;
use crate::nal::SplitUnits;

/// Returns true if the first picture unit in the packet is an IRAP picture.
///
/// # Safety
///
/// A non-NULL `data` must be readable for `size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hevc_keyframe(data: *const u8, size: usize) -> bool {
    // SAFETY: forwarded from the caller.
    hevc::keyframe(unsafe { bytes(data, size) })
}

/// Copies `*src` to `*hevc_packet` with its data converted to
/// length-prefixed units, behind a `long` reference count.
///
/// # Safety
///
/// `src` is a readable packet whose `data` is readable for `size` bytes;
/// `hevc_packet` is writable. They may be the same packet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_parse_hevc_packet(
    hevc_packet: *mut encoder_packet,
    src: *const encoder_packet,
) {
    // SAFETY: forwarded from the caller.
    unsafe {
        parse_packet(hevc_packet, src, |data, keyframe, priority, write| {
            hevc::write_hvcc(data, keyframe, priority, write)
        })
    };
}

/// Returns the packet's priority raised to the highest NAL priority in it.
///
/// # Safety
///
/// `packet` is readable and its `data` readable for `size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_parse_hevc_packet_priority(packet: *const encoder_packet) -> c_int {
    // SAFETY: the caller guarantees `packet` and its data are readable.
    let (data, priority) = unsafe { (bytes((*packet).data, (*packet).size), (*packet).priority) };
    hevc::packet_priority(data, priority)
}

/// Splits an Annex B packet into its VPS/SPS/PPS units, its SEI units, and
/// the rest, each output `bmalloc`ed (or NULL with size 0 when empty).
///
/// # Safety
///
/// `packet` is readable for `size` bytes; every output pointer is
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_extract_hevc_headers(
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
    let units = SplitUnits::new(unsafe { bytes(packet, size) }, hevc::bucket);
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
