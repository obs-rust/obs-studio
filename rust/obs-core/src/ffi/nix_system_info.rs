//! C ABI for `libobs/obs-nix-system-info.c.h` and
//! `libobs/obs-nix-log-rules.c.h`, the system info parsing and log rules of
//! `obs-nix.c.h`'s startup log, declared in `libobs/obs-nix-system-info.h`
//! without `EXPORT`.
//!
//! `obs-nix.c.h` (still C) reads each line with `getline()`, reads the
//! `sysinfo()` and `sysctl()` results, passes them in and acts on what
//! comes back. A line or value is a NUL-terminated C string, read up to its
//! NUL as the C's `strncmp()`, `strchr()`, `strrchr()` and `strcspn()` do;
//! a returned position is a pointer into that same string, so the C keeps
//! its own buffer. These shims keep no state, take no lock, write nothing
//! but `*len` and `*key` and call nothing in libobs. All sixteen stay hidden
//! (`rust-hidden.d`).
//!
//! The distribution, Flatpak and desktop log rules of
//! `libobs/obs-nix-log-rules.c.h` are here too: `obs-nix-os-release.h` and
//! `obs-nix.c.h` read the lines, split the extension list and read the
//! environment, and pass the strings and flags in. The two enums cross as
//! `c_int`, with the values `obs-nix-system-info.h` spells out.
//!
//! One intentional difference from C (#3473): a Flatpak extension entry
//! with an empty name gives NULL, so it logs no line, where the C gives the
//! commit hash or `"(null)"`.

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

/// `enum obs_nix_os_release_key`'s values.
fn os_release_key(key: info::OsReleaseKey) -> c_int {
    match key {
        info::OsReleaseKey::Name => 0,
        info::OsReleaseKey::VersionId => 1,
        info::OsReleaseKey::BuildId => 2,
    }
}

/// `const char *obs_nix_system_info_os_release_field(const char *line, enum
/// obs_nix_os_release_key *key)`: a pointer into `line` after its key, with
/// the key in `*key`, or NULL with `*key` untouched.
///
/// # Safety
///
/// `line` must point to a NUL-terminated string, and `key` to a writable
/// `enum obs_nix_os_release_key`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_system_info_os_release_field(
    line: *const c_char,
    key: *mut c_int,
) -> *const c_char {
    // SAFETY: forwarded to the caller.
    match info::os_release_field(unsafe { bytes(line) }) {
        Some((which, start)) => {
            // SAFETY: the caller guarantees `key` is writable.
            unsafe { key.write(os_release_key(which)) };
            // SAFETY: `start` is at most the string's length, so the pointer
            // stays within it.
            unsafe { line.add(start) }
        }
        None => ptr::null(),
    }
}

/// `const char *obs_nix_system_info_os_release_value(const char *start,
/// size_t *len)`: a pointer into `start`, one past an opening quote that is
/// cut, with the value's length in `*len`.
///
/// # Safety
///
/// `start` must point to a NUL-terminated string, and `len` to a writable
/// `size_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_system_info_os_release_value(
    start: *const c_char,
    len: *mut usize,
) -> *const c_char {
    // SAFETY: forwarded to the caller.
    let range = info::os_release_value(unsafe { bytes(start) });
    // SAFETY: the caller guarantees `len` is writable.
    unsafe { len.write(range.len()) };
    // SAFETY: `range.start` is at most the string's length, so the pointer
    // stays within it.
    unsafe { start.add(range.start) }
}

/// `const char *obs_nix_system_info_version_fallback(bool has_build_id,
/// const char *distro)`: NULL for the `BUILD_ID`, or a static `"rolling"` or
/// `"Unknown"`. `distro` is read only when `has_build_id` is false.
///
/// # Safety
///
/// Without a `BUILD_ID`, `distro` must point to a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_system_info_version_fallback(
    has_build_id: bool,
    distro: *const c_char,
) -> *const c_char {
    // SAFETY: forwarded to the caller; read only without a BUILD_ID.
    match info::version_fallback(has_build_id, || unsafe { bytes(distro) }) {
        info::VersionFallback::BuildId => ptr::null(),
        info::VersionFallback::Rolling => c"rolling".as_ptr(),
        info::VersionFallback::Unknown => c"Unknown".as_ptr(),
    }
}

/// `const char *obs_nix_system_info_extension_name(const char *ext, size_t
/// *len)`: a pointer to the name, `ext` itself, with its length in `*len`,
/// or NULL with `*len` untouched when the name is empty (the entry logs no
/// line).
///
/// Intentional difference (#3473): the C gives the commit hash for an empty
/// name, or `"(null)"` when every piece is empty.
///
/// # Safety
///
/// `ext` must point to a NUL-terminated string, and `len` to a writable
/// `size_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_system_info_extension_name(
    ext: *const c_char,
    len: *mut usize,
) -> *const c_char {
    // SAFETY: forwarded to the caller.
    match info::extension_name(unsafe { bytes(ext) }) {
        Some(range) => {
            // SAFETY: the caller guarantees `len` is writable.
            unsafe { len.write(range.len()) };
            // SAFETY: `range.start` is at most the string's length, so the
            // pointer stays within it.
            unsafe { ext.add(range.start) }
        }
        None => ptr::null(),
    }
}

/// `enum obs_nix_desktop_form obs_nix_system_info_desktop_form(bool
/// has_current, bool has_session)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_system_info_desktop_form(has_current: bool, has_session: bool) -> c_int {
    match info::desktop_form(has_current, has_session) {
        info::DesktopForm::None => 0,
        info::DesktopForm::One => 1,
        info::DesktopForm::Both => 2,
    }
}
