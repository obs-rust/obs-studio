//! C ABI for `libobs/audio-monitoring/osx/coreaudio-device-rules.c.h`.
//!
//! `coreaudio-enum-devices.c` calls these inside libobs on macOS. The C file
//! has no `EXPORT`, so the symbols stay hidden (`rust-hidden.d`). The five
//! functions keep the C signatures: `uint32_t` is `u32`, `size_t` is
//! `usize` and `bool` is `bool`. The strings are read only up to their NUL.
//!
//! Only macOS calls them. Elsewhere the shims are built but unused, and
//! hidden all the same.

use std::ffi::{CStr, c_char};

use crate::coreaudio_devices::{count, is_default, is_virtual, listed, names_match};

/// The bytes of a NUL-terminated string, or `None` for NULL.
///
/// # Safety
///
/// `s` is null or a NUL-terminated string that outlives the result.
unsafe fn c_str<'a>(s: *const c_char) -> Option<&'a [u8]> {
    if s.is_null() {
        None
    } else {
        // SAFETY: the caller passes a NUL-terminated string.
        Some(unsafe { CStr::from_ptr(s) }.to_bytes())
    }
}

/// Same contract as C `coreaudio_device_listed`.
#[unsafe(no_mangle)]
pub extern "C" fn coreaudio_device_listed(allow_inputs: bool, output_stream_bytes: u32) -> bool {
    listed(allow_inputs, output_stream_bytes)
}

/// C `coreaudio_device_count`, with the port's intentional difference: the
/// smaller of the two sizes over `id_size`, where the C takes the first.
/// A zero `id_size`, which the C divides by, counts none.
#[unsafe(no_mangle)]
pub extern "C" fn coreaudio_device_count(first_size: u32, second_size: u32, id_size: usize) -> u32 {
    count(first_size, second_size, id_size)
}

/// Same contract as C `coreaudio_device_is_default`. A null `id`, which the
/// C dereferences, is not `"default"`.
///
/// # Safety
///
/// `id` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn coreaudio_device_is_default(id: *const c_char) -> bool {
    // SAFETY: as the caller promises.
    unsafe { c_str(id) }.is_some_and(is_default)
}

/// Same contract as C `coreaudio_device_names_match`: a null name never
/// matches.
///
/// # Safety
///
/// `name1` and `name2` are each null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn coreaudio_device_names_match(
    name1: *const c_char,
    name2: *const c_char,
) -> bool {
    // SAFETY: as the caller promises.
    names_match(unsafe { c_str(name1) }, unsafe { c_str(name2) })
}

/// Same contract as C `coreaudio_device_is_input`: true unless the uid
/// names a virtual device. A null `uid` is an input, as in the C, where
/// `astrstri()` finds nothing in NULL.
///
/// # Safety
///
/// `uid` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn coreaudio_device_is_input(uid: *const c_char) -> bool {
    // SAFETY: as the caller promises.
    !unsafe { c_str(uid) }.is_some_and(is_virtual)
}
