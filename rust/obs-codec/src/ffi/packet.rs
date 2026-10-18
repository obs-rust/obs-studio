//! Shared C ABI plumbing for the AVC and HEVC shims: `struct
//! encoder_packet`, and the `bmalloc` outputs both C files produce the same
//! way.

use core::ffi::{c_int, c_long, c_void};
use core::mem::size_of;
use core::{ptr, slice};

use obs_util::ffi::darray::{bmalloc, darray, darray_push_back_array, darray_reserve};

use crate::nal::{LENGTH_PREFIX_SLACK, SplitHeaders, UnitRating};

/// `struct encoder_packet` from `obs-encoder.h`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub struct encoder_packet {
    pub data: *mut u8,
    pub size: usize,
    pub pts: i64,
    pub dts: i64,
    pub timebase_num: i32,
    pub timebase_den: i32,
    /// `enum obs_encoder_type`.
    pub type_: c_int,
    pub keyframe: bool,
    pub dts_usec: i64,
    pub sys_dts_usec: i64,
    pub priority: c_int,
    pub drop_priority: c_int,
    pub track_idx: usize,
    /// `obs_encoder_t *`.
    pub encoder: *mut c_void,
}

/// `[data, data + size)` as a slice; empty for a NULL `data`.
///
/// # Safety
///
/// A non-NULL `data` must be readable for `size` bytes.
pub(crate) unsafe fn bytes<'a>(data: *const u8, size: usize) -> &'a [u8] {
    if data.is_null() || size == 0 {
        &[]
    } else {
        // SAFETY: the caller guarantees `size` readable bytes.
        unsafe { slice::from_raw_parts(data, size) }
    }
}

/// A `bmalloc`ed copy of `parts`, concatenated; NULL when they are empty,
/// as an untouched `DARRAY` is.
pub(crate) fn bmalloc_concat(parts: &[&[u8]]) -> *mut u8 {
    let len: usize = parts.iter().map(|p| p.len()).sum();
    bmalloc_concat_iter(len, parts.iter().copied())
}

/// As [`bmalloc_concat`], for parts from an iterator that yields `len`
/// bytes in total: one `bmalloc`, no Rust heap allocation (#1304). Panics
/// instead of writing past the buffer when the parts do not add up to `len`.
pub(crate) fn bmalloc_concat_iter<'a>(
    len: usize,
    parts: impl IntoIterator<Item = &'a [u8]>,
) -> *mut u8 {
    if len == 0 {
        return ptr::null_mut();
    }
    // SAFETY: bmalloc returns `len` writable bytes; every copy below is
    // checked to stay inside them, and all `len` are written.
    unsafe {
        let out = bmalloc(len).cast::<u8>();
        let mut at = 0;
        for part in parts {
            assert!(
                part.len() <= len - at,
                "bmalloc_concat_iter: parts exceed {len} bytes"
            );
            ptr::copy_nonoverlapping(part.as_ptr(), out.add(at), part.len());
            at += part.len();
        }
        assert_eq!(
            at, len,
            "bmalloc_concat_iter: parts are shorter than {len} bytes"
        );
        out
    }
}

/// `obs_parse_avc_packet` / `obs_parse_hevc_packet`: copies `*src` to
/// `*out` with its data converted by `convert`.
///
/// The new data is `bmalloc`ed with a `long` reference count of 1 in front,
/// as the C serializer wrote it; `data` points just past it.
///
/// # Safety
///
/// `src` is a readable packet whose `data` is readable for `size` bytes;
/// `out` is writable. They may be the same packet.
pub(crate) unsafe fn parse_packet(
    out: *mut encoder_packet,
    src: *const encoder_packet,
    convert: impl FnOnce(&[u8], bool, i32, &mut dyn FnMut(&[u8])) -> UnitRating,
) {
    // SAFETY: the caller guarantees `src` is readable; read before writing.
    let mut packet = unsafe { ptr::read(src) };
    // SAFETY: the caller guarantees the packet data is readable.
    let data = unsafe { bytes(packet.data, packet.size) };

    // The C serializes into a `DARRAY(uint8_t)`. Reserving the usual size
    // up front keeps the appends from reallocating.
    let mut block = darray {
        array: ptr::null_mut(),
        num: 0,
        capacity: 0,
    };
    let reserve = size_of::<c_long>() + data.len() + LENGTH_PREFIX_SLACK;
    // SAFETY: `block` is a valid, empty byte darray.
    unsafe { darray_reserve(1, &raw mut block, reserve) };
    let mut write = |bytes: &[u8]| {
        // SAFETY: `block` is a byte darray; `bytes` is readable.
        unsafe { darray_push_back_array(1, &raw mut block, bytes.as_ptr().cast(), bytes.len()) };
    };

    // C: long ref = 1; serialize(&s, &ref, sizeof(ref));
    let reference: c_long = 1;
    write(&reference.to_ne_bytes());
    let parsed = convert(data, packet.keyframe, packet.priority, &mut write);

    // SAFETY: the block holds the reference count first.
    packet.data = unsafe { block.array.cast::<u8>().add(size_of::<c_long>()) };
    packet.size = block.num - size_of::<c_long>();
    packet.keyframe = parsed.keyframe;
    packet.priority = parsed.priority;
    packet.drop_priority = parsed.priority;
    // SAFETY: the caller guarantees `out` is writable.
    unsafe { ptr::write(out, packet) };
}

/// Writes the three `obs_extract_*_headers` outputs, each `bmalloc`ed (or
/// NULL with size 0 when empty).
///
/// # Safety
///
/// Every output pointer is writable.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn write_split(
    split: &SplitHeaders,
    new_packet_data: *mut *mut u8,
    new_packet_size: *mut usize,
    header_data: *mut *mut u8,
    header_size: *mut usize,
    sei_data: *mut *mut u8,
    sei_size: *mut usize,
) {
    // SAFETY: the caller guarantees every output pointer is writable.
    unsafe {
        *new_packet_data = bmalloc_concat(&[&split.packet]);
        *new_packet_size = split.packet.len();
        *header_data = bmalloc_concat(&[&split.header]);
        *header_size = split.header.len();
        *sei_data = bmalloc_concat(&[&split.sei]);
        *sei_size = split.sei.len();
    }
}
