//! C stdio `FILE *` streams and locale text conversion for
//! `libobs/util/platform-file.c` on Unix: `fopen(3)`, `fseeko(3)`,
//! `ftello(3)`, and `mbstowcs(3)`/`wcstombs(3)` for the `_mbs_` helpers.
//! The streams are the C library's own, so a `FILE *` handed to C callers
//! works with their `fread`/`fclose`.

#![allow(unsafe_code)]

use core::ffi::{CStr, c_char, c_int};
use core::ptr::{self, NonNull};

use crate::utf8;

// `libc` declares `wcstombs` but not `mbstowcs`; both are ISO C.
unsafe extern "C" {
    fn mbstowcs(dst: *mut WChar, src: *const c_char, count: usize) -> usize;
    fn wcstombs(dst: *mut c_char, src: *const WChar, count: usize) -> usize;
}

/// `wchar_t` on Unix.
pub type WChar = libc::wchar_t;

/// An open C stdio stream. Dropping it closes it; [`CFile::into_raw`] and
/// [`core::mem::ManuallyDrop`] keep it open for a C caller.
#[derive(Debug)]
pub struct CFile(NonNull<libc::FILE>);

impl CFile {
    /// Wraps a stream a C caller owns.
    ///
    /// # Safety
    ///
    /// `file` must be an open stream from the C library's `fopen` family.
    /// Wrap it in [`core::mem::ManuallyDrop`] unless this value should close it.
    pub unsafe fn from_raw(file: NonNull<core::ffi::c_void>) -> Self {
        Self(file.cast())
    }

    /// Releases the stream to a C caller, which closes it.
    pub fn into_raw(self) -> *mut core::ffi::c_void {
        let file = self.0;
        core::mem::forget(self);
        file.as_ptr().cast()
    }

    /// `fopen(path, mode)`: the byte path is passed as-is.
    pub fn open(path: &CStr, mode: &CStr) -> Option<Self> {
        // SAFETY: both strings are NUL-terminated for the call.
        NonNull::new(unsafe { libc::fopen(path.as_ptr(), mode.as_ptr()) }).map(Self)
    }

    /// `os_wfopen` on Unix: converts `path` (no NUL) with `wchar_to_utf8`
    /// and opens the result. A path that does not convert opens `""`.
    // wchar_t is i32 on x86-64 Linux and macOS and u32 on aarch64 Linux, so
    // the `as u32` is needed on the first and a no-op on the second.
    #[allow(clippy::unnecessary_cast)]
    pub fn open_wide(path: &[WChar], mode: &CStr) -> Option<Self> {
        let wide: Vec<u32> = path.iter().map(|&c| c as u32).collect();
        let mut bytes = wcs_to_utf8(&wide);
        bytes.push(0);
        let path = CStr::from_bytes_until_nul(&bytes).unwrap_or_default();
        Self::open(path, mode)
    }

    /// `fseek(file, offset, origin)` with a C `long` offset.
    pub fn seek_long(&mut self, offset: libc::c_long, origin: c_int) -> c_int {
        // SAFETY: `self.0` is an open stream.
        unsafe { libc::fseek(self.0.as_ptr(), offset, origin) }
    }

    /// `fseeko(file, offset, origin)`.
    pub fn seek(&mut self, offset: i64, origin: c_int) -> c_int {
        // SAFETY: `self.0` is an open stream.
        unsafe { libc::fseeko(self.0.as_ptr(), offset as libc::off_t, origin) }
    }

    /// `ftello(file)`, -1 on error.
    #[allow(clippy::useless_conversion)] // `off_t` is 32-bit on some Unix targets
    pub fn tell(&mut self) -> i64 {
        // SAFETY: `self.0` is an open stream.
        i64::from(unsafe { libc::ftello(self.0.as_ptr()) })
    }

    /// `fread(buf, 1, buf.len(), file)`.
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        // SAFETY: `buf` is writable for `buf.len()` bytes.
        unsafe { libc::fread(buf.as_mut_ptr().cast(), 1, buf.len(), self.0.as_ptr()) }
    }

    /// `fwrite(data, size, count, file)`; `data` holds `size * count` bytes.
    pub fn write(&mut self, data: &[u8], size: usize, count: usize) -> usize {
        debug_assert_eq!(data.len(), size * count);
        // SAFETY: `data` is readable for `size * count` bytes.
        unsafe { libc::fwrite(data.as_ptr().cast(), size, count, self.0.as_ptr()) }
    }

    /// `fclose(file) == 0`.
    pub fn close(self) -> bool {
        let file = self.into_raw();
        // SAFETY: `into_raw` released the only owner of this open stream.
        unsafe { libc::fclose(file.cast()) == 0 }
    }
}

