//! C ABI for `libobs/util/windows/window-match.c.h`.
//!
//! The window helpers (`window-helpers.c`) call these inside libobs on
//! Windows. The C file has no `EXPORT`, so the symbols stay hidden
//! (`rust-hidden.d`). The eight functions keep the C signatures: C
//! strings, buffers, `bool`, `int` and `size_t`. A null class, which C's
//! `strcmp()` would dereference in `ms_window_match_is_uwp_class`, is not a
//! UWP class here; `window-helpers.c` returns before it asks with one.
//!
//! Only Windows calls them. Elsewhere the shims are built but unused, and
//! hidden all the same.

use std::ffi::{CStr, c_char, c_int};
use std::slice;

use crate::window_match::{
    decode, encode_into, encoded_len, is_generic_class, is_internal_exe, is_untitled_explorer,
    is_uwp_class, rating, split,
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

/// Same contract as C `ms_window_match_encode`: NULL `s` is "", and a NULL
/// `out` is only measured.
///
/// # Safety
///
/// `s` is null or a NUL-terminated string; `out` is null or `out_size`
/// writable bytes that do not overlap `s`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ms_window_match_encode(
    s: *const c_char,
    out: *mut c_char,
    out_size: usize,
) -> usize {
    // SAFETY: forwarded to the caller.
    let s = unsafe { opt_cstr(s) }.unwrap_or_default();
    if out.is_null() {
        return encoded_len(s);
    }
    // SAFETY: `out` is `out_size` writable bytes apart from `s`, as the
    // caller promises.
    let out = unsafe { slice::from_raw_parts_mut(out.cast::<u8>(), out_size) };
    encode_into(s, out)
}

/// Same contract as C `ms_window_match_decode`: NULL is 0.
///
/// # Safety
///
/// `s` is null or a writable NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ms_window_match_decode(s: *mut c_char) -> usize {
    if s.is_null() {
        return 0;
    }
    // SAFETY: a NUL-terminated string, as the caller promises.
    let len = unsafe { CStr::from_ptr(s) }.count_bytes();
    // SAFETY: its `len` bytes before the NUL are writable.
    decode(unsafe { slice::from_raw_parts_mut(s.cast::<u8>(), len) })
}

/// Same contract as C `ms_window_match_split`: on false, all six outputs
/// are 0.
///
/// # Safety
///
/// `str` is null or a NUL-terminated string; `start` and `len` each point
/// to three writable `size_t`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ms_window_match_split(
    str: *const c_char,
    start: *mut usize,
    len: *mut usize,
) -> bool {
    // SAFETY: forwarded to the caller.
    let fields = unsafe { opt_cstr(str) }.and_then(split);
    // SAFETY: three writable `size_t`s each, as the caller promises.
    let (start, len) = unsafe {
        (
            slice::from_raw_parts_mut(start, 3),
            slice::from_raw_parts_mut(len, 3),
        )
    };
    let ranges = fields.clone().unwrap_or_default();
    for ((start, len), range) in start.iter_mut().zip(len.iter_mut()).zip(ranges) {
        *start = range.start;
        *len = range.len();
    }
    fields.is_some()
}
