//! C stdio `FILE *` streams and locale text conversion for
//! `libobs/util/platform-file.c` on Windows (MSVC branch): `_wfopen`,
//! `_fseeki64`, `_ftelli64` and `_wstat`, with paths converted by
//! `utf8_to_wchar`, and `mbstowcs`/`wcstombs` for the `_mbs_` helpers.
//! The streams are the UCRT's own, so a `FILE *` handed to C callers works
//! with their `fread`/`fclose`.

use core::ffi::{CStr, c_char, c_int, c_long, c_void};
use core::ptr::{self, NonNull};

use super::utf8;

/// `wchar_t` on Windows.
pub type WChar = u16;

/// The UCRT's `struct _stat64i32`, which `struct stat` and `struct _stat`
/// name in libobs's default configuration (64-bit `time_t`, 32-bit size).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stat {
    pub st_dev: u32,
    pub st_ino: u16,
    pub st_mode: u16,
    pub st_nlink: i16,
    pub st_uid: i16,
    pub st_gid: i16,
    pub st_rdev: u32,
    pub st_size: i32,
    pub st_atime: i64,
    pub st_mtime: i64,
    pub st_ctime: i64,
}

unsafe extern "C" {
    fn _wfopen(path: *const WChar, mode: *const WChar) -> *mut c_void;
    fn fseek(file: *mut c_void, offset: c_long, origin: c_int) -> c_int;
    fn _fseeki64(file: *mut c_void, offset: i64, origin: c_int) -> c_int;
    fn _ftelli64(file: *mut c_void) -> i64;
    fn fread(buf: *mut c_void, size: usize, count: usize, file: *mut c_void) -> usize;
    fn fwrite(buf: *const c_void, size: usize, count: usize, file: *mut c_void) -> usize;
    fn fclose(file: *mut c_void) -> c_int;
    fn _wstat64i32(path: *const WChar, st: *mut Stat) -> c_int;
    fn mbstowcs(dst: *mut WChar, src: *const c_char, count: usize) -> usize;
    fn wcstombs(dst: *mut c_char, src: *const WChar, count: usize) -> usize;
    fn _errno() -> *mut c_int;
}

/// `SEEK_SET`.
pub const SEEK_SET: c_int = 0;
/// `SEEK_CUR`.
pub const SEEK_CUR: c_int = 1;
/// `SEEK_END`.
pub const SEEK_END: c_int = 2;

/// An open C stdio stream. Dropping it closes it; [`CFile::into_raw`] and
/// [`core::mem::ManuallyDrop`] keep it open for a C caller.
#[derive(Debug)]
pub struct CFile(NonNull<c_void>);

impl CFile {
    /// Wraps a stream a C caller owns.
    ///
    /// # Safety
    ///
    /// `file` must be an open stream from the UCRT's `fopen` family.
    /// Wrap it in [`core::mem::ManuallyDrop`] unless this value should close it.
    pub unsafe fn from_raw(file: NonNull<c_void>) -> Self {
        Self(file)
    }

    /// Releases the stream to a C caller, which closes it.
    pub fn into_raw(self) -> *mut c_void {
        let file = self.0;
        core::mem::forget(self);
        file.as_ptr()
    }

    /// `os_fopen` on Windows: `utf8_to_wchar` on the whole path, then
    /// [`CFile::open_wide`]. A path that does not convert opens `L""`.
    pub fn open(path: &CStr, mode: &CStr) -> Option<Self> {
        let wide = utf8_to_wcs(path.to_bytes());
        Self::open_wide(&wide[..wide.len() - 1], mode)
    }

    /// `os_wfopen` with MSVC: `_wfopen(path, mode)`, the mode converted by
    /// `utf8_to_wchar`. `path` has no NUL.
    pub fn open_wide(path: &[WChar], mode: &CStr) -> Option<Self> {
        let mut path = path.to_vec();
        path.push(0);
        let mode = utf8_to_wcs(mode.to_bytes());
        // SAFETY: both strings are NUL-terminated for the call.
        NonNull::new(unsafe { _wfopen(path.as_ptr(), mode.as_ptr()) }).map(Self)
    }

