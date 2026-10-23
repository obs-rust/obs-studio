//! C ABI shims for the `EXPORT`ed non-variadic functions of
//! `libobs/util/dstr-libc.c`. The variadic `dstr_printf`, `dstr_catf`,
//! `dstr_vprintf` and `dstr_vcatf` stay C in `util/dstr-variadic.c`.
//!
//! The wide-string functions turn pointers into NUL-terminated slices (NULL
//! is the empty string, as in C) and run the safe core in
//! [`crate::dstr_libc`]. The conversions call the same functions the C
//! does, in the same order: `os_mbs_to_utf8_ptr` and `os_utf8_to_wcs_ptr`
//! (the obs-util port in [`super::platform_conversions`]), the ported
//! `wchar_to_utf8`,
//! and the `dstr.h` inlines `dstr_free`, `dstr_resize` and
//! `dstr_is_empty`, so `array`, `len` and `capacity` match C.

use core::ffi::{c_char, c_int, c_void};
use core::{ptr, slice};

use super::darray::bfree;
use super::dstr::{dstr, ensure_capacity, free};
use super::utf8::wchar_to_utf8;
use crate::dstr_libc::{self as core_libc, WChar};

unsafe extern "C" {
    // util/platform-conversions.c: the obs-util shims in
    // super::platform_conversions, in libobs and in tests alike.
    fn os_mbs_to_utf8_ptr(str: *const c_char, len: usize, pstr: *mut *mut c_char) -> usize;
    fn os_utf8_to_wcs_ptr(str: *const c_char, len: usize, pstr: *mut *mut WChar) -> usize;
}

/// `wstrblank`.
static BLANK: [WChar; 1] = [0];

/// The NUL-terminated wide string at `p`, terminator included, or the
/// empty string for NULL.
///
/// # Safety
///
/// `p` is null or a NUL-terminated wide string that outlives `'a`.
unsafe fn wide<'a>(p: *const WChar) -> &'a [WChar] {
    if p.is_null() {
        return &BLANK;
    }
    let mut n = 0;
    // SAFETY: the string is NUL-terminated, so every read up to the NUL is
    // in bounds.
    while unsafe { *p.add(n) } != 0 {
        n += 1;
    }
    // SAFETY: `n + 1` elements, the NUL included, are readable.
    unsafe { slice::from_raw_parts(p, n + 1) }
}

/// [`wide`], writable.
///
/// # Safety
///
/// `p` is non-null and a writable NUL-terminated wide string that
/// outlives `'a`, not aliased during it.
unsafe fn wide_mut<'a>(p: *mut WChar) -> &'a mut [WChar] {
    // SAFETY: forwarded to the caller.
    let n = unsafe { wide(p) }.len();
    // SAFETY: `n` elements are readable and writable, exclusively.
    unsafe { slice::from_raw_parts_mut(p, n) }
}

/// `int wstrcmpi(const wchar_t *str1, const wchar_t *str2)`.
///
/// # Safety
///
/// Each pointer is null or a NUL-terminated wide string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wstrcmpi(str1: *const WChar, str2: *const WChar) -> c_int {
    // SAFETY: forwarded to the caller.
    unsafe { core_libc::wstrcmpi(wide(str1), wide(str2)) }
}

/// `int wstrcmp_n(const wchar_t *str1, const wchar_t *str2, size_t n)`.
///
/// # Safety
///
/// As for [`wstrcmpi`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wstrcmp_n(str1: *const WChar, str2: *const WChar, n: usize) -> c_int {
    if n == 0 {
        return 0;
    }
    // SAFETY: forwarded to the caller.
    unsafe { core_libc::wstrcmp_n(wide(str1), wide(str2), n) }
}

/// `int wstrcmpi_n(const wchar_t *str1, const wchar_t *str2, size_t n)`.
///
/// # Safety
///
/// As for [`wstrcmpi`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wstrcmpi_n(str1: *const WChar, str2: *const WChar, n: usize) -> c_int {
    if n == 0 {
        return 0;
    }
    // SAFETY: forwarded to the caller.
    unsafe { core_libc::wstrcmpi_n(wide(str1), wide(str2), n) }
}

/// `wchar_t *wstrstri(const wchar_t *str, const wchar_t *find)`: a pointer
/// into `str` (cast from const, `wcsstr`-style), or NULL.
///
/// # Safety
///
/// As for [`wstrcmpi`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wstrstri(str: *const WChar, find: *const WChar) -> *mut WChar {
    if str.is_null() || find.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: both are non-null NUL-terminated strings (caller).
    match unsafe { core_libc::wstrstri(wide(str), wide(find)) } {
        // SAFETY: `off` is at most the offset of the NUL, inside `str`.
        Some(off) => unsafe { str.add(off) }.cast_mut(),
        None => ptr::null_mut(),
    }
}

