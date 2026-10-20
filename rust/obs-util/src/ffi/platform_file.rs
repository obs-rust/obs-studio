//! C ABI shims for the file helpers of `libobs/util/platform-file.c`
//! (`libobs/util/platform.h`), over the safe core in
//! [`crate::platform_file`].
//!
//! `FILE *` is passed through as an opaque pointer to the C library's own
//! stream. Strings returned to C are `bmalloc`ed and NUL-terminated, as in C.
//!
//! NULL handling follows C where C checks it (a NULL path to `os_fopen`,
//! `os_wfopen`, the `os_quick_*` helpers and `os_get_file_size`; a NULL
//! `str` to `os_quick_write_mbs_file`; a NULL `temp_ext`). Where C would
//! crash (a NULL `FILE *`, `mode`, `pstr` or `os_stat` buffer, a NULL path to
//! `os_quick_write_utf8_file_safe`, or a NULL `str` with a nonzero length),
//! the shim returns the failure value instead.

use core::ffi::{CStr, c_char, c_int, c_void};
use core::mem::ManuallyDrop;
use core::ptr::{self, NonNull};

use crate::platform_file::{self as core_file, CFile, SafeWriteError, WChar};

unsafe extern "C" {
    fn blog(log_level: c_int, format: *const c_char, ...);
}

/// `ptr` as a C string, or `None` for NULL.
///
/// # Safety
///
/// `ptr` is NULL or a valid NUL-terminated string that outlives `'a`.
unsafe fn cstr<'a>(ptr: *const c_char) -> Option<&'a CStr> {
    // SAFETY: the caller guarantees a valid NUL-terminated string.
    (!ptr.is_null()).then(|| unsafe { CStr::from_ptr(ptr) })
}

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

/// A caller's stream, borrowed without closing it.
///
/// # Safety
///
/// `file` is NULL or an open stream from the C library's `fopen` family.
unsafe fn borrow(file: *mut c_void) -> Option<ManuallyDrop<CFile>> {
    // SAFETY: the caller guarantees an open stream; `ManuallyDrop` leaves it open.
    NonNull::new(file).map(|file| ManuallyDrop::new(unsafe { CFile::from_raw(file) }))
}

/// `bmalloc`ed, NUL-terminated copy of `bytes`.
pub(crate) fn to_bmalloc(bytes: &[u8]) -> *mut c_char {
    // SAFETY: allocates `len + 1` bytes, released by the caller's `bfree`.
    let out = unsafe { crate::ffi::bmem::bmalloc(bytes.len() + 1) }.cast::<u8>();
    // SAFETY: `out` holds `len + 1` bytes.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len());
        *out.add(bytes.len()) = 0;
    }
    out.cast()
}

/// Opens `path` (UTF-8; converted to UTF-16 on Windows).
///
/// # Safety
///
/// `path` and `mode` are NULL or valid NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_fopen(path: *const c_char, mode: *const c_char) -> *mut c_void {
    // SAFETY: the caller guarantees valid strings or NULL.
    let (Some(path), Some(mode)) = (unsafe { cstr(path) }, unsafe { cstr(mode) }) else {
        return ptr::null_mut();
    };
    core_file::fopen(path, mode).map_or(ptr::null_mut(), CFile::into_raw)
}

/// Opens the wide-character `path`.
///
/// # Safety
///
/// `path` is NULL or a NUL-terminated wide string; `mode` is NULL or a valid
/// NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_wfopen(path: *const WChar, mode: *const c_char) -> *mut c_void {
    // SAFETY: the caller guarantees a valid string or NULL.
    let Some(mode) = (unsafe { cstr(mode) }) else {
        return ptr::null_mut();
    };
    if path.is_null() {
        return ptr::null_mut();
    }
    let mut len = 0;
    // SAFETY: the caller guarantees a NUL-terminated wide string.
    while unsafe { *path.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: the `len` units before the NUL are readable.
    let path = unsafe { core::slice::from_raw_parts(path, len) };
    core_file::wfopen(path, mode).map_or(ptr::null_mut(), CFile::into_raw)
}

