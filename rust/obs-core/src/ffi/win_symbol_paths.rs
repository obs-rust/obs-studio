//! C ABI for `libobs/obs-windows-symbol-paths.c.h`, the string rules of
//! `obs-windows.c.h`'s `reset_win32_symbol_paths()`, declared in
//! `libobs/obs-windows-symbol-paths.h` without `EXPORT`.
//!
//! `obs-windows.c.h` (still C) loads DbgHelp, makes each directory absolute,
//! keeps the list, converts the search path and hands it to DbgHelp. The
//! shims fill the caller's `struct dstr` with obs-util's `dstr_copy()` and
//! `dstr_ncat()`, one call where the C makes one (so the array grows as the
//! C's does), and keep no state. All three
//! stay hidden (`rust-hidden.d`).
//!
//! One intentional difference: `obs_windows_symbol_paths_dir()` returns
//! false for a NULL or empty `bin_path`, after `dstr_copy()` has freed `dir`
//! as the C's does; the C then calls `strrchr()` on the NULL array. A null
//! `dir`, `paths` (with `num` above 0) or `out`, which the C never passes,
//! does nothing (false for the two that return).

use std::ffi::c_char;

use obs_util::ffi::dstr::{dstr, dstr_copy, dstr_ncat};

use crate::ffi::module_paths::{bytes, c_path};
use crate::win_symbol_paths as paths;

/// The first `num` entries of a C array of C strings, `None` for NULL.
///
/// # Safety
///
/// `array` is valid for `num` reads, each entry null or NUL-terminated, and
/// all of them outlive `'a`.
unsafe fn entries<'a>(
    array: *const *mut c_char,
    num: usize,
) -> impl Iterator<Item = Option<&'a [u8]>> {
    (0..num).map(move |i| {
        // SAFETY: the caller guarantees `num` readable entries, each null or
        // NUL-terminated.
        unsafe { bytes((*array.add(i)).cast_const()) }
    })
}

/// `bool obs_windows_symbol_paths_dir(const char *bin_path, struct dstr
/// *dir)`.
///
/// # Safety
///
/// `bin_path` is null or NUL-terminated; `dir` is null or a valid
/// `struct dstr`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_windows_symbol_paths_dir(
    bin_path: *const c_char,
    dir: *mut dstr,
) -> bool {
    if dir.is_null() {
        return false;
    }
    // SAFETY: `bin_path` is null or NUL-terminated.
    let Some(path) = unsafe { bytes(bin_path) }.filter(|path| !path.is_empty()) else {
        // SAFETY: `dir` is a valid dstr; a NULL or empty source frees it,
        // as the C's dstr_copy() does.
        unsafe { dstr_copy(dir, bin_path) };
        return false;
    };
    let cut = paths::cut(path);
    let Some(copy) = c_path(&cut.copy) else {
        return false;
    };
    // SAFETY: `dir` is a valid dstr and `copy` is NUL-terminated.
    unsafe { dstr_copy(dir, copy.as_ptr().cast()) };
    match cut.end {
        Some(end) => {
            // SAFETY: dstr_copy() left `cut.copy.len() + 1` bytes in the
            // array, and `end` is below `cut.copy.len()`.
            unsafe { *(*dir).array.add(end) = 0 };
            true
        }
        None => false,
    }
}

/// `bool obs_windows_symbol_paths_listed(char *const *paths, size_t num,
/// const char *abspath)`.
///
/// # Safety
///
/// `paths` is valid for `num` reads of entries that are null or
/// NUL-terminated (or null with `num` 0), and `abspath` is null or
/// NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_windows_symbol_paths_listed(
    paths: *const *mut c_char,
    num: usize,
    abspath: *const c_char,
) -> bool {
    if paths.is_null() {
        return false;
    }
    // SAFETY: forwarded to the caller.
    unsafe { paths::listed(entries(paths, num), bytes(abspath)) }
}

/// `void obs_windows_symbol_paths_join(char *const *paths, size_t num,
/// struct dstr *out)`.
///
/// # Safety
///
/// `paths` is valid for `num` reads of entries that are null or
/// NUL-terminated (or null with `num` 0), and `out` is null or a valid
/// `struct dstr`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_windows_symbol_paths_join(
    paths: *const *mut c_char,
    num: usize,
    out: *mut dstr,
) {
    if paths.is_null() || out.is_null() {
        return;
    }
    // SAFETY: forwarded to the caller.
    for piece in paths::join_pieces(unsafe { entries(paths, num) }) {
        // SAFETY: `out` is a valid dstr and `piece` holds `len` bytes.
        unsafe { dstr_ncat(out, piece.as_ptr().cast(), piece.len()) };
    }
}
