//! C ABI shims for the string conversions of
//! `libobs/util/platform-conversions.c` (`libobs/util/platform.h`), over the
//! safe core in [`crate::platform_conversions`].
//!
//! The shims turn `(str, len)` into the text the C converts, `(dst,
//! dst_size)` into a slice (`None` for a NULL `dst`, which ignores
//! `dst_size`), and give the `_ptr` forms a `bmalloc` allocator. A NULL `str`
//! returns 0 with `dst` untouched, and sets `*pstr` to NULL in the `_ptr`
//! forms, as in C. No conversion writes past `dst_size` elements, unlike the
//! C `os_utf8_to_wcs` (see the core's module docs).

use core::ffi::{CStr, c_char};
use core::ptr;

use crate::ffi::bmem::bmalloc;
use crate::platform_conversions::{self as conv, WChar};

/// The `(str, len)` pair as bytes: `len` bytes, or up to the NUL when `len`
/// is 0. `None` for a NULL `str`.
///
/// # Safety
///
/// `str` is NULL, or readable for `len` bytes, or NUL-terminated when `len`
/// is 0.
unsafe fn text<'a>(str: *const c_char, len: usize) -> Option<&'a [u8]> {
    if str.is_null() {
        None
    } else if len == 0 {
        // SAFETY: the caller guarantees a NUL-terminated string.
        Some(unsafe { CStr::from_ptr(str) }.to_bytes())
    } else {
        // SAFETY: the caller guarantees `len` readable bytes.
        Some(unsafe { core::slice::from_raw_parts(str.cast::<u8>(), len) })
    }
}

/// `wcslen(str)`.
///
/// # Safety
///
/// `str` is a NUL-terminated wide string.
unsafe fn wcslen(str: *const WChar) -> usize {
    let mut len = 0;
    // SAFETY: the string is readable up to and including its NUL.
    while unsafe { *str.add(len) } != 0 {
        len += 1;
    }
    len
}

/// The `(str, len)` pair as wide units: `len` of them, or up to the NUL when
/// `len` is 0. `None` for a NULL `str`.
///
/// # Safety
///
/// `str` is NULL, or readable for `len` units, or NUL-terminated when `len`
/// is 0.
unsafe fn wide_text<'a>(str: *const WChar, len: usize) -> Option<&'a [WChar]> {
    if str.is_null() {
        return None;
    }
    // SAFETY: the caller guarantees a NUL-terminated string when `len` is 0.
    let len = if len == 0 {
        unsafe { wcslen(str) }
    } else {
        len
    };
    // SAFETY: `str` is readable for `len` units.
    Some(unsafe { core::slice::from_raw_parts(str, len) })
}

/// A wide string up to and including its NUL, for the `mbs` side, which reads
/// up to the NUL whatever `len` says. `None` for a NULL `str`.
///
/// # Safety
///
/// `str` is NULL or a NUL-terminated wide string.
unsafe fn wide_cstr<'a>(str: *const WChar) -> Option<&'a [WChar]> {
    if str.is_null() {
        return None;
    }
    // SAFETY: NUL-terminated, so readable for its length plus the NUL.
    Some(unsafe { core::slice::from_raw_parts(str, wcslen(str) + 1) })
}

/// `(dst, dst_size)` as a slice; `None` for a NULL `dst`.
///
/// # Safety
///
/// `dst` is NULL or writable for `dst_size` elements.
unsafe fn out<'a, T>(dst: *mut T, dst_size: usize) -> Option<&'a mut [T]> {
    // SAFETY: the caller guarantees `dst_size` writable elements.
    (!dst.is_null()).then(|| unsafe { core::slice::from_raw_parts_mut(dst, dst_size) })
}

/// A `_ptr` allocator: `n` zeroed elements from `bmalloc`, stored in
/// `*pstr`. The C leaves them uninitialized; zeroing them only changes the
/// bytes after the NUL.
///
/// # Safety
///
/// `pstr` is writable. The caller frees `*pstr` with `bfree`.
unsafe fn alloc_into<'a, T>(pstr: *mut *mut T, n: usize) -> &'a mut [T] {
    let size = n * size_of::<T>();
    // SAFETY: `n` is at least 1, so `size` is not 0; the block is released by
    // the caller's `bfree`.
    let buf = unsafe { bmalloc(size) }.cast::<T>();
    // SAFETY: `buf` holds `size` bytes, now zeroed (a valid value of every
    // integer `T`), and `pstr` is writable.
    unsafe {
        ptr::write_bytes(buf.cast::<u8>(), 0, size);
        *pstr = buf;
        core::slice::from_raw_parts_mut(buf, n)
    }
}

/// # Safety
///
/// `str` is NULL, or readable for `len` bytes (NUL-terminated when `len` is
/// 0). `dst` is NULL or writable for `dst_size` wide characters.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_utf8_to_wcs(
    str: *const c_char,
    len: usize,
    dst: *mut WChar,
    dst_size: usize,
) -> usize {
    // SAFETY: the caller's contract.
    let Some(text) = (unsafe { text(str, len) }) else {
        return 0;
    };
    // SAFETY: the caller's contract.
    conv::utf8_to_wcs(text, unsafe { out(dst, dst_size) })
}

/// # Safety
///
/// `str` is NULL, or readable for `len` wide characters (NUL-terminated when
/// `len` is 0). `dst` is NULL or writable for `dst_size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_wcs_to_utf8(
    str: *const WChar,
    len: usize,
    dst: *mut c_char,
    dst_size: usize,
) -> usize {
    // SAFETY: the caller's contract.
    let Some(text) = (unsafe { wide_text(str, len) }) else {
        return 0;
    };
    // SAFETY: the caller's contract.
    conv::wcs_to_utf8(text, unsafe { out(dst.cast::<u8>(), dst_size) })
}