/// The size of `file`, or -1; its position is unchanged.
///
/// # Safety
///
/// `file` is NULL or an open stream.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_fgetsize(file: *mut c_void) -> i64 {
    // SAFETY: the caller guarantees an open stream or NULL.
    unsafe { borrow(file) }.map_or(-1, |mut file| core_file::fgetsize(&mut file))
}

/// `fseeko`/`_fseeki64`.
///
/// # Safety
///
/// `file` is NULL or an open stream.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_fseeki64(file: *mut c_void, offset: i64, origin: c_int) -> c_int {
    // SAFETY: the caller guarantees an open stream or NULL.
    unsafe { borrow(file) }.map_or(-1, |mut file| {
        core_file::fseeki64(&mut file, offset, origin)
    })
}

/// `ftello`/`_ftelli64`.
///
/// # Safety
///
/// `file` is NULL or an open stream.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_ftelli64(file: *mut c_void) -> i64 {
    // SAFETY: the caller guarantees an open stream or NULL.
    unsafe { borrow(file) }.map_or(-1, |mut file| core_file::ftelli64(&mut file))
}

/// Reads all of `file` as multibyte text into `*pstr` (UTF-8, `bmalloc`ed,
/// NULL when empty) and returns its length.
///
/// # Safety
///
/// `file` is NULL or an open stream; `pstr` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_fread_mbs(file: *mut c_void, pstr: *mut *mut c_char) -> usize {
    if pstr.is_null() {
        return 0;
    }
    // SAFETY: the caller guarantees an open stream or NULL.
    let text = unsafe { borrow(file) }.and_then(|mut file| core_file::fread_mbs(&mut file));
    let len = text.as_ref().map_or(0, Vec::len);
    // SAFETY: `pstr` is writable.
    unsafe { *pstr = text.as_deref().map_or(ptr::null_mut(), to_bmalloc) };
    len
}

/// Reads all of `file` as UTF-8 into `*pstr` (`bmalloc`ed, BOM removed, NULL
/// when empty). Returns 0, as C does.
///
/// # Safety
///
/// `file` is NULL or an open stream; `pstr` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_fread_utf8(file: *mut c_void, pstr: *mut *mut c_char) -> usize {
    if pstr.is_null() {
        return 0;
    }
    // SAFETY: the caller guarantees an open stream or NULL.
    let text = unsafe { borrow(file) }.and_then(|mut file| core_file::fread_utf8(&mut file));
    // SAFETY: `pstr` is writable.
    unsafe { *pstr = text.as_deref().map_or(ptr::null_mut(), to_bmalloc) };
    0
}

/// The multibyte file at `path` as UTF-8 (`bmalloc`ed), or NULL.
///
/// # Safety
///
/// `path` is NULL or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_quick_read_mbs_file(path: *const c_char) -> *mut c_char {
    // SAFETY: the caller guarantees a valid string or NULL.
    unsafe { cstr(path) }
        .and_then(core_file::quick_read_mbs_file)
        .as_deref()
        .map_or(ptr::null_mut(), to_bmalloc)
}

/// The UTF-8 file at `path` without its BOM (`bmalloc`ed), or NULL.
///
/// # Safety
///
/// `path` is NULL or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_quick_read_utf8_file(path: *const c_char) -> *mut c_char {
    // SAFETY: the caller guarantees a valid string or NULL.
    unsafe { cstr(path) }
        .and_then(core_file::quick_read_utf8_file)
        .as_deref()
        .map_or(ptr::null_mut(), to_bmalloc)
}

/// Writes the UTF-8 `str` (`len` bytes, or up to its NUL when 0) to `path`
/// in the locale's multibyte encoding.
///
/// # Safety
///
/// `path` is NULL or a valid NUL-terminated string; `str` is NULL or as
/// described for `len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_quick_write_mbs_file(
    path: *const c_char,
    str: *const c_char,
    len: usize,
) -> bool {
    // SAFETY: the caller guarantees a valid string or NULL.
    let Some(path) = (unsafe { cstr(path) }) else {
        return false;
    };
    // SAFETY: the caller guarantees `str` matches `len`.
    core_file::quick_write_mbs_file(path, unsafe { text(str, len) })
}

