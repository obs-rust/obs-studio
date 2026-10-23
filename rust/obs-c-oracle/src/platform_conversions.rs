//! The string conversions of `libobs/util/platform-conversions.c`, compiled
//! unmodified by `oracle/platform.c` with every export renamed `oracle_*`.
//! They allocate with the oracle's own `oracle_bmalloc` (free their `_ptr`
//! results with [`crate::bmem::oracle_bfree`]) and convert with the oracle's
//! `oracle_utf8_to_wchar` / `oracle_wchar_to_utf8` and the C library's
//! `mbstowcs` / `wcstombs`.
//!
//! `oracle_os_utf8_to_wcs` writes its NUL one element past `dst_size` for
//! text exactly `dst_size` wide characters long; callers give it a buffer
//! with room for that element.

use core::ffi::c_char;

pub use crate::utf8::WChar;

unsafe extern "C" {
    pub fn oracle_os_utf8_to_wcs(
        str: *const c_char,
        len: usize,
        dst: *mut WChar,
        dst_size: usize,
    ) -> usize;
    pub fn oracle_os_wcs_to_utf8(
        str: *const WChar,
        len: usize,
        dst: *mut c_char,
        dst_size: usize,
    ) -> usize;
    pub fn oracle_os_mbs_to_wcs(
        str: *const c_char,
        len: usize,
        dst: *mut WChar,
        dst_size: usize,
    ) -> usize;
    pub fn oracle_os_wcs_to_mbs(
        str: *const WChar,
        len: usize,
        dst: *mut c_char,
        dst_size: usize,
    ) -> usize;
    pub fn oracle_os_utf8_to_wcs_ptr(
        str: *const c_char,
        len: usize,
        pstr: *mut *mut WChar,
    ) -> usize;
    pub fn oracle_os_wcs_to_utf8_ptr(
        str: *const WChar,
        len: usize,
        pstr: *mut *mut c_char,
    ) -> usize;
    pub fn oracle_os_mbs_to_wcs_ptr(str: *const c_char, len: usize, pstr: *mut *mut WChar)
    -> usize;
    pub fn oracle_os_wcs_to_mbs_ptr(str: *const WChar, len: usize, pstr: *mut *mut c_char)
    -> usize;
    pub fn oracle_os_utf8_to_mbs_ptr(
        str: *const c_char,
        len: usize,
        pstr: *mut *mut c_char,
    ) -> usize;
    pub fn oracle_os_mbs_to_utf8_ptr(
        str: *const c_char,
        len: usize,
        pstr: *mut *mut c_char,
    ) -> usize;
}
