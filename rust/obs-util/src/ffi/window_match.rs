//! C ABI for `libobs/util/windows/window-match.c.h`.
//!
//! The window helpers (`window-helpers.c`) call these inside libobs on
//! Windows. The C file has no `EXPORT`, so the symbols stay hidden
//! (`rust-hidden.d`). The five functions keep the C signatures: C
//! strings, `bool` and `int`. A null class, which C's `strcmp()` would
//! dereference in `ms_window_match_is_uwp_class`, is not a UWP class here;
//! `window-helpers.c` returns before it asks with one.
//!
//! Only Windows calls them. Elsewhere the shims are built but unused, and
//! hidden all the same.

use std::ffi::{CStr, c_char, c_int};

use crate::window_match::{
    is_generic_class, is_internal_exe, is_untitled_explorer, is_uwp_class, rating,
};

/// The bytes of a C string, or `None` for NULL.
///
/// # Safety
///
/// `p` is null or a NUL-terminated string that outlives `'a`.
unsafe fn opt_cstr<'a>(p: *const c_char) -> Option<&'a [u8]> {
    // SAFETY: null or NUL-terminated, as the caller promises.
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_bytes())
}

/// Same contract as C `ms_window_match_is_internal_exe`.
///
/// # Safety
///
/// `exe` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ms_window_match_is_internal_exe(exe: *const c_char) -> bool {
    // SAFETY: forwarded to the caller.
    is_internal_exe(unsafe { opt_cstr(exe) })
}

/// Same contract as C `ms_window_match_is_untitled_explorer`.
///
/// # Safety
///
/// `exe` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ms_window_match_is_untitled_explorer(
    exe: *const c_char,
    title_empty: bool,
) -> bool {
    // SAFETY: forwarded to the caller.
    is_untitled_explorer(unsafe { opt_cstr(exe) }, title_empty)
}

/// Same contract as C `ms_window_match_is_generic_class`.
///
/// # Safety
///
/// `window_class` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ms_window_match_is_generic_class(window_class: *const c_char) -> bool {
    // SAFETY: forwarded to the caller.
    is_generic_class(unsafe { opt_cstr(window_class) })
}

/// Same contract as C `ms_window_match_is_uwp_class`; NULL is not a UWP
/// class.
///
/// # Safety
///
/// `window_class` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ms_window_match_is_uwp_class(window_class: *const c_char) -> bool {
    // SAFETY: forwarded to the caller.
    unsafe { opt_cstr(window_class) }.is_some_and(is_uwp_class)
}

/// Same contract as C `ms_window_match_rating`.
#[unsafe(no_mangle)]
pub extern "C" fn ms_window_match_rating(
    priority: c_int,
    uwp_window: bool,
    generic_class: bool,
    class_matches: bool,
    exe_matches: bool,
    title_cmp: c_int,
) -> c_int {
    rating(
        priority,
        uwp_window,
        generic_class,
        class_matches,
        exe_matches,
        title_cmp,
    )
}
