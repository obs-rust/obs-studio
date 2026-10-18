//! The file helpers of `libobs/util/platform-file.c`, compiled unmodified as
//! `oracle_*` by `oracle/platform.c`. `os_stat` exists only on Windows
//! (elsewhere it is `#define os_stat stat`).

use core::ffi::{c_char, c_int, c_void};

/// `wchar_t`.
#[cfg(windows)]
pub type WChar = u16;
/// `wchar_t`.
#[cfg(not(windows))]
pub type WChar = i32;

unsafe extern "C" {
    pub fn oracle_os_wfopen(path: *const WChar, mode: *const c_char) -> *mut c_void;
    pub fn oracle_os_fopen(path: *const c_char, mode: *const c_char) -> *mut c_void;
    pub fn oracle_os_fgetsize(file: *mut c_void) -> i64;
    pub fn oracle_os_fseeki64(file: *mut c_void, offset: i64, origin: c_int) -> c_int;
    pub fn oracle_os_ftelli64(file: *mut c_void) -> i64;
    pub fn oracle_os_fread_mbs(file: *mut c_void, pstr: *mut *mut c_char) -> usize;
    pub fn oracle_os_fread_utf8(file: *mut c_void, pstr: *mut *mut c_char) -> usize;
    pub fn oracle_os_quick_read_mbs_file(path: *const c_char) -> *mut c_char;
    pub fn oracle_os_quick_read_utf8_file(path: *const c_char) -> *mut c_char;
    pub fn oracle_os_quick_write_mbs_file(
        path: *const c_char,
        str: *const c_char,
        len: usize,
    ) -> bool;
    pub fn oracle_os_quick_write_utf8_file(
        path: *const c_char,
        str: *const c_char,
        len: usize,
        marker: bool,
    ) -> bool;
    pub fn oracle_os_quick_write_utf8_file_safe(
        path: *const c_char,
        str: *const c_char,
        len: usize,
        marker: bool,
        temp_ext: *const c_char,
        backup_ext: *const c_char,
    ) -> bool;
    pub fn oracle_os_get_file_size(path: *const c_char) -> i64;
}

/// The UCRT's `struct _stat64i32` (`struct stat` in libobs's configuration).
#[cfg(windows)]
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

#[cfg(windows)]
unsafe extern "C" {
    pub fn oracle_os_stat(path: *const c_char, st: *mut Stat) -> c_int;
}
