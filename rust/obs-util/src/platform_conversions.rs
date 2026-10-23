//! Safe core for the string conversions of `libobs/util/platform-conversions.c`
//! (obs-rust/obs-studio#262 slice 1): `os_utf8_to_wcs`, `os_wcs_to_utf8`,
//! `os_mbs_to_wcs`, `os_wcs_to_mbs`, their `_ptr` forms, `os_utf8_to_mbs_ptr`
//! and `os_mbs_to_utf8_ptr`.
//!
//! The conversions themselves are the platform's, as in C: the UTF-8 ones go
//! through `utf8_to_wchar` / `wchar_to_utf8` (libobs's own decoder off
//! Windows, `MultiByteToWideChar` / `WideCharToMultiByte` on Windows), the
//! `mbs` ones through the C library's `mbstowcs` / `wcstombs` in the current
//! `LC_CTYPE`. See [`crate::platform`]. This module holds the buffer
//! handling around them.
//!
//! A `dst` is the C's `dst` and `dst_size` as a slice: `None` for a NULL
//! `dst` (a size query, which ignores `dst_size`), `Some` with the buffer's
//! length otherwise. The NUL always goes inside it.
//!
//! Intentional difference: `os_utf8_to_wcs` in C gives `utf8_to_wchar` a
//! capacity of `dst_size` rather than `dst_size - 1`, so text exactly
//! `dst_size` wide characters long converts in full and its NUL is written at
//! `dst[dst_size]`, one element past the buffer. C writes one element past the
//! buffer; Rust doesn't. Here the capacity is `dst_size - 1`, as
//! `os_wcs_to_utf8` has it, so that text is too long like any other, and
//! gives 0 and an empty string.
//!
//! The `_ptr` forms take the allocator as a closure, which the C ABI shim
//! makes `bmalloc`: it is called once, with the number of elements to
//! allocate (the size query's result plus one for the NUL).

use core::ffi::CStr;

pub use crate::platform::WChar;

/// `os_utf8_to_wcs(str, len, dst, dst_size)`, with `text` the bytes it
/// converts (`len` of them, or up to the NUL when `len` is 0). Returns the
/// number of wide characters, without the NUL.
///
/// A size query gives the length the conversion needs. Into a buffer, text
/// that is invalid or does not fit before the NUL gives 0 and an empty
/// string (the buffer may hold part of the text after that NUL). An empty
/// buffer is left untouched.
pub fn utf8_to_wcs(text: &[u8], dst: Option<&mut [WChar]>) -> usize {
    todo!()
}

/// `os_wcs_to_utf8(str, len, dst, dst_size)`, with `text` the units it
/// converts (`len` of them, or up to the NUL when `len` is 0). Returns the
/// number of bytes, without the NUL. Buffers as in [`utf8_to_wcs`].
pub fn wcs_to_utf8(text: &[WChar], dst: Option<&mut [u8]>) -> usize {
    todo!()
}

/// `os_mbs_to_wcs(str, len, dst, dst_size)` in the current `LC_CTYPE`. `len`
/// is ignored, as in C: the whole of `text` is converted.
///
/// Text the locale cannot convert gives 0 (and an empty string). Into a
/// buffer, text that does not fit is cut to `dst_size - 1` units, on a
/// character boundary. An empty buffer is left untouched.
pub fn mbs_to_wcs(text: &CStr, dst: Option<&mut [WChar]>) -> usize {
    todo!()
}

/// `os_wcs_to_mbs(str, len, dst, dst_size)` in the current `LC_CTYPE`. `len`
/// is ignored, as in C: `text` is converted up to its first NUL (all of it if
/// it has none). Buffers as in [`mbs_to_wcs`].
pub fn wcs_to_mbs(text: &[WChar], dst: Option<&mut [u8]>) -> usize {
    todo!()
}

/// `os_utf8_to_wcs_ptr`: [`utf8_to_wcs`]'s size query, a buffer of that many
/// units plus the NUL from `alloc`, then the conversion into it.
pub fn utf8_to_wcs_ptr<'a>(text: &[u8], alloc: impl FnOnce(usize) -> &'a mut [WChar]) -> usize {
    todo!()
}

/// `os_wcs_to_utf8_ptr`, as [`utf8_to_wcs_ptr`].
pub fn wcs_to_utf8_ptr<'a>(text: &[WChar], alloc: impl FnOnce(usize) -> &'a mut [u8]) -> usize {
    todo!()
}

/// `os_mbs_to_wcs_ptr`, as [`utf8_to_wcs_ptr`].
pub fn mbs_to_wcs_ptr<'a>(text: &CStr, alloc: impl FnOnce(usize) -> &'a mut [WChar]) -> usize {
    todo!()
}

/// `os_wcs_to_mbs_ptr`, as [`utf8_to_wcs_ptr`].
pub fn wcs_to_mbs_ptr<'a>(text: &[WChar], alloc: impl FnOnce(usize) -> &'a mut [u8]) -> usize {
    todo!()
}

/// `os_utf8_to_mbs_ptr(str, len, &dst)`: [`utf8_to_wcs_ptr`], then
/// [`wcs_to_mbs_ptr`] of that up to its first NUL. `len` is honored, as in C.
/// The wide string in between is a Rust `Vec`, where C `bmalloc`s and frees
/// it.
pub fn utf8_to_mbs_ptr<'a>(text: &[u8], alloc: impl FnOnce(usize) -> &'a mut [u8]) -> usize {
    todo!()
}

/// `os_mbs_to_utf8_ptr(str, len, &dst)`: [`mbs_to_wcs_ptr`], then
/// [`wcs_to_utf8_ptr`] of its result. `len` is ignored, as in C. The wide
/// string in between is a Rust `Vec`, as in [`utf8_to_mbs_ptr`].
pub fn mbs_to_utf8_ptr<'a>(text: &CStr, alloc: impl FnOnce(usize) -> &'a mut [u8]) -> usize {
    todo!()
}

/// [`utf8_to_mbs_ptr`] into a `Vec`, without the NUL.
pub fn utf8_to_mbs(text: &[u8]) -> Vec<u8> {
    todo!()
}

/// [`mbs_to_utf8_ptr`] into a `Vec`, without the NUL.
pub fn mbs_to_utf8(text: &CStr) -> Vec<u8> {
    todo!()
}
