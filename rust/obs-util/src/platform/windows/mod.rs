//! Windows side of the [platform boundary](super). [`pipe`], [`sync`] and
//! [`utf8`] call Win32 directly, declared against kernel32.lib.

pub mod crt;
pub mod file;
pub mod host;
pub mod pipe;
pub mod sync;
pub mod utf8;

pub use pipe::Pipe;
pub use sync::{Event, Sem, set_thread_name};

use std::path::PathBuf;

/// libobs `os_fopen` converts with `utf8_to_wchar` (MultiByteToWideChar,
/// flags 0), which substitutes U+FFFD for invalid UTF-8 and then attempts
/// the substituted path.
pub fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

unsafe extern "system" {
    fn CloseHandle(object: *mut core::ffi::c_void) -> i32;
}

/// `fclose` result: closes `file` once, with no retry, and reports a failed
/// `CloseHandle` (a plain `File` drop discards it).
pub fn close_file(file: std::fs::File) -> std::io::Result<()> {
    use std::os::windows::io::IntoRawHandle;
    let handle = file.into_raw_handle();
    // SAFETY: `handle` was just released by `into_raw_handle`, so this is its only owner.
    if unsafe { CloseHandle(handle) } != 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}
