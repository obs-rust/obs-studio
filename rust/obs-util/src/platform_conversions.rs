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
//! gives 0 and an empty string. The same capacity lets C's failed conversion
//! of longer text leave one more unit of partial text after the NUL, at
//! `dst[dst_size - 1]`.
//!
//! The `_ptr` forms take the allocator as a closure, which the C ABI shim
//! makes `bmalloc`: it is called once, with the number of elements to
//! allocate (the size query's result plus one for the NUL).

use core::ffi::CStr;

use crate::platform;

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
    let Some(dst) = dst else {
        return platform::utf8_to_wide(text, None);
    };
    let Some(room) = dst.len().checked_sub(1) else {
        return 0;
    };
    let len = if room != 0 {
        platform::utf8_to_wide(text, Some(&mut dst[..room]))
    } else {
        0
    };
    dst[len] = 0;
    len
}

/// `os_wcs_to_utf8(str, len, dst, dst_size)`, with `text` the units it
/// converts (`len` of them, or up to the NUL when `len` is 0). Returns the
/// number of bytes, without the NUL. Buffers as in [`utf8_to_wcs`].
pub fn wcs_to_utf8(text: &[WChar], dst: Option<&mut [u8]>) -> usize {
    let Some(dst) = dst else {
        return platform::wide_to_utf8(text, None);
    };
    let Some(room) = dst.len().checked_sub(1) else {
        return 0;
    };
    let len = if room != 0 {
        platform::wide_to_utf8(text, Some(&mut dst[..room]))
    } else {
        0
    };
    dst[len] = 0;
    len
}

/// `os_mbs_to_wcs(str, len, dst, dst_size)` in the current `LC_CTYPE`. `len`
/// is ignored, as in C: the whole of `text` is converted.
///
/// Text the locale cannot convert gives 0 (and an empty string). Into a
/// buffer, text that does not fit is cut to `dst_size - 1` units, on a
/// character boundary. An empty buffer is left untouched.
pub fn mbs_to_wcs(text: &CStr, dst: Option<&mut [WChar]>) -> usize {
    let Some(dst) = dst else {
        let len = platform::mbs_to_wide(text, None);
        return if len == usize::MAX { 0 } else { len };
    };
    let Some(room) = dst.len().checked_sub(1) else {
        return 0;
    };
    let mut len = room;
    if len != 0 {
        len = platform::mbs_to_wide(text, Some(&mut dst[..room]));
    }
    if len == usize::MAX {
        len = 0;
    }
    dst[len] = 0;
    len
}

/// `os_wcs_to_mbs(str, len, dst, dst_size)` in the current `LC_CTYPE`. `len`
/// is ignored, as in C: `text` is converted up to its first NUL (all of it if
/// it has none). Buffers as in [`mbs_to_wcs`].
pub fn wcs_to_mbs(text: &[WChar], dst: Option<&mut [u8]>) -> usize {
    let Some(dst) = dst else {
        let len = platform::wide_to_mbs(text, None);
        return if len == usize::MAX { 0 } else { len };
    };
    let Some(room) = dst.len().checked_sub(1) else {
        return 0;
    };
    let mut len = room;
    if len != 0 {
        len = platform::wide_to_mbs(text, Some(&mut dst[..room]));
    }
    if len == usize::MAX {
        len = 0;
    }
    dst[len] = 0;
    len
}

/// `os_utf8_to_wcs_ptr`: [`utf8_to_wcs`]'s size query, a buffer of that many
/// units plus the NUL from `alloc`, then the conversion into it.
pub fn utf8_to_wcs_ptr<'a>(text: &[u8], alloc: impl FnOnce(usize) -> &'a mut [WChar]) -> usize {
    let len = utf8_to_wcs(text, None);
    utf8_to_wcs(text, Some(alloc(len + 1)))
}

/// `os_wcs_to_utf8_ptr`, as [`utf8_to_wcs_ptr`].
pub fn wcs_to_utf8_ptr<'a>(text: &[WChar], alloc: impl FnOnce(usize) -> &'a mut [u8]) -> usize {
    let len = wcs_to_utf8(text, None);
    wcs_to_utf8(text, Some(alloc(len + 1)))
}

/// `os_mbs_to_wcs_ptr`, as [`utf8_to_wcs_ptr`].
pub fn mbs_to_wcs_ptr<'a>(text: &CStr, alloc: impl FnOnce(usize) -> &'a mut [WChar]) -> usize {
    let len = mbs_to_wcs(text, None);
    mbs_to_wcs(text, Some(alloc(len + 1)))
}

/// `os_wcs_to_mbs_ptr`, as [`utf8_to_wcs_ptr`].
pub fn wcs_to_mbs_ptr<'a>(text: &[WChar], alloc: impl FnOnce(usize) -> &'a mut [u8]) -> usize {
    let len = wcs_to_mbs(text, None);
    wcs_to_mbs(text, Some(alloc(len + 1)))
}

/// `os_utf8_to_mbs_ptr(str, len, &dst)`: [`utf8_to_wcs_ptr`], then
/// [`wcs_to_mbs_ptr`] of that up to its first NUL. `len` is honored, as in C.
/// The wide string in between is a Rust `Vec`, where C `bmalloc`s and frees
/// it.
pub fn utf8_to_mbs_ptr<'a>(text: &[u8], alloc: impl FnOnce(usize) -> &'a mut [u8]) -> usize {
    let wide = to_vec(|buf| utf8_to_wcs_ptr(text, move |n| fill(buf, n))).0;
    wcs_to_mbs_ptr(&wide, alloc)
}

/// `os_mbs_to_utf8_ptr(str, len, &dst)`: [`mbs_to_wcs_ptr`], then
/// [`wcs_to_utf8_ptr`] of its result. `len` is ignored, as in C. The wide
/// string in between is a Rust `Vec`, as in [`utf8_to_mbs_ptr`].
pub fn mbs_to_utf8_ptr<'a>(text: &CStr, alloc: impl FnOnce(usize) -> &'a mut [u8]) -> usize {
    let (wide, wlen) = to_vec(|buf| mbs_to_wcs_ptr(text, move |n| fill(buf, n)));
    // C passes the length on, and a `len` of 0 means "up to the NUL" there,
    // which the conversion put at `wlen`.
    wcs_to_utf8_ptr(&wide[..wlen], alloc)
}

/// [`utf8_to_mbs_ptr`] into a `Vec`, without the NUL.
pub fn utf8_to_mbs(text: &[u8]) -> Vec<u8> {
    let (mut out, len) = to_vec(|buf| utf8_to_mbs_ptr(text, move |n| fill(buf, n)));
    out.truncate(len);
    out
}

/// [`mbs_to_utf8_ptr`] into a `Vec`, without the NUL.
pub fn mbs_to_utf8(text: &CStr) -> Vec<u8> {
    let (mut out, len) = to_vec(|buf| mbs_to_utf8_ptr(text, move |n| fill(buf, n)));
    out.truncate(len);
    out
}

/// Runs a `_ptr` conversion whose allocator is [`fill`] on a `Vec`. Returns
/// the buffer, as allocated, and the conversion's result.
fn to_vec<T>(convert: impl FnOnce(&mut Vec<T>) -> usize) -> (Vec<T>, usize) {
    let mut buf = Vec::new();
    let len = convert(&mut buf);
    (buf, len)
}

/// A `_ptr` allocator over a `Vec`: `n` zeroed elements.
fn fill<T: Copy + Default>(buf: &mut Vec<T>, n: usize) -> &mut [T] {
    *buf = vec![T::default(); n];
    buf
}
