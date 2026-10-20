//! Unix (Linux, macOS) side of the [platform boundary](super).

#[cfg(target_os = "macos")]
pub mod cocoa;
pub mod crt;
pub mod event;
pub mod file;
pub mod nix;
#[cfg(not(target_os = "macos"))]
pub mod nix_inhibit;
pub mod pipe;

pub use crate::ffi::native_sem::Sem;
pub use event::Event;
pub use pipe::Pipe;

use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

/// Byte paths are passed to `fopen(3)` as-is.
pub fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

/// `fclose(3)` result: closes `file` once, with no retry, and reports a
/// failed `close(2)` (a plain `File` drop discards it).
#[allow(unsafe_code)] // one close(2) call, no safe std equivalent that reports the error
pub fn close_file(file: std::fs::File) -> std::io::Result<()> {
    use std::os::fd::IntoRawFd;
    let fd = file.into_raw_fd();
    // SAFETY: `fd` was just released by `into_raw_fd`, so this is its only owner.
    if unsafe { libc::close(fd) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}