    /// `fseek(file, offset, origin)` with a C `long` offset.
    pub fn seek_long(&mut self, offset: c_long, origin: c_int) -> c_int {
        // SAFETY: `self.0` is an open stream.
        unsafe { fseek(self.0.as_ptr(), offset, origin) }
    }

    /// `_fseeki64(file, offset, origin)`.
    pub fn seek(&mut self, offset: i64, origin: c_int) -> c_int {
        // SAFETY: `self.0` is an open stream.
        unsafe { _fseeki64(self.0.as_ptr(), offset, origin) }
    }

    /// `_ftelli64(file)`, -1 on error.
    pub fn tell(&mut self) -> i64 {
        // SAFETY: `self.0` is an open stream.
        unsafe { _ftelli64(self.0.as_ptr()) }
    }

    /// `fread(buf, 1, buf.len(), file)`.
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        // SAFETY: `buf` is writable for `buf.len()` bytes.
        unsafe { fread(buf.as_mut_ptr().cast(), 1, buf.len(), self.0.as_ptr()) }
    }

    /// `fwrite(data, size, count, file)`; `data` holds `size * count` bytes.
    pub fn write(&mut self, data: &[u8], size: usize, count: usize) -> usize {
        debug_assert_eq!(data.len(), size * count);
        // SAFETY: `data` is readable for `size * count` bytes.
        unsafe { fwrite(data.as_ptr().cast(), size, count, self.0.as_ptr()) }
    }

    /// `fclose(file) == 0`.
    pub fn close(self) -> bool {
        let file = self.into_raw();
        // SAFETY: `into_raw` released the only owner of this open stream.
        unsafe { fclose(file) == 0 }
    }
}

impl Drop for CFile {
    fn drop(&mut self) {
        // SAFETY: `self.0` is an open stream this value owns.
        unsafe { fclose(self.0.as_ptr()) };
    }
}

/// The calling thread's `errno`.
pub fn errno() -> c_int {
    // SAFETY: `_errno` returns the calling thread's errno location.
    unsafe { *_errno() }
}

/// Sets the calling thread's `errno`.
pub fn set_errno(value: c_int) {
    // SAFETY: `_errno` returns the calling thread's errno location.
    unsafe { *_errno() = value };
}

/// `os_stat` on Windows: `_wstat` on the path converted by
/// `utf8_to_wchar`, copying the result into `st` only on success. -1 when
/// the path is empty or does not convert.
pub fn stat(path: &[u8], st: &mut Stat) -> c_int {
    let wide = utf8_to_wcs(path);
    if wide.len() == 1 {
        return -1;
    }
    let mut st_w32 = Stat::default();
    // SAFETY: `wide` is NUL-terminated and `st_w32` is a writable `Stat`.
    let ret = unsafe { _wstat64i32(wide.as_ptr(), &mut st_w32) };
    if ret == 0 {
        *st = st_w32;
    }
    ret
}

/// `utf8_to_wchar(text, len, out, out.len(), 0)`: the UTF-16 length of
/// `text` with `out == None`, else that many units written to `out`
/// (`MultiByteToWideChar`, a leading BOM dropped). 0 for an `out` that is
/// empty or too small, which may then hold part of the text.
pub fn utf8_to_wide(text: &[u8], out: Option<&mut [WChar]>) -> usize {
    utf8::utf8_to_wchar(text, out)
}

/// `wchar_to_utf8(wide, len, out, out.len(), 0)`: as [`utf8_to_wide`], the
/// other way (`WideCharToMultiByte`).
pub fn wide_to_utf8(wide: &[WChar], out: Option<&mut [u8]>) -> usize {
    utf8::wchar_to_utf8(wide, out)
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

/// `os_utf8_to_wcs_ptr(text, 0)`: `utf8_to_wchar` counts, then converts.
/// Always NUL-terminated; text that does not convert gives just the NUL.
fn utf8_to_wcs(text: &[u8]) -> Vec<WChar> {
    let count = utf8::utf8_to_wchar(text, None);
    let mut wide = vec![0u16; count + 1];
    let len = if count != 0 {
        utf8::utf8_to_wchar(text, Some(&mut wide))
    } else {
        0
    };
    wide.truncate(len);
    wide.push(0);
    wide
}
