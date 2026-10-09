//! C ABI shims for `libobs/util/text-lookup.h`.
//!
//! The shims only convert between C and Rust types and call the safe core in
//! [`crate::text_lookup`]. `lookup_t` is a `Box<TextLookup>` raw pointer.

use core::ffi::{CStr, c_char, c_void};
use std::path::Path;

use crate::text_lookup::TextLookup;

/// Converts a C path to a [`Path`]; `None` for NULL (and for non-UTF-8 paths
/// on platforms where paths must be Unicode).
///
/// # Safety
///
/// `path` must be NULL or a valid NUL-terminated string.
unsafe fn path_from_c<'a>(path: *const c_char) -> Option<&'a Path> {
    if path.is_null() {
        return None;
    }
    // SAFETY: non-null and NUL-terminated per the caller.
    let bytes = unsafe { CStr::from_ptr(path) }.to_bytes();

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Some(Path::new(std::ffi::OsStr::from_bytes(bytes)))
    }
    #[cfg(not(unix))]
    {
        core::str::from_utf8(bytes).ok().map(Path::new)
    }
}

/// Creates a lookup from the file at `path`; returns NULL on failure.
///
/// # Safety
///
/// `path` must be NULL or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn text_lookup_create(path: *const c_char) -> *mut c_void {
    let mut lookup = Box::new(TextLookup::new());
    // SAFETY: forwarded from the caller.
    let added = unsafe { path_from_c(path) }.is_some_and(|p| lookup.add_file(p));
    if added {
        Box::into_raw(lookup).cast::<c_void>()
    } else {
        core::ptr::null_mut()
    }
}

/// Adds the entries of the file at `path`. A NULL `lookup` returns false
/// (C would dereference it).
///
/// # Safety
///
/// `lookup` must be NULL or come from [`text_lookup_create`] and not be
/// destroyed; `path` must be NULL or a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn text_lookup_add(lookup: *mut c_void, path: *const c_char) -> bool {
    if lookup.is_null() {
        return false;
    }
    // SAFETY: non-null and created by `text_lookup_create` per the caller.
    let lookup = unsafe { &mut *lookup.cast::<TextLookup>() };
    // SAFETY: forwarded from the caller.
    unsafe { path_from_c(path) }.is_some_and(|p| lookup.add_file(p))
}

/// Frees a lookup. NULL is a no-op.
///
/// # Safety
///
/// `lookup` must be NULL or come from [`text_lookup_create`] and not have
/// been destroyed already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn text_lookup_destroy(lookup: *mut c_void) {
    if !lookup.is_null() {
        // SAFETY: created by `Box::into_raw` in `text_lookup_create`.
        drop(unsafe { Box::from_raw(lookup.cast::<TextLookup>()) });
    }
}

/// Looks up `lookup_val`; on success stores a pointer valid until the lookup
/// is destroyed (or the key is replaced by `text_lookup_add`) into `*out`.
///
/// # Safety
///
/// `lookup` must be NULL or a live lookup; `lookup_val` must be NULL or a
/// valid NUL-terminated string; `out` must be NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn text_lookup_getstr(
    lookup: *mut c_void,
    lookup_val: *const c_char,
    out: *mut *const c_char,
) -> bool {
    if lookup.is_null() || lookup_val.is_null() {
        return false;
    }
    // SAFETY: non-null and live per the caller.
    let lookup = unsafe { &*lookup.cast::<TextLookup>() };
    // SAFETY: non-null and NUL-terminated per the caller.
    let key = unsafe { CStr::from_ptr(lookup_val) };
    match lookup.get(key) {
        Some(value) => {
            if !out.is_null() {
                // SAFETY: `out` is writable per the caller.
                unsafe { *out = value.as_ptr() };
            }
            true
        }
        None => false,
    }
}
