//! C ABI for `libobs/obs-module-locale.c.h`.
//!
//! `obs_module_load_locale()` in `obs-module.c` calls the three functions
//! inside libobs, for every module's locale text: the default locale's file
//! name, whether the locale is the default one, and the locale's file name.
//! They are declared in `obs-internal.h` without `EXPORT`, so the shims stay
//! hidden (`libobs/cmake/rust-hidden.txt`). The names they return are
//! `bmalloc`'d, as the C's `dstr` arrays are, and the caller `bfree`s them.
//! A NULL locale is empty, as the C's `dstr_cat` and `astrcmpi` take it.

use core::ffi::c_char;

use super::module_paths::{bstrdup, bytes};
use crate::module_locale;

/// Same contract as C `obs_module_locale_default_file`.
///
/// # Safety
///
/// `default_locale` is null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_locale_default_file(
    default_locale: *const c_char,
) -> *mut c_char {
    // SAFETY: the caller guarantees a NUL-terminated string.
    let default_locale = unsafe { bytes(default_locale) };
    bstrdup(&module_locale::default_file(default_locale))
}

/// Same contract as C `obs_module_locale_is_default`.
///
/// # Safety
///
/// `locale` and `default_locale` are null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_locale_is_default(
    locale: *const c_char,
    default_locale: *const c_char,
) -> bool {
    // SAFETY: the caller guarantees NUL-terminated strings.
    let (locale, default_locale) = unsafe { (bytes(locale), bytes(default_locale)) };
    module_locale::is_default(locale, default_locale)
}

/// Same contract as C `obs_module_locale_file`.
///
/// # Safety
///
/// `locale` is null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_locale_file(locale: *const c_char) -> *mut c_char {
    // SAFETY: the caller guarantees a NUL-terminated string.
    let locale = unsafe { bytes(locale) };
    bstrdup(&module_locale::locale_file(locale))
}
