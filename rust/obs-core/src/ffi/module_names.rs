//! C ABI for `libobs/obs-module-names.c.h`.
//!
//! `obs-module.c` calls the six functions inside libobs: for the file name
//! and the name of every module it opens or records as disabled, for every
//! `obs_module_get_config_path()`, for the manifest of every module, for
//! the safe and disabled module lists of every module it loads, and for
//! every core module. They are declared in `obs-internal.h` without
//! `EXPORT`, so the shims stay hidden (`libobs/cmake/rust-hidden.txt`). The
//! strings they return are `bmalloc`'d, as the C's `dstr` arrays are, and
//! the caller `bfree`s them; `obs_module_names_file_name` returns a pointer
//! into its argument.
//!
//! The file system is libobs's own `os_file_exists()` (obs-util's port),
//! and `ext` is the caller's `get_module_extension()`. The callback of
//! `obs_module_names_find_core` gets a `struct obs_module_info2` whose
//! strings are valid for the call, as the C's are; `module_type` is passed
//! through as given.
//!
//! Intentional differences: a file name shorter than `ext` has no name
//! (NULL; the C's length underflows and the process crashes). For
//! arguments `obs-module.c` never passes: a NULL path has a NULL file name,
//! a NULL `ext` is empty, a NULL name or list entry is on no list, and a
//! NULL callback finds no core module (the C dereferences or calls each).

use core::ffi::{c_char, c_int, c_void};
use core::ptr;

use super::module_paths::{
    LibobsFs, bstrdup, bytes, c_path, obs_find_module_callback2_t, obs_module_info2,
};
use crate::module_names::{self, CoreModule};

/// Same contract as C `obs_module_names_file_name`: a pointer into `path`.
///
/// # Safety
///
/// `path` is null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_names_file_name(path: *const c_char) -> *const c_char {
    // SAFETY: the caller guarantees a NUL-terminated string.
    let Some(all) = (unsafe { bytes(path) }) else {
        return ptr::null();
    };
    let file = module_names::file_name(all);
    // SAFETY: `file` is the end of `all`, so the offset is inside `path`.
    unsafe { path.add(all.len().saturating_sub(file.len())) }
}

/// Same contract as C `obs_module_names_module_name`.
///
/// # Safety
///
/// `file` and `ext` are null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_names_module_name(
    file: *const c_char,
    ext: *const c_char,
) -> *mut c_char {
    // SAFETY: the caller guarantees NUL-terminated strings.
    let (file, ext) = unsafe { (bytes(file), bytes(ext)) };
    module_names::module_name(file, ext.unwrap_or_default())
        .map_or(ptr::null_mut(), |n| bstrdup(&n))
}

/// Same contract as C `obs_module_names_config_path`.
///
/// # Safety
///
/// `config_path`, `module_name` and `file` are null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_names_config_path(
    config_path: *const c_char,
    module_name: *const c_char,
    file: *const c_char,
) -> *mut c_char {
    // SAFETY: the caller guarantees NUL-terminated strings.
    let (config, name, file) = unsafe { (bytes(config_path), bytes(module_name), bytes(file)) };
    bstrdup(&module_names::config_path(config, name, file))
}

/// Same contract as C `obs_module_names_manifest_path`.
///
/// # Safety
///
/// `data_path` is null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_names_manifest_path(data_path: *const c_char) -> *mut c_char {
    // SAFETY: the caller guarantees a NUL-terminated string.
    let data = unsafe { bytes(data_path) };
    bstrdup(&module_names::manifest_path(data))
}

/// Same contract as C `obs_module_names_listed`.
///
/// # Safety
///
/// `names` is null or holds `count` pointers, each null or NUL-terminated;
/// `name` is null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_names_listed(
    names: *const *mut c_char,
    count: usize,
    name: *const c_char,
) -> bool {
    // SAFETY: the caller guarantees a NUL-terminated string.
    let Some(name) = (unsafe { bytes(name) }) else {
        return false;
    };
    if names.is_null() {
        return false;
    }
    // SAFETY: `names` holds `count` pointers.
    let names = unsafe { core::slice::from_raw_parts(names, count) };
    // SAFETY: each is null or NUL-terminated.
    let names = names
        .iter()
        .filter_map(|&n| unsafe { bytes(n.cast_const()) });
    module_names::listed(names, name)
}

/// Same contract as C `obs_module_names_find_core`.
///
/// # Safety
///
/// `binary`, `name`, `data` and `ext` are null or NUL-terminated.
/// `callback` is null or safe to call with `param` during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_names_find_core(
    binary: *const c_char,
    name: *const c_char,
    data: *const c_char,
    module_type: c_int,
    ext: *const c_char,
    callback: obs_find_module_callback2_t,
    param: *mut c_void,
) -> bool {
    let Some(callback) = callback else {
        return false;
    };
    // SAFETY: the caller guarantees NUL-terminated strings.
    let (binary, name, data, ext) =
        unsafe { (bytes(binary), bytes(name), bytes(data), bytes(ext)) };
    let mut report = |found: &CoreModule<'_>| {
        let bin_path = c_path(found.bin_path);
        let data_path = found.data_path.and_then(c_path);
        let name = found.name.and_then(c_path);
        let as_ptr = |s: &Option<Vec<u8>>| s.as_ref().map_or(ptr::null(), |s| s.as_ptr().cast());
        let info = obs_module_info2 {
            bin_path: as_ptr(&bin_path),
            data_path: as_ptr(&data_path),
            name: as_ptr(&name),
            module_type,
        };
        // SAFETY: the caller's callback and param, with an info whose
        // strings live until it returns.
        unsafe { callback(param, &raw const info) };
    };
    module_names::find_core(
        &LibobsFs,
        binary,
        name,
        data,
        ext.unwrap_or_default(),
        &mut report,
    )
}