impl Drop for CFile {
    fn drop(&mut self) {
        // SAFETY: `self.0` is an open stream this value owns.
        unsafe { libc::fclose(self.0.as_ptr()) };
    }
}

pub use libc::{SEEK_CUR, SEEK_END, SEEK_SET};

/// The calling thread's `errno`.
pub fn errno() -> c_int {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

/// Sets the calling thread's `errno`.
pub fn set_errno(value: c_int) {
    // SAFETY: the C library returns the calling thread's errno location.
    unsafe {
        cfg_select! {
            any(target_os = "macos", target_os = "freebsd") => { *libc::__error() = value; }
            target_os = "openbsd" => { *libc::__errno() = value; }
            _ => { *libc::__errno_location() = value; }
        }
    }
}

/// `wchar_t` as the UTF-8 decoder's code units: the same 32 bits, signed on
/// some targets.
fn wide_as_units(wide: &[WChar]) -> &[u32] {
    // SAFETY: `WChar` is a 32-bit integer like `u32`, with the same size and
    // alignment; every bit pattern is valid for both.
    unsafe { core::slice::from_raw_parts(wide.as_ptr().cast::<u32>(), wide.len()) }
}

fn wide_as_units_mut(wide: &mut [WChar]) -> &mut [u32] {
    // SAFETY: as in `wide_as_units`; the borrow is unique.
    unsafe { core::slice::from_raw_parts_mut(wide.as_mut_ptr().cast::<u32>(), wide.len()) }
}

/// `utf8_to_wchar(text, len, out, out.len(), 0)`: the wide length of `text`
/// with `out == None`, else that many units written to `out`. 0 for invalid
/// input or an `out` that is empty or too small, which may then hold part of
/// the text.
pub fn utf8_to_wide(text: &[u8], out: Option<&mut [WChar]>) -> usize {
    utf8::utf8_to_wchar(text, out.map(wide_as_units_mut), 0)
}

/// `wchar_to_utf8(wide, len, out, out.len(), 0)`: as [`utf8_to_wide`], the
/// other way.
pub fn wide_to_utf8(wide: &[WChar], out: Option<&mut [u8]>) -> usize {
    utf8::wchar_to_utf8(wide_as_units(wide), out, 0)
}

/// `mbstowcs(out, text, out.len())`, or `mbstowcs(NULL, text, 0)` with
/// `out == None`, in the current `LC_CTYPE`. `usize::MAX` (`(size_t)-1`) for
/// text the locale cannot convert.
pub fn mbs_to_wide(text: &CStr, out: Option<&mut [WChar]>) -> usize {
    let (dst, count) = match out {
        Some(out) => (out.as_mut_ptr(), out.len()),
        None => (ptr::null_mut(), 0),
    };
    // SAFETY: `text` is NUL-terminated; `dst` is null, or holds `count` units.
    unsafe { mbstowcs(dst, text.as_ptr(), count) }
}

/// `wcstombs(out, wide, out.len())`, or `wcstombs(NULL, wide, 0)` with
/// `out == None`, in the current `LC_CTYPE`. The text ends at the first NUL
/// in `wide` (a copy with one added if it has none). `usize::MAX` for text the
/// locale cannot convert.
pub fn wide_to_mbs(wide: &[WChar], out: Option<&mut [u8]>) -> usize {
    let copy;
    let wide = if wide.contains(&0) {
        wide
    } else {
        copy = [wide, &[0]].concat();
        &copy[..]
    };
    let (dst, count) = match out {
        Some(out) => (out.as_mut_ptr().cast::<c_char>(), out.len()),
        None => (ptr::null_mut(), 0),
    };
    // SAFETY: `wide` holds a NUL; `dst` is null, or holds `count` bytes.
    unsafe { wcstombs(dst, wide.as_ptr(), count) }
}

/// `os_wcs_to_utf8` over the whole of `wide` (no NUL): `wchar_to_utf8`
/// counts, then converts into a buffer of exactly that many bytes.
fn wcs_to_utf8(wide: &[u32]) -> Vec<u8> {
    let len = utf8::wchar_to_utf8(wide, None, 0);
    let mut out = vec![0u8; len];
    let len = if len != 0 {
        utf8::wchar_to_utf8(wide, Some(&mut out), 0)
    } else {
        0
    };
    out.truncate(len);
    out
}
