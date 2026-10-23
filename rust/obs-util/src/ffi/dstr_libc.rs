//! C ABI shims for the `EXPORT`ed functions of `libobs/util/dstr-libc.c`
//! other than the printf family.
//!
//! `dstr_printf`, `dstr_catf`, `dstr_vprintf` and `dstr_vcatf` stay C in
//! `libobs/util/dstr-variadic.c` (stable Rust cannot define a C variadic
//! function or take a `va_list`).
//!
//! The wide compares, `wstrstri`, `wcsdepad` and the case loop delegate to
//! the safe core in [`crate::dstr_libc`]. The case mapping is the C
//! library's `towupper` / `towlower`, as in C, so the results follow the
//! C library and the current `LC_CTYPE` locale exactly. The conversions call
//! what the C calls, in the C's order: `os_mbs_to_utf8_ptr` and
//! `os_utf8_to_wcs_ptr` (libobs's `util/platform.c`, still C) and
//! `wchar_to_utf8` (the Rust port in [`super::utf8`]). Every buffer comes
//! from the `bmalloc` family, so C can free or resize it.

// wchar_t and wint_t are both u16 on Windows: the `as` conversions below are
// needed on Linux and macOS (i32 / u32) and no-ops there, where clippy calls
// them unnecessary.
#![allow(clippy::unnecessary_cast)]

use core::ffi::{c_char, c_int, c_void};
use core::{ptr, slice};

use super::darray::bfree;
use super::dstr::{dstr, ensure_capacity, free};
use super::utf8::{WChar, wchar_to_utf8};
use crate::dstr_libc as core_wide;

/// `wint_t`.
#[cfg(windows)]
type WInt = u16;
/// `wint_t` (`unsigned int` on Linux, `int` on macOS: the same 32 bits).
#[cfg(not(windows))]
type WInt = u32;

unsafe extern "C" {
    // The C library: the result depends on the current LC_CTYPE locale.
    fn towupper(wc: WInt) -> WInt;
    fn towlower(wc: WInt) -> WInt;
    // libobs util/platform.c (util/platform-conversions.c).
    fn os_mbs_to_utf8_ptr(str: *const c_char, len: usize, pstr: *mut *mut c_char) -> usize;
    fn os_utf8_to_wcs_ptr(str: *const c_char, len: usize, pstr: *mut *mut WChar) -> usize;
}

/// `(wchar_t)towupper((wint_t)c)`.
fn upper(c: WChar) -> WChar {
    // SAFETY: towupper takes any wint_t value and has no other inputs.
    unsafe { towupper(c as WInt) as WChar }
}

/// `(wchar_t)towlower((wint_t)c)`.
fn lower(c: WChar) -> WChar {
    // SAFETY: towlower takes any wint_t value and has no other inputs.
    unsafe { towlower(c as WInt) as WChar }
}

/// Unit `i` of `p`, with NULL read as the empty string (`wstrblank`).
///
/// # Safety
///
/// `p` is null or readable at `i`.
unsafe fn unit(p: *const WChar, i: usize) -> WChar {
    if p.is_null() {
        0
    } else {
        // SAFETY: the caller guarantees `p` is readable at `i`.
        unsafe { *p.add(i) }
    }
}

/// The wide string at `p`, without its NUL.
///
/// # Safety
///
/// `p` is a NUL-terminated wide string.
unsafe fn wide<'a>(p: *const WChar) -> &'a [WChar] {
    let mut n = 0;
    // SAFETY: the string is NUL-terminated, so every unit up to the NUL is
    // readable.
    while unsafe { *p.add(n) } != 0 {
        n += 1;
    }
    // SAFETY: the first `n` units are readable.
    unsafe { slice::from_raw_parts(p, n) }
}

/// # Safety
///
/// Each non-null pointer is readable through the first differing unit, the
/// first NUL, or `n` units when `n` is `Some`.
unsafe fn compare(
    a: *const WChar,
    b: *const WChar,
    n: Option<usize>,
    fold: fn(WChar) -> WChar,
) -> c_int {
    core_wide::compare_by(n, fold, |i| {
        // SAFETY: the core reads units in order and stops at a difference,
        // a NUL or n, which the caller guarantees are readable.
        unsafe { (unit(a, i), unit(b, i)) }
    })
}

/// # Safety
///
/// `str1` and `str2` are each null or NUL-terminated wide strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wstrcmpi(str1: *const WChar, str2: *const WChar) -> c_int {
    // SAFETY: forwarded to the caller.
    unsafe { compare(str1, str2, None, upper) }
}

/// # Safety
///
/// Each non-null pointer is readable through the first differing unit, a
/// NUL, or `n` units, whichever comes first.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wstrcmp_n(str1: *const WChar, str2: *const WChar, n: usize) -> c_int {
    // SAFETY: forwarded to the caller.
    unsafe { compare(str1, str2, Some(n), |c| c) }
}

