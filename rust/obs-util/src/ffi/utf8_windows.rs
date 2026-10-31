#![cfg(windows)]
//! C ABI shims for `utf8_to_wchar` and `wchar_to_utf8` on Windows, over
//! the Windows branch of `utf8.c` in [`crate::platform::windows::utf8`].
//!
//! The functions only convert pointers to slices (including the `insize ==
//! 0` NUL-terminated scan) and map the two pointer cases the API decides by
//! itself. The symbols are not exported from libobs (`rust-hidden.d`).
//! Linux and macOS use `ffi/utf8.rs`.

use core::ffi::{CStr, c_char, c_int};

use crate::platform::windows::utf8;

/// `wchar_t` on Windows.
pub type WChar = u16;

/// Converts the UTF-8 string `in_` to UTF-16. `flags` are ignored, as in C.
///
/// # Safety
///
/// `in_` must be null or point to `insize` readable bytes (or to a
/// NUL-terminated string when `insize` is 0). `out` must be null or point to
/// `outsize` writable units.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn utf8_to_wchar(
    in_: *const c_char,
    insize: usize,
    out: *mut WChar,
    outsize: usize,
    flags: c_int,
) -> usize {
    let _ = flags;
    // C: insize 0 would strlen(NULL); otherwise the API rejects a null
    // input, so 0 either way.
    if in_.is_null() {
        return 0;
    }
    // The API rejects a null output with a size.
    if out.is_null() && outsize != 0 {
        return 0;
    }

    let input: &[u8] = if insize == 0 {
        // SAFETY: with `insize == 0` the caller guarantees `in_` points to a
        // NUL-terminated string.
        unsafe { CStr::from_ptr(in_) }.to_bytes()
    } else {
        // SAFETY: the caller guarantees `in_` points to `insize` readable
        // bytes.
        unsafe { core::slice::from_raw_parts(in_.cast::<u8>(), insize) }
    };
    let out: Option<&mut [u16]> = if out.is_null() {
        None
    } else {
        // SAFETY: `out` is non-null and the caller guarantees `outsize`
        // writable units (an empty slice when 0, which asks for the size).
        Some(unsafe { core::slice::from_raw_parts_mut(out, outsize) })
    };

    utf8::utf8_to_wchar(input, out)
}

/// Converts the UTF-16 string `in_` to UTF-8. `flags` are ignored, as in C.
///
/// # Safety
///
/// `in_` must be null or point to `insize` readable units (or to a
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
    let _ = flags;
    // C: insize 0 would wcslen(NULL); otherwise the API rejects a null
    // input, so 0 either way.
    if in_.is_null() {
        return 0;
    }
    // The API rejects a null output with a size.
    if out.is_null() && outsize != 0 {
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
    let input: &[u16] = if len == 0 {
        &[]
    } else {
        // SAFETY: `len` is non-zero and the caller guarantees `in_` points
        // to `len` readable units.
        unsafe { core::slice::from_raw_parts(in_, len) }
    };
    let out: Option<&mut [u8]> = if out.is_null() {
        None
    } else {
        // SAFETY: `out` is non-null and the caller guarantees `outsize`
        // writable bytes (an empty slice when 0, which asks for the size).
        Some(unsafe { core::slice::from_raw_parts_mut(out.cast::<u8>(), outsize) })
    };

    utf8::wchar_to_utf8(input, out)
}
