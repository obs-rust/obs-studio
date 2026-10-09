//! Declarations for `oracle/utf8.c`, the original `libobs/util/utf8.c`.

use core::ffi::{c_char, c_int};

/// `wchar_t`: 32-bit on Linux and macOS, 16-bit on Windows.
#[cfg(not(windows))]
pub type WChar = i32;
/// `wchar_t`: 32-bit on Linux and macOS, 16-bit on Windows.
#[cfg(windows)]
pub type WChar = u16;

unsafe extern "C" {
    pub fn oracle_utf8_to_wchar(
        in_: *const c_char,
        insize: usize,
        out: *mut WChar,
        outsize: usize,
        flags: c_int,
    ) -> usize;
    pub fn oracle_wchar_to_utf8(
        in_: *const WChar,
        insize: usize,
        out: *mut c_char,
        outsize: usize,
        flags: c_int,
    ) -> usize;
}