/// # Safety
///
/// Each non-null pointer is readable through the first differing unit, a
/// NUL, or `n` units, whichever comes first.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wstrcmpi_n(str1: *const WChar, str2: *const WChar, n: usize) -> c_int {
    // SAFETY: forwarded to the caller.
    unsafe { compare(str1, str2, Some(n), upper) }
}

/// # Safety
///
/// `str` and `find` are each null or NUL-terminated wide strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wstrstri(str: *const WChar, find: *const WChar) -> *mut WChar {
    if str.is_null() || find.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: both are NUL-terminated per the contract.
    let (s, f) = unsafe { (wide(str), wide(find)) };
    match core_wide::wstrstri(s, f, upper) {
        // SAFETY: the offset is at most the length of `str`, so within it
        // or at its NUL.
        Some(off) => unsafe { str.add(off) as *mut WChar },
        None => ptr::null_mut(),
    }
}

/// # Safety
///
/// `str` is null or a writable NUL-terminated wide string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wcsdepad(str: *mut WChar) -> *mut WChar {
    if str.is_null() {
        return str;
    }
    // SAFETY: the string and its NUL are writable per the contract.
    unsafe {
        let n = wide(str).len();
        core_wide::wcsdepad(slice::from_raw_parts_mut(str, n + 1));
    }
    str
}

/// `dstr_is_empty`.
///
/// # Safety
///
/// `d` points at a valid `dstr`.
unsafe fn is_empty(d: *const dstr) -> bool {
    // SAFETY: the caller guarantees `d`; a non-null array with `len > 0`
    // holds at least one byte.
    unsafe { (*d).array.is_null() || (*d).len == 0 || *(*d).array == 0 }
}

/// # Safety
///
/// `dst` points at a valid `dstr` whose array is null or from the `bmalloc`
/// family; `mbstr` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_from_mbs(dst: *mut dstr, mbstr: *const c_char) {
    // SAFETY: forwarded to the caller. Like C, the converted buffer is moved
    // in and `capacity` stays 0.
    unsafe {
        free(dst);
        (*dst).len = os_mbs_to_utf8_ptr(mbstr, 0, &mut (*dst).array);
    }
}

/// # Safety
///
/// `str` points at a valid `dstr` whose array is null or a NUL-terminated
/// string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_to_mbs(str: *const dstr) -> *mut c_char {
    let mut out = ptr::null_mut();
    // SAFETY: forwarded to the caller.
    unsafe { os_mbs_to_utf8_ptr((*str).array, (*str).len, &mut out) };
    out
}

/// # Safety
///
/// `str` points at a valid `dstr` whose array is null or holds `len`
/// readable bytes (a NUL-terminated string when `len` is 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_to_wcs(str: *const dstr) -> *mut WChar {
    let mut out = ptr::null_mut();
    // SAFETY: forwarded to the caller.
    unsafe { os_utf8_to_wcs_ptr((*str).array, (*str).len, &mut out) };
    out
}

/// # Safety
///
/// `dst` points at a valid `dstr` whose array is null or from the `bmalloc`
/// family; `wstr` is null or a NUL-terminated wide string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_from_wcs(dst: *mut dstr, wstr: *const WChar) {
    // SAFETY: forwarded to the caller. The first call only counts; the
    // second writes at most `len + 1` bytes, which `dstr_resize` made room
    // for.
    unsafe {
        let len = wchar_to_utf8(wstr, 0, ptr::null_mut(), 0, 0);
        if len != 0 {
            // dstr_resize(dst, len)
            ensure_capacity(dst, len + 1);
            *(*dst).array.add(len) = 0;
            (*dst).len = len;
            wchar_to_utf8(wstr, 0, (*dst).array, len + 1, 0);
        } else {
            free(dst);
        }
    }
}

/// `dstr_to_upper` / `dstr_to_lower`.
///
/// # Safety
///
/// As for [`dstr_to_wcs`] and [`dstr_from_wcs`].
unsafe fn map_case(str: *mut dstr, map: fn(WChar) -> WChar) {
    // SAFETY: forwarded to the caller.
    unsafe {
        if is_empty(str) {
            return;
        }
        let wstr = dstr_to_wcs(str);
        if wstr.is_null() {
            return;
        }
        let n = wide(wstr).len();
        core_wide::map_case(slice::from_raw_parts_mut(wstr, n), map);
        dstr_from_wcs(str, wstr);
        bfree(wstr as *mut c_void);
    }
}

/// # Safety
///
/// `str` points at a valid `dstr` whose array is null or from the `bmalloc`
/// family and holds `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_to_upper(str: *mut dstr) {
    // SAFETY: forwarded to the caller.
    unsafe { map_case(str, upper) }
}

/// # Safety
///
/// `str` points at a valid `dstr` whose array is null or from the `bmalloc`
/// family and holds `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_to_lower(str: *mut dstr) {
    // SAFETY: forwarded to the caller.
    unsafe { map_case(str, lower) }
}
