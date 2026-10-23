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

use crate::{platform, platform_conversions};

pub use crate::platform::{CFile, SEEK_CUR, SEEK_END, SEEK_SET, WChar};

/// The UTF-8 byte order mark `os_quick_write_utf8_file` writes and
/// `os_fread_utf8` skips.
pub const UTF8_BOM: &[u8; 3] = b"\xEF\xBB\xBF";

/// `os_fopen(path, mode)`.
pub fn fopen(path: &CStr, mode: &CStr) -> Option<CFile> {
    CFile::open(path, mode)
}

/// `os_wfopen(path, mode)`; `path` has no NUL.
pub fn wfopen(path: &[WChar], mode: &CStr) -> Option<CFile> {
    CFile::open_wide(path, mode)
}

/// `os_fseeki64(file, offset, origin)`.
pub fn fseeki64(file: &mut CFile, offset: i64, origin: c_int) -> c_int {
    file.seek(offset, origin)
}

/// `os_ftelli64(file)`.
pub fn ftelli64(file: &mut CFile) -> i64 {
    file.tell()
}

/// `os_fgetsize(file)`: the size, or -1. The position is restored.
pub fn fgetsize(file: &mut CFile) -> i64 {
    let cur_offset = file.tell();
    if file.seek_long(0, SEEK_END) == -1 {
        return -1;
    }
    let size = file.tell();
    let errval = if size == -1 { platform::errno() } else { 0 };
    if file.seek(cur_offset, SEEK_SET) != 0 && errval != 0 {
        platform::set_errno(errval);
    }
    size
}

/// `os_fread_mbs(file, &str)`: the whole file converted from the locale's
/// multibyte encoding to UTF-8, and its length. `None` for an empty or
/// unreadable file.
pub fn fread_mbs(file: &mut CFile) -> Option<Vec<u8>> {
    let mbs = read_from(file, 0)?;
    // C converts the buffer as a NUL-terminated string.
    let end = mbs.iter().position(|&b| b == 0).unwrap_or(mbs.len());
    let mut text = mbs[..end].to_vec();
    text.push(0);
    let text = CStr::from_bytes_with_nul(&text).unwrap_or_default();
    Some(platform_conversions::mbs_to_utf8(text))
}

/// `os_fread_utf8(file, &str)`: the whole file without a leading UTF-8 BOM.
/// `None` for an empty, BOM-only or unreadable file.
pub fn fread_utf8(file: &mut CFile) -> Option<Vec<u8>> {
    file.seek_long(0, SEEK_END);
    let size = file.tell();
    if size <= 0 {
        return None;
    }
    let mut bom = [0u8; 3];
    file.seek_long(0, SEEK_SET);
    file.read(&mut bom);
    let offset = if &bom == UTF8_BOM { 3 } else { 0 };
    if size == offset {
        return None;
    }
    read_from(file, offset)
}

/// `os_quick_read_mbs_file(path)`.
pub fn quick_read_mbs_file(path: &CStr) -> Option<Vec<u8>> {
    let mut file = fopen(path, c"rb")?;
    fread_mbs(&mut file)
}

/// `os_quick_read_utf8_file(path)`.
pub fn quick_read_utf8_file(path: &CStr) -> Option<Vec<u8>> {
    let mut file = fopen(path, c"rb")?;
    fread_utf8(&mut file)
}

/// `os_quick_write_mbs_file(path, str, len)`: `text` converted from UTF-8
/// to the locale's multibyte encoding (`None` for a NULL `str`).
pub fn quick_write_mbs_file(path: &CStr, text: Option<&[u8]>) -> bool {
    let Some(mut file) = fopen(path, c"wb") else {
        return false;
    };
    let mbs = text
        .map(platform_conversions::utf8_to_mbs)
        .unwrap_or_default();
    let success = mbs.is_empty() || file.write(&mbs, 1, mbs.len()) == mbs.len();
    file.close() && success
}

/// `os_quick_write_utf8_file(path, str, len, marker)`.
pub fn quick_write_utf8_file(path: &CStr, data: &[u8], marker: bool) -> bool {
    let Some(mut file) = fopen(path, c"wb") else {
        return false;
    };
    if marker && file.write(UTF8_BOM, 3, 1) != 1 {
        return false;
    }
    if !data.is_empty() && file.write(data, data.len(), 1) != 1 {
        return false;
    }
    file.close()
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
    let temp_ext = temp_ext
        .filter(|ext| !ext.is_empty())
        .ok_or(SafeWriteError::InvalidTempExt)?;
    let temp_path = with_ext(path, temp_ext);

    let mut temp_c = temp_path.clone();
    temp_c.push(0);
    let temp_c = CStr::from_bytes_until_nul(&temp_c).unwrap_or_default();
    if !quick_write_utf8_file(temp_c, data, marker) {
        return Err(SafeWriteError::WriteFailed(temp_path));
    }

    let backup_path = backup_ext
        .filter(|ext| !ext.is_empty())
        .map(|ext| with_ext(path, ext));
    if platform::safe_replace(path, &temp_path, backup_path.as_deref()) == 0 {
        Ok(())
    } else {
        Err(SafeWriteError::ReplaceFailed)
    }
}

/// `os_get_file_size(path)`: the size, or -1.
pub fn get_file_size(path: &CStr) -> i64 {
    let Some(mut file) = fopen(path, c"rb") else {
        return -1;
    };
    fgetsize(&mut file)
}

/// The rest of `file` from byte `offset` up to the size `ftell` reported at
/// its end, as `os_fread_mbs`/`os_fread_utf8` read it. `None` when the size
/// is 0 or unknown, or nothing reads.
fn read_from(file: &mut CFile, offset: i64) -> Option<Vec<u8>> {
    file.seek_long(0, SEEK_END);
    let size = file.tell();
    let len = usize::try_from(size.checked_sub(offset)?)
        .ok()
        .filter(|&len| len > 0)?;
    // `offset` is 0 or 3, so it fits any C `long`.
    file.seek_long(offset as core::ffi::c_long, SEEK_SET);
    // A directory is seekable on POSIX and `tell` then reports a bogus length
    // (i64::MAX on glibc), so the reported size is not proof the stream can be
    // read. Ask for one byte before allocating it (obs-rust/obs-studio#1768).
    let mut first = [0u8; 1];
    if file.read(&mut first) == 0 {
        return None;
    }
    let mut buf = vec![0u8; len];
    file.seek_long(offset as core::ffi::c_long, SEEK_SET);
    let got = file.read(&mut buf);
    if got == 0 {
        return None;
    }
    buf.truncate(got);
    Some(buf)
}

/// `path` + `.` (unless `ext` starts with one) + `ext`, as the dstr calls
/// in `os_quick_write_utf8_file_safe` build it.
fn with_ext(path: &[u8], ext: &[u8]) -> Vec<u8> {
    let mut out = path.to_vec();
    if ext.first() != Some(&b'.') {
        out.push(b'.');
    }
    out.extend_from_slice(ext);
    out
}
