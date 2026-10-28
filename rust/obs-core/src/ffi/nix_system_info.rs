//! C ABI for `libobs/obs-nix-system-info.c.h`, the system info parsing of
//! `obs-nix.c.h`'s startup log, declared in `libobs/obs-nix-system-info.h`
//! without `EXPORT`.
//!
//! `obs-nix.c.h` (still C) reads each line with `getline()`, reads the
//! `sysinfo()` and `sysctl()` results, passes them in and acts on what
//! comes back. A line or value is a NUL-terminated C string, read up to its
//! NUL as the C's `strncmp()`, `strchr()`, `strrchr()` and `strcspn()` do;
//! a returned position is a pointer into that same string, so the C keeps
//! its own buffer. These shims keep no state, take no lock, write nothing
//! but `*len` and call nothing in libobs. All eleven stay hidden
//! (`rust-hidden.txt`).
//!
//! No intentional differences from C.

use core::ffi::{CStr, c_char, c_int};
use core::ptr;

use crate::nix_system_info as info;

/// The bytes of a C string, before its NUL.
///
/// # Safety
///
/// `s` must point to a NUL-terminated string that outlives the result.
unsafe fn bytes<'a>(s: *const c_char) -> &'a [u8] {
    // SAFETY: forwarded to the caller.
    unsafe { CStr::from_ptr(s) }.to_bytes()
}

/// `bool obs_nix_system_info_is_name(const char *line)`.
///
/// # Safety
///
/// `line` must point to a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_system_info_is_name(line: *const c_char) -> bool {
    // SAFETY: forwarded to the caller.
    info::is_name(unsafe { bytes(line) })
}

/// `bool obs_nix_system_info_is_physical_id(const char *line)`.
///
/// # Safety
///
/// `line` must point to a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_system_info_is_physical_id(line: *const c_char) -> bool {
    // SAFETY: forwarded to the caller.
    info::is_physical_id(unsafe { bytes(line) })
}

/// `bool obs_nix_system_info_is_speed(const char *line)`.
///
/// # Safety
///
/// `line` must point to a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_system_info_is_speed(line: *const c_char) -> bool {
    // SAFETY: forwarded to the caller.
    info::is_speed(unsafe { bytes(line) })
}

/// `const char *obs_nix_system_info_value(const char *line)`: a pointer
/// into `line`, or NULL.
///
/// # Safety
///
/// `line` must point to a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_system_info_value(line: *const c_char) -> *const c_char {
    // SAFETY: forwarded to the caller.
    match info::value(unsafe { bytes(line) }) {
        // SAFETY: `start` is at most the string's length, so the pointer
        // stays within it.
        Some(start) => unsafe { line.add(start) },
        None => ptr::null(),
    }
}

/// `int obs_nix_system_info_physical_id(const char *value)`.
///
/// # Safety
///
/// `value` must point to a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_system_info_physical_id(value: *const c_char) -> c_int {
    // SAFETY: forwarded to the caller.
    info::physical_id(unsafe { bytes(value) })
}

/// `bool obs_nix_system_info_prints(const char *line, int physical_id, int
/// last_physical_id)`.
///
/// # Safety
///
/// `line` must point to a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_system_info_prints(
    line: *const c_char,
    physical_id: c_int,
    last_physical_id: c_int,
) -> bool {
    // SAFETY: forwarded to the caller.
    info::prints(unsafe { bytes(line) }, physical_id, last_physical_id)
}

/// `bool obs_nix_system_info_is_dmesg_cpu(const char *line)`.
///
/// # Safety
///
/// `line` must point to a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_system_info_is_dmesg_cpu(line: *const c_char) -> bool {
    // SAFETY: forwarded to the caller.
    info::is_dmesg_cpu(unsafe { bytes(line) })
}

/// `const char *obs_nix_system_info_dmesg_speed(const char *line, size_t
/// *len)`: a pointer into `line` with the speed's length in `*len`, or NULL
/// with `*len` untouched.
///
/// # Safety
///
/// `line` must point to a NUL-terminated string, and `len` to a writable
/// `size_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_system_info_dmesg_speed(
    line: *const c_char,
    len: *mut usize,
) -> *const c_char {
    // SAFETY: forwarded to the caller.
    match info::dmesg_speed(unsafe { bytes(line) }) {
        Some(range) => {
            // SAFETY: the caller guarantees `len` is writable.
            unsafe { len.write(range.len()) };
            // SAFETY: `range.start` is at most the string's length, so the
            // pointer stays within it.
            unsafe { line.add(range.start) }
        }
        None => ptr::null(),
    }
}

/// `uint64_t obs_nix_system_info_total_mb(uint64_t totalram, uint32_t
/// mem_unit)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_system_info_total_mb(totalram: u64, mem_unit: u32) -> u64 {
    info::total_mb(totalram, mem_unit)
}

/// `uint64_t obs_nix_system_info_free_mb(uint64_t freeram, uint64_t
/// bufferram, uint32_t mem_unit)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_system_info_free_mb(freeram: u64, bufferram: u64, mem_unit: u32) -> u64 {
    info::free_mb(freeram, bufferram, mem_unit)
}

/// `int64_t obs_nix_system_info_physmem_mb(int64_t mem)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_system_info_physmem_mb(mem: i64) -> i64 {
    info::physmem_mb(mem)
}
