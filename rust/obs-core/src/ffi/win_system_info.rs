//! C ABI for `libobs/obs-windows-system-info.c.h`, the rules of
//! `obs-windows.c.h`'s system info log, declared in
//! `libobs/obs-windows-system-info.h` without `EXPORT`.
//!
//! `obs-windows.c.h` (still C) makes every Windows call and registry read,
//! passes the values in and logs what comes back. The shims take and return
//! integers and `'static` C strings, write only `*label` and `*text` (and
//! those only when a line is logged), allocate nothing and keep no state.
//! All ten stay hidden (`rust-hidden.d`).
//!
//! No intentional differences from C. `gaming_value_line` and
//! `gaming_fallback_line` return false for a null `label` or `text`, which
//! the C never passes (the C would write through it).

use std::ffi::c_char;

use crate::win_system_info as info;

/// `uint32_t obs_windows_system_info_version(uint32_t major, uint32_t
/// minor)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_windows_system_info_version(major: u32, minor: u32) -> u32 {
    info::version(major, minor)
}

/// `uint32_t obs_windows_system_info_mb(uint64_t bytes)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_windows_system_info_mb(bytes: u64) -> u32 {
    info::mb(bytes)
}

/// `bool obs_windows_system_info_gaming_logged(uint32_t win_ver)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_windows_system_info_gaming_logged(win_ver: u32) -> bool {
    info::gaming_logged(win_ver)
}

/// `bool obs_windows_system_info_game_mode_retry(int32_t status)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_windows_system_info_game_mode_retry(status: i32) -> bool {
    info::game_mode_retry(status)
}

/// `bool obs_windows_system_info_gaming_found(int32_t status)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_windows_system_info_gaming_found(status: i32) -> bool {
    info::gaming_found(status)
}

/// Writes a line through the two out-pointers, or nothing when either is
/// null or there is no line.
///
/// # Safety
///
/// `label` and `text` are null or valid for a write of one pointer each.
unsafe fn write_line(
    line: Option<info::Line>,
    label: *mut *const c_char,
    text: *mut *const c_char,
) -> bool {
    let Some(line) = line else {
        return false;
    };
    if label.is_null() || text.is_null() {
        return false;
    }
    // SAFETY: both are non-null, and the caller gives pointers valid for a
    // write of one pointer each. The strings are 'static.
    unsafe {
        label.write(line.label.as_ptr());
        text.write(line.text.as_ptr());
    }
    true
}

/// `bool obs_windows_system_info_gaming_value_line(int feature, uint32_t
/// value, const char **label, const char **text)`.
///
/// # Safety
///
/// `label` and `text` are null or valid for a write of one pointer each.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_windows_system_info_gaming_value_line(
    feature: i32,
    value: u32,
    label: *mut *const c_char,
    text: *mut *const c_char,
) -> bool {
    let line = info::Feature::from_index(feature).map(|feature| info::value_line(feature, value));
    // SAFETY: forwarded from this function's contract.
    unsafe { write_line(line, label, text) }
}

/// `bool obs_windows_system_info_gaming_fallback(int feature)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_windows_system_info_gaming_fallback(feature: i32) -> bool {
    info::Feature::from_index(feature).is_some_and(info::has_fallback)
}

/// `bool obs_windows_system_info_gaming_fallback_line(int feature, uint32_t
/// win_build, const char **label, const char **text)`.
///
/// # Safety
///
/// `label` and `text` are null or valid for a write of one pointer each.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_windows_system_info_gaming_fallback_line(
    feature: i32,
    win_build: u32,
    label: *mut *const c_char,
    text: *mut *const c_char,
) -> bool {
    let line = info::Feature::from_index(feature)
        .and_then(|feature| info::fallback_line(feature, win_build));
    // SAFETY: forwarded from this function's contract.
    unsafe { write_line(line, label, text) }
}

/// `const char *obs_windows_system_info_product_state(int state)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_windows_system_info_product_state(state: i32) -> *const c_char {
    info::product_state(state).as_ptr()
}

/// `const char *obs_windows_system_info_product_type(int type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_windows_system_info_product_type(kind: i32) -> *const c_char {
    info::product_type(kind).as_ptr()
}
