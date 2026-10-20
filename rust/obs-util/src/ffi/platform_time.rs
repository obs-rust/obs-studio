//! C ABI shim for `os_nstime_to_timespec` from `libobs/util/platform-time.c`
//! (`libobs/util/platform.h`), over the safe core in
//! [`crate::platform_time`].
//!
//! The base is recorded on the first call, a NULL `storage` included, as the
//! C's `pthread_once` does. The C returns NULL when `timespec_get` fails;
//! the Rust wall clock (`SystemTime::now`) cannot fail, so only a NULL
//! `storage` returns NULL.

use core::ffi::c_long;

use crate::platform_time;

/// `struct timespec` from `<time.h>`. `time_t` is the host's; `tv_nsec` is a
/// C `long`, 32-bit on Windows.
#[repr(C)]
#[allow(non_camel_case_types)] // the C name
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct timespec {
    pub tv_sec: libc::time_t,
    pub tv_nsec: c_long,
}

/// # Safety
/// `storage` is null or points to a writable `struct timespec`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_nstime_to_timespec(
    timestamp: u64,
    storage: *mut timespec,
) -> *mut timespec {
    let base = platform_time::base();
    if storage.is_null() {
        return core::ptr::null_mut();
    }
    let ts = platform_time::nstime_to_timespec(base, timestamp);
    // The C's `(long)` and `(time_t)` casts.
    let out = timespec {
        tv_sec: ts.sec as libc::time_t,
        tv_nsec: ts.nsec as c_long,
    };
    // SAFETY: `storage` is non-null and writable (the caller's contract).
    unsafe { storage.write(out) };
    storage
}
