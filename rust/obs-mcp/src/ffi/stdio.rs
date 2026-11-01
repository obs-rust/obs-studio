//! Keeps the protocol's stdout clean (#3289).
//!
//! libobs and the C runtime print to file descriptor 1 (the default libobs
//! log handler writes info lines with `fprintf(stdout, ...)`). An MCP stdio
//! server must put only JSON-RPC on stdout, so the host takes the original
//! stdout for the protocol and points descriptor 1 at stderr.

use std::fs::File;
use std::io;

/// Returns the process's original stdout for the protocol and redirects
/// descriptor 1 to stderr, so everything else that prints goes to stderr.
///
/// # Errors
///
/// Returns the OS error when a descriptor cannot be duplicated.
#[cfg(unix)]
pub fn claim_stdout() -> io::Result<File> {
    use std::os::fd::FromRawFd;

    // SAFETY: duplicating descriptor 1 has no memory-safety preconditions.
    let saved = unsafe { libc::dup(1) };
    if saved < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `saved` is a fresh descriptor that nothing else owns.
    let protocol = unsafe { File::from_raw_fd(saved) };
    // SAFETY: descriptors 1 and 2 are the process's standard streams.
    if unsafe { libc::dup2(2, 1) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(protocol)
}

#[cfg(windows)]
unsafe extern "C" {
    fn _dup(fd: i32) -> i32;
    fn _dup2(from: i32, to: i32) -> i32;
    fn _get_osfhandle(fd: i32) -> isize;
}

/// Returns the process's original stdout for the protocol and redirects the C
/// runtime's descriptor 1 to stderr, so everything else that prints goes to
/// stderr.
///
/// # Errors
///
/// Returns the OS error when a descriptor cannot be duplicated.
#[cfg(windows)]
pub fn claim_stdout() -> io::Result<File> {
    use std::os::windows::io::{FromRawHandle, RawHandle};

    // SAFETY: duplicating descriptor 1 has no memory-safety preconditions.
    let saved = unsafe { _dup(1) };
    if saved < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `saved` is a descriptor this function just created.
    let handle = unsafe { _get_osfhandle(saved) };
    if handle == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: descriptors 1 and 2 are the C runtime's standard streams.
    if unsafe { _dup2(2, 1) } < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `handle` is the open handle behind `saved`; the `File` takes
    // ownership of it and the descriptor is never used again.
    Ok(unsafe { File::from_raw_handle(handle as RawHandle) })
}