/// `wchar_t *wcsdepad(wchar_t *str)`: depads in place and returns `str`.
///
/// # Safety
///
/// `str` is null or a writable NUL-terminated wide string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wcsdepad(str: *mut WChar) -> *mut WChar {
    if str.is_null() {
        return str;
    }
    // SAFETY: non-null, writable and NUL-terminated (caller).
    core_libc::wcsdepad(unsafe { wide_mut(str) });
    str
}

/// `dstr_resize` from `dstr.h`.
///
/// # Safety
///
/// `dst` is a valid `dstr` whose array is null or from `bmalloc`.
unsafe fn resize(dst: *mut dstr, num: usize) {
    // SAFETY: forwarded to the caller.
    unsafe {
        if num == 0 {
            free(dst);
            return;
        }
        ensure_capacity(dst, num + 1);
        *(*dst).array.add(num) = 0;
        (*dst).len = num;
    }
}

/// `void dstr_from_mbs(struct dstr *dst, const char *mbstr)`.
///
/// # Safety
///
/// `dst` is a valid `dstr`; `mbstr` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_from_mbs(dst: *mut dstr, mbstr: *const c_char) {
    // SAFETY: forwarded to the caller.
    unsafe {
        free(dst);
        (*dst).len = os_mbs_to_utf8_ptr(mbstr, 0, &raw mut (*dst).array);
    }
}

/// `char *dstr_to_mbs(const struct dstr *str)`: `os_mbs_to_utf8_ptr`, as
/// the C calls it.
///
/// # Safety
///
/// `str` is a valid `dstr`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_to_mbs(str: *const dstr) -> *mut c_char {
    let mut out = ptr::null_mut();
    // SAFETY: forwarded to the caller.
    unsafe { os_mbs_to_utf8_ptr((*str).array, (*str).len, &mut out) };
    out
}

/// `wchar_t *dstr_to_wcs(const struct dstr *str)`.
///
/// # Safety
///
/// `str` is a valid `dstr`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_to_wcs(str: *const dstr) -> *mut WChar {
    let mut out = ptr::null_mut();
    // SAFETY: forwarded to the caller.
    unsafe { os_utf8_to_wcs_ptr((*str).array, (*str).len, &mut out) };
    out
}

/// `void dstr_from_wcs(struct dstr *dst, const wchar_t *wstr)`.
///
/// # Safety
///
/// `dst` is a valid `dstr`; `wstr` is null or a NUL-terminated wide
/// string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_from_wcs(dst: *mut dstr, wstr: *const WChar) {
    // SAFETY: forwarded to the caller; `wchar_to_utf8` takes the host
    // `wchar_t`, whichever sign its declaration spells.
    unsafe {
        let len = wchar_to_utf8(wstr.cast(), 0, ptr::null_mut(), 0, 0);
        if len != 0 {
            resize(dst, len);
            wchar_to_utf8(wstr.cast(), 0, (*dst).array, len + 1, 0);
        } else {
            free(dst);
        }
    }
}

/// `dstr_is_empty` from `dstr.h`.
///
/// # Safety
///
/// `str` is a valid `dstr`.
unsafe fn is_empty(str: *const dstr) -> bool {
    // SAFETY: forwarded to the caller.
    unsafe { (*str).array.is_null() || (*str).len == 0 || *(*str).array == 0 }
}

/// The shared body of `dstr_to_upper` and `dstr_to_lower`.
///
/// # Safety
///
/// `str` is a valid `dstr`.
unsafe fn map_case(str: *mut dstr, map: fn(&mut [WChar])) {
    // SAFETY: forwarded to the caller.
    unsafe {
        if is_empty(str) {
            return;
        }
        let wstr = dstr_to_wcs(str);
        if wstr.is_null() {
            return;
        }
        map(wide_mut(wstr));
        dstr_from_wcs(str, wstr);
        bfree(wstr.cast::<c_void>());
    }
}

/// `void dstr_to_upper(struct dstr *str)`.
///
/// # Safety
///
/// `str` is a valid `dstr`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_to_upper(str: *mut dstr) {
    // SAFETY: forwarded to the caller.
    unsafe { map_case(str, core_libc::to_upper) }
}

/// `void dstr_to_lower(struct dstr *str)`.
///
/// # Safety
///
/// `str` is a valid `dstr`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dstr_to_lower(str: *mut dstr) {
    // SAFETY: forwarded to the caller.
    unsafe { map_case(str, core_libc::to_lower) }
}
