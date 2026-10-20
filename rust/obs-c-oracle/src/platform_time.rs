//! `os_nstime_to_timespec` from `libobs/util/platform-time.c`, compiled
//! unmodified as `oracle_os_nstime_to_timespec` by `oracle/platform.c`, over
//! the oracle's own `oracle_os_gettime_ns`.
//!
//! `storage` is a `struct timespec`; the layout functions give its C layout.
//! `oracle_platform_time_set_base` runs the C's first-call initialization,
//! then replaces the base it recorded, so a differential test can give the C
//! and the Rust core the same base. The base is one static: callers must not
//! set it from two threads at once.

use core::ffi::{c_long, c_void};

unsafe extern "C" {
    pub fn oracle_os_nstime_to_timespec(timestamp: u64, storage: *mut c_void) -> *mut c_void;
    pub fn oracle_platform_time_set_base(sec: i64, nsec: c_long, timestamp: u64);
    pub fn oracle_timespec_size() -> usize;
    pub fn oracle_timespec_align() -> usize;
    pub fn oracle_timespec_offset_tv_sec() -> usize;
    pub fn oracle_timespec_offset_tv_nsec() -> usize;
}
