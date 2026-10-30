//! C ABI for `libobs/obs-output-caption.c.h`, the caption plan of
//! `add_caption()` in `obs-output.c`, declared in
//! `libobs/obs-output-caption.h` without `EXPORT`.
//!
//! These only decide. `obs-output.c.h` (still C) keeps the caption track,
//! the queue, every libcaption call, the new packet and the logs, and passes
//! plain values here: the codec name, the priority, the packet's first
//! bytes and one triplet at a time. So `struct obs_output` and
//! `struct encoder_packet` are never mirrored. These shims allocate nothing
//! and call nothing in libobs. All three stay hidden (`rust-hidden.txt`).
//!
//! No intentional differences: [`obs_output_caption_hevc_sei_header`] reads
//! the bytes the C reads, also past the end of a packet that is too short
//! for them (#3074).

use core::ffi::{CStr, c_char, c_int};

use crate::output_caption as caption;

/// `bool obs_output_caption_begin(const char *codec, bool hevc, int
/// priority, uint32_t *carrier)`.
///
/// # Safety
///
/// As the C: `codec` points to a NUL-terminated string (the encoder's
/// `info.codec`) and `carrier` to a writable `uint32_t`. `add_caption()`
/// passes the encoder's codec and a local.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_caption_begin(
    codec: *const c_char,
    hevc: bool,
    priority: c_int,
    carrier: *mut u32,
) -> bool {
    // SAFETY: the caller's contract.
    let codec = unsafe { CStr::from_ptr(codec) };
    let begin = caption::begin(codec.to_bytes(), hevc, priority);
    // SAFETY: the caller's contract.
    unsafe { carrier.write(begin.carrier.to_c()) };
    begin.takes
}

/// `bool obs_output_caption_hevc_sei_header(const uint8_t *data, uint8_t
/// header[2])`.
///
/// # Safety
///
/// As the C: `data` is readable for its first 3 bytes, the 4th when they
/// are not `00 00 01`, and the 2 bytes after the start code when there is
/// one; `header` points to 2 writable bytes. `add_caption()` passes an HEVC
/// packet's data and a local.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_caption_hevc_sei_header(
    data: *const u8,
    header: *mut [u8; 2],
) -> bool {
    // The C's two memcmp(): `00 00 01` over 3 bytes, then `00 00 00 01`
    // over 4. Only a packet that starts `00 00 00` can match the second, so
    // the 4th byte is read only then.
    // SAFETY: the caller's contract: the first 3 bytes are readable.
    let head = unsafe { data.cast::<[u8; 3]>().read() };
    let start = match head {
        [0, 0, 1] => 3,
        // SAFETY: the caller's contract: the 4th byte is readable when the
        // first 3 are not `00 00 01`.
        [0, 0, 0] if unsafe { data.add(3).read() } == 1 => 4,
        _ => return false,
    };
    // SAFETY: the caller's contract: the start code and the 2 header bytes
    // after it are readable.
    let packet = unsafe { core::slice::from_raw_parts(data, start + 2) };
    match caption::hevc_sei_header(packet) {
        Some(h) => {
            // SAFETY: the caller's contract.
            unsafe { header.write(h) };
            true
        }
        None => false,
    }
}

/// `bool obs_output_caption_cc_keep(const uint8_t triplet[3], uint16_t
/// *cc_data)`.
///
/// # Safety
///
/// As the C: `triplet` points to 3 readable bytes and `cc_data` to a
/// writable `uint16_t`. `add_caption()` passes the buffer it pops each
/// triplet into and a local.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_caption_cc_keep(
    triplet: *const [u8; 3],
    cc_data: *mut u16,
) -> bool {
    // SAFETY: the caller's contract.
    let triplet = unsafe { triplet.read() };
    match caption::cc_keep(triplet) {
        Some(word) => {
            // SAFETY: the caller's contract.
            unsafe { cc_data.write(word) };
            true
        }
        None => false,
    }
}