/// Writes `len` bytes of `str` to `path`, after a UTF-8 BOM when `marker`.
///
/// # Safety
///
/// `path` is NULL or a valid NUL-terminated string; `str` is readable for
/// `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_quick_write_utf8_file(
    path: *const c_char,
    str: *const c_char,
    len: usize,
    marker: bool,
) -> bool {
    // SAFETY: the caller guarantees a valid string or NULL.
    let Some(path) = (unsafe { cstr(path) }) else {
        return false;
    };
    let data = if len == 0 {
        &[][..]
    } else if str.is_null() {
        return false;
    } else {
        // SAFETY: the caller guarantees `len` readable bytes.
        unsafe { core::slice::from_raw_parts(str.cast::<u8>(), len) }
    };
    core_file::quick_write_utf8_file(path, data, marker)
}

/// Writes `str` to `<path>.<temp_ext>`, then replaces `path` with it,
/// keeping the old file as `<path>.<backup_ext>` when `backup_ext` is set.
///
/// # Safety
///
/// `path`, `temp_ext` and `backup_ext` are NULL or valid NUL-terminated
/// strings; `str` is readable for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_quick_write_utf8_file_safe(
    path: *const c_char,
    str: *const c_char,
    len: usize,
    marker: bool,
    temp_ext: *const c_char,
    backup_ext: *const c_char,
) -> bool {
    // SAFETY: the caller guarantees valid strings or NULL.
    let (path, temp_ext, backup_ext) = unsafe { (cstr(path), cstr(temp_ext), cstr(backup_ext)) };
    let data = if len == 0 {
        &[][..]
    } else if str.is_null() {
        return false;
    } else {
        // SAFETY: the caller guarantees `len` readable bytes.
        unsafe { core::slice::from_raw_parts(str.cast::<u8>(), len) }
    };
    // C checks the extension first, then would crash on a NULL path.
    let path = match path {
        Some(path) => path.to_bytes(),
        None if temp_ext.is_some_and(|ext| !ext.is_empty()) => return false,
        None => &[][..],
    };
    match core_file::quick_write_utf8_file_safe(
        path,
        data,
        marker,
        temp_ext.map(CStr::to_bytes),
        backup_ext.map(CStr::to_bytes),
    ) {
        Ok(()) => true,
        Err(SafeWriteError::InvalidTempExt) => {
            // SAFETY: the format has no conversions.
            unsafe {
                blog(
                    crate::base::LOG_ERROR,
                    c"os_quick_write_utf8_file_safe: invalid temporary extension specified"
                        .as_ptr(),
                );
            }
            false
        }
        Err(SafeWriteError::WriteFailed(mut temp_path)) => {
            temp_path.push(0);
            // SAFETY: the format matches the NUL-terminated byte path, which
            // stays live for the call.
            unsafe {
                blog(
                    crate::base::LOG_ERROR,
                    c"os_quick_write_utf8_file_safe: failed to write to %s".as_ptr(),
                    temp_path.as_ptr().cast::<c_char>(),
                );
            }
            false
        }
        Err(SafeWriteError::ReplaceFailed) => false,
    }
}

/// The size of the file at `path`, or -1.
///
/// # Safety
///
/// `path` is NULL or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_get_file_size(path: *const c_char) -> i64 {
    // SAFETY: the caller guarantees a valid string or NULL.
    unsafe { cstr(path) }.map_or(-1, core_file::get_file_size)
}

#[cfg(windows)]
mod windows {
    use core::ffi::{c_char, c_int};

    use crate::platform::windows::file::{self, Stat};

    /// `stat` for a UTF-8 path; `st` is written only on success.
    ///
    /// # Safety
    ///
    /// `path` is NULL or a valid NUL-terminated string; `st` is NULL or
    /// points to a writable `struct stat`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_stat(path: *const c_char, st: *mut Stat) -> c_int {
        // SAFETY: the caller guarantees a valid string or NULL.
        let Some(path) = (unsafe { super::cstr(path) }) else {
            return -1;
        };
        // SAFETY: the caller guarantees a writable `struct stat` or NULL.
        let Some(st) = (unsafe { st.as_mut() }) else {
            return -1;
        };
        file::stat(path.to_bytes(), st)
    }
}
#[cfg(windows)]
pub use windows::os_stat;
