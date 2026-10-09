#![cfg(not(windows))]
//! C ABI shims for `utf8_to_wchar` and `wchar_to_utf8` in `libobs/util/utf8.h`.
//!
//! The functions only convert pointers to slices (including the `insize == 0`
//! NUL-terminated scan) and delegate to the safe core in [`crate::utf8`]. The
//! symbols are not exported from libobs. On Windows `utf8.c` stays C.

use core::ffi::{CStr, c_char, c_int};

use crate::utf8;

/// `wchar_t` on Linux and macOS.
pub type WChar = i32;

/// Converts the UTF-8 string `in_` to wide characters.
///
/// # Safety
///
/// `in_` must be null or point to `insize` readable bytes (or to a
/// NUL-terminated string when `insize` is 0). `out` must be null or point to
/// `outsize` writable wide characters.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn utf8_to_wchar(
    in_: *const c_char,
    insize: usize,
    out: *mut WChar,
    outsize: usize,
    flags: c_int,
) -> usize {
    if in_.is_null() || (outsize == 0 && !out.is_null()) {
        return 0;
    }

    let len = if insize == 0 {
        // SAFETY: with `insize == 0` the caller guarantees `in_` points to a
        // NUL-terminated string.
        unsafe { CStr::from_ptr(in_) }.to_bytes().len()
    } else {
        insize
    };
    let input: &[u8] = if len == 0 {
        &[]
    } else {
        // SAFETY: `len` is non-zero and the caller guarantees `in_` points to
        // `len` readable bytes.
        unsafe { core::slice::from_raw_parts(in_.cast::<u8>(), len) }
    };

    let out: Option<&mut [u32]> = if out.is_null() {
        None
    } else {
        // SAFETY: `out` is non-null and `outsize` is non-zero (checked
        // above); the caller guarantees `outsize` writable wide characters.
        Some(unsafe { core::slice::from_raw_parts_mut(out.cast::<u32>(), outsize) })
    };

    utf8::utf8_to_wchar(input, out, flags)
}

/// Converts the wide string `in_` to UTF-8.
///
/// # Safety
///
/// `in_` must be null or point to `insize` readable wide characters (or to a
/// zero-terminated wide string when `insize` is 0). `out` must be null or
/// point to `outsize` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wchar_to_utf8(
    in_: *const WChar,
    insize: usize,
    out: *mut c_char,
    outsize: usize,
    flags: c_int,
) -> usize {
    if in_.is_null() || (outsize == 0 && !out.is_null()) {
        return 0;
    }

    let len = if insize == 0 {
        let mut n = 0usize;
        // SAFETY: with `insize == 0` the caller guarantees `in_` points to a
        // zero-terminated wide string, so every read before the terminator
        // is in bounds.
        while unsafe { *in_.add(n) } != 0 {
            n += 1;
        }
        n
    } else {
        insize
    };
    let input: &[u32] = if len == 0 {
        &[]
    } else {
        // SAFETY: `len` is non-zero and the caller guarantees `in_` points to
        // `len` readable wide characters.
        unsafe { core::slice::from_raw_parts(in_.cast::<u32>(), len) }
    };

    let out: Option<&mut [u8]> = if out.is_null() {
        None
    } else {
        // SAFETY: `out` is non-null and `outsize` is non-zero (checked
        // above); the caller guarantees `outsize` writable bytes.
        Some(unsafe { core::slice::from_raw_parts_mut(out.cast::<u8>(), outsize) })
    };

    utf8::wchar_to_utf8(input, out, flags)
}