/// `len` is ignored, as in C.
///
/// # Safety
///
/// `str` is NULL or NUL-terminated. `dst` is NULL or writable for `dst_size`
/// wide characters.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_mbs_to_wcs(
    str: *const c_char,
    len: usize,
    dst: *mut WChar,
    dst_size: usize,
) -> usize {
    let _ = len;
    if str.is_null() {
        return 0;
    }
    // SAFETY: the caller's contract.
    unsafe { conv::mbs_to_wcs(CStr::from_ptr(str), out(dst, dst_size)) }
}

/// `len` is ignored, as in C.
///
/// # Safety
///
/// `str` is NULL or NUL-terminated. `dst` is NULL or writable for `dst_size`
/// bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_wcs_to_mbs(
    str: *const WChar,
    len: usize,
    dst: *mut c_char,
    dst_size: usize,
) -> usize {
    let _ = len;
    // SAFETY: the caller's contract.
    let Some(text) = (unsafe { wide_cstr(str) }) else {
        return 0;
    };
    // SAFETY: the caller's contract.
    conv::wcs_to_mbs(text, unsafe { out(dst.cast::<u8>(), dst_size) })
}

/// # Safety
///
/// `str` as for [`os_utf8_to_wcs`]; `pstr` is writable. `*pstr` is freed with
/// `bfree`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_utf8_to_wcs_ptr(
    str: *const c_char,
    len: usize,
    pstr: *mut *mut WChar,
) -> usize {
    // SAFETY: the caller's contract.
    match unsafe { text(str, len) } {
        // SAFETY: `pstr` is writable.
        Some(text) => conv::utf8_to_wcs_ptr(text, |n| unsafe { alloc_into(pstr, n) }),
        None => {
            // SAFETY: `pstr` is writable.
            unsafe { *pstr = ptr::null_mut() };
            0
        }
    }
}

/// # Safety
///
/// `str` as for [`os_wcs_to_utf8`]; `pstr` is writable. `*pstr` is freed with
/// `bfree`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_wcs_to_utf8_ptr(
    str: *const WChar,
    len: usize,
    pstr: *mut *mut c_char,
) -> usize {
    let pstr = pstr.cast::<*mut u8>();
    // SAFETY: the caller's contract.
    match unsafe { wide_text(str, len) } {
        // SAFETY: `pstr` is writable.
        Some(text) => conv::wcs_to_utf8_ptr(text, |n| unsafe { alloc_into(pstr, n) }),
        None => {
            // SAFETY: `pstr` is writable.
            unsafe { *pstr = ptr::null_mut() };
            0
        }
    }
}

/// `len` is ignored, as in C.
///
/// # Safety
///
/// `str` is NULL or NUL-terminated; `pstr` is writable. `*pstr` is freed with
/// `bfree`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_mbs_to_wcs_ptr(
    str: *const c_char,
    len: usize,
    pstr: *mut *mut WChar,
) -> usize {
    let _ = len;
    if str.is_null() {
        // SAFETY: `pstr` is writable.
        unsafe { *pstr = ptr::null_mut() };
        return 0;
    }
    // SAFETY: `str` is NUL-terminated; `pstr` is writable.
    conv::mbs_to_wcs_ptr(unsafe { CStr::from_ptr(str) }, |n| unsafe {
        alloc_into(pstr, n)
    })
}

/// `len` is ignored, as in C.
///
/// # Safety
///
/// `str` is NULL or NUL-terminated; `pstr` is writable. `*pstr` is freed with
/// `bfree`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_wcs_to_mbs_ptr(
    str: *const WChar,
    len: usize,
    pstr: *mut *mut c_char,
) -> usize {
    let _ = len;
    let pstr = pstr.cast::<*mut u8>();
    // SAFETY: the caller's contract.
    match unsafe { wide_cstr(str) } {
        // SAFETY: `pstr` is writable.
        Some(text) => conv::wcs_to_mbs_ptr(text, |n| unsafe { alloc_into(pstr, n) }),
        None => {
            // SAFETY: `pstr` is writable.
            unsafe { *pstr = ptr::null_mut() };
            0
        }
    }
}

/// # Safety
///
/// `str` as for [`os_utf8_to_wcs`]; `pstr` is writable. `*pstr` is freed with
/// `bfree`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_utf8_to_mbs_ptr(
    str: *const c_char,
    len: usize,
    pstr: *mut *mut c_char,
) -> usize {
    let pstr = pstr.cast::<*mut u8>();
    // SAFETY: the caller's contract.
    match unsafe { text(str, len) } {
        // SAFETY: `pstr` is writable.
        Some(text) => conv::utf8_to_mbs_ptr(text, |n| unsafe { alloc_into(pstr, n) }),
        None => {
            // SAFETY: `pstr` is writable.
            unsafe { *pstr = ptr::null_mut() };
            0
        }
    }
}

/// `len` is ignored, as in C.
///
/// # Safety
///
/// `str` is NULL or NUL-terminated; `pstr` is writable. `*pstr` is freed with
/// `bfree`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_mbs_to_utf8_ptr(
    str: *const c_char,
    len: usize,
    pstr: *mut *mut c_char,
) -> usize {
    let _ = len;
    let pstr = pstr.cast::<*mut u8>();
    if str.is_null() {
        // SAFETY: `pstr` is writable.
        unsafe { *pstr = ptr::null_mut() };
        return 0;
    }
    // SAFETY: `str` is NUL-terminated; `pstr` is writable.
    conv::mbs_to_utf8_ptr(unsafe { CStr::from_ptr(str) }, |n| unsafe {
        alloc_into(pstr, n)
    })
}
