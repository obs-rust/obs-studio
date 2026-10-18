//! Safe core for the file helpers of `libobs/util/platform-file.c`
//! (obs-rust/obs-studio#262 slice 3): `os_fopen`, `os_wfopen`,
//! `os_fgetsize`, `os_fseeki64`/`os_ftelli64`, `os_fread_mbs`/`os_fread_utf8`,
//! the `os_quick_read_*`/`os_quick_write_*` helpers and `os_get_file_size`.
//! `os_stat` is Windows-only (Unix has `#define os_stat stat`) and lives in
//! [`crate::ffi::platform_file`] over the platform's `_wstat`.
//!
//! The streams are the C library's (`FILE *`), opened and moved through
//! [`CFile`] in the [platform boundary](crate::platform), so C callers can
//! keep using them with stdio.

use core::ffi::{CStr, c_int};

pub use crate::platform::{CFile, SEEK_CUR, SEEK_END, SEEK_SET, WChar};

/// The UTF-8 byte order mark `os_quick_write_utf8_file` writes and
/// `os_fread_utf8` skips.
pub const UTF8_BOM: &[u8; 3] = b"\xEF\xBB\xBF";

/// `os_fopen(path, mode)`.
pub fn fopen(path: &CStr, mode: &CStr) -> Option<CFile> {
    let _ = (path, mode);
    todo!("os_fopen port (#262)")
}

/// `os_wfopen(path, mode)`; `path` has no NUL.
pub fn wfopen(path: &[WChar], mode: &CStr) -> Option<CFile> {
    let _ = (path, mode);
    todo!("os_wfopen port (#262)")
}

/// `os_fseeki64(file, offset, origin)`.
pub fn fseeki64(file: &mut CFile, offset: i64, origin: c_int) -> c_int {
    let _ = (file, offset, origin);
    todo!("os_fseeki64 port (#262)")
}

/// `os_ftelli64(file)`.
pub fn ftelli64(file: &mut CFile) -> i64 {
    let _ = file;
    todo!("os_ftelli64 port (#262)")
}

/// `os_fgetsize(file)`: the size, or -1. The position is restored.
pub fn fgetsize(file: &mut CFile) -> i64 {
    let _ = file;
    todo!("os_fgetsize port (#262)")
}

/// `os_fread_mbs(file, &str)`: the whole file converted from the locale's
/// multibyte encoding to UTF-8, and its length. `None` for an empty or
/// unreadable file.
pub fn fread_mbs(file: &mut CFile) -> Option<Vec<u8>> {
    let _ = file;
    todo!("os_fread_mbs port (#262)")
}

/// `os_fread_utf8(file, &str)`: the whole file without a leading UTF-8 BOM.
/// `None` for an empty, BOM-only or unreadable file.
pub fn fread_utf8(file: &mut CFile) -> Option<Vec<u8>> {
    let _ = file;
    todo!("os_fread_utf8 port (#262)")
}

/// `os_quick_read_mbs_file(path)`.
pub fn quick_read_mbs_file(path: &CStr) -> Option<Vec<u8>> {
    let _ = path;
    todo!("os_quick_read_mbs_file port (#262)")
}

/// `os_quick_read_utf8_file(path)`.
pub fn quick_read_utf8_file(path: &CStr) -> Option<Vec<u8>> {
    let _ = path;
    todo!("os_quick_read_utf8_file port (#262)")
}

/// `os_quick_write_mbs_file(path, str, len)`: `text` converted from UTF-8
/// to the locale's multibyte encoding (`None` for a NULL `str`).
pub fn quick_write_mbs_file(path: &CStr, text: Option<&[u8]>) -> bool {
    let _ = (path, text);
    todo!("os_quick_write_mbs_file port (#262)")
}

/// `os_quick_write_utf8_file(path, str, len, marker)`.
pub fn quick_write_utf8_file(path: &CStr, data: &[u8], marker: bool) -> bool {
    let _ = (path, data, marker);
    todo!("os_quick_write_utf8_file port (#262)")
}

/// Why `os_quick_write_utf8_file_safe` failed; the first two are logged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafeWriteError {
    /// A missing or empty temporary extension.
    InvalidTempExt,
    /// Writing the temporary file (this path) failed.
    WriteFailed(Vec<u8>),
    /// `os_safe_replace` failed.
    ReplaceFailed,
}

/// `os_quick_write_utf8_file_safe(path, str, len, marker, temp_ext,
/// backup_ext)`: writes `<path>.<temp_ext>`, then replaces `path` with it,
/// keeping the old file as `<path>.<backup_ext>` when one is given.
pub fn quick_write_utf8_file_safe(
    path: &[u8],
    data: &[u8],
    marker: bool,
    temp_ext: Option<&[u8]>,
    backup_ext: Option<&[u8]>,
) -> Result<(), SafeWriteError> {
    let _ = (path, data, marker, temp_ext, backup_ext);
    todo!("os_quick_write_utf8_file_safe port (#262)")
}

/// `os_get_file_size(path)`: the size, or -1.
pub fn get_file_size(path: &CStr) -> i64 {
    let _ = path;
    todo!("os_get_file_size port (#262)")
}
