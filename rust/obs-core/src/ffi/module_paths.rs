//! C ABI for `libobs/obs-module-paths.c.h`.
//!
//! `obs-module.c` calls the three functions inside libobs: for every search
//! path of `obs_find_modules2()` and the one of `obs_load_plugins()`, for the
//! data path of each core module, and for every `obs_find_module_file()`.
//! They are declared in `obs-internal.h` without `EXPORT`, so the shims stay
//! hidden (`libobs/cmake/rust-hidden.txt`). The paths they return are
//! `bmalloc`'d, as the C's `dstr` arrays are, and the caller `bfree`s them.
//!
//! The file system is libobs's own `os_file_exists()` and `os_glob()`
//! (obs-util's ports). `ext` is the caller's `get_module_extension()` and
//! `add_extension` its `obs_host_add_module_extension()`, called with a
//! `bmalloc`'d `struct dstr` that it may grow (with `dstr_cat`) as the C's.
//! The callback gets a `struct obs_module_info2` whose strings are valid for
//! the call, as the C's are; `module_type` is passed through as given.
//!
//! Intentional differences, for arguments `obs-module.c` never passes: an
//! empty or NULL `bin` searches nothing (the C crashes in `strstr()`), a
//! NULL `callback` searches nothing and a NULL `add_extension` leaves the
//! path as it is (the C calls them).

use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr;

use obs_util::ffi::bmem::{bfree, bmalloc};
use obs_util::ffi::dstr::dstr;
#[cfg(unix)]
use obs_util::ffi::platform_nix::{os_file_exists, os_glob, os_glob_info, os_globfree};
#[cfg(windows)]
use obs_util::ffi::platform_windows::{os_file_exists, os_glob, os_glob_info, os_globfree};

use crate::module_paths::{self, GlobEntry, ModuleFs, ModuleInfo};

/// `struct obs_module_info2` (`obs.h`).
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct obs_module_info2 {
    pub bin_path: *const c_char,
    pub data_path: *const c_char,
    pub name: *const c_char,
    /// `enum obs_runtime_module_type`.
    pub module_type: c_int,
}

/// `obs_find_module_callback2_t` (`obs.h`).
#[allow(non_camel_case_types)]
pub type obs_find_module_callback2_t =
    Option<unsafe extern "C" fn(param: *mut c_void, info: *const obs_module_info2)>;

/// `obs_module_add_extension_t` (`obs-internal.h`).
#[allow(non_camel_case_types)]
pub type obs_module_add_extension_t = Option<unsafe extern "C" fn(bin_path: *mut dstr)>;

/// A NUL-terminated copy, for a C call.
pub(crate) fn c_path(path: &[u8]) -> Option<Vec<u8>> {
    if path.contains(&0) {
        return None;
    }
    let mut out = Vec::with_capacity(path.len().saturating_add(1));
    out.extend_from_slice(path);
    out.push(0);
    Some(out)
}

/// A `bmalloc`'d, NUL-terminated copy of bytes that have no NUL.
pub(crate) fn bstrdup(bytes: &[u8]) -> *mut c_char {
    let Some(size) = bytes.len().checked_add(1) else {
        return ptr::null_mut();
    };
    // SAFETY: `bmalloc` returns `size` writable bytes (it aborts on
    // failure), which hold the copy and its NUL.
    unsafe {
        let out = bmalloc(size).cast::<u8>();
        ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len());
        *out.add(bytes.len()) = 0;
        out.cast()
    }
}

/// The bytes of a C string, `None` for NULL.
///
/// # Safety
///
/// `s` is null or NUL-terminated, and outlives `'a`.
pub(crate) unsafe fn bytes<'a>(s: *const c_char) -> Option<&'a [u8]> {
    // SAFETY: the caller guarantees a NUL-terminated string.
    (!s.is_null()).then(|| unsafe { CStr::from_ptr(s) }.to_bytes())
}

/// libobs's file system: `os_file_exists()` and `os_glob()`.
pub(crate) struct LibobsFs;

impl ModuleFs for LibobsFs {
    fn file_exists(&self, path: &[u8]) -> bool {
        let Some(path) = c_path(path) else {
            return false;
        };
        // SAFETY: `path` is NUL-terminated.
        unsafe { os_file_exists(path.as_ptr().cast()) }
    }

    fn glob(&self, pattern: &[u8]) -> Option<Vec<GlobEntry>> {
        let pattern = c_path(pattern)?;
        let mut info: *mut os_glob_info = ptr::null_mut();
        // SAFETY: `pattern` is NUL-terminated and `info` is an out-pointer.
        if unsafe { os_glob(pattern.as_ptr().cast(), 0, &raw mut info) } != 0 || info.is_null() {
            return None;
        }
        // SAFETY: `os_glob` succeeded, so `info` holds `gl_pathc` entries
        // with NUL-terminated paths until `os_globfree`.
        let entries = unsafe {
            let count = (*info).gl_pathc;
            let pathv = (*info).gl_pathv;
            let mut entries = Vec::with_capacity(count);
            for i in 0..count {
                let entry = &*pathv.add(i);
                entries.push(GlobEntry {
                    path: bytes(entry.path).unwrap_or_default().to_vec(),
                    directory: entry.directory,
                });
            }
            os_globfree(info);
            entries
        };
        Some(entries)
    }
}

/// Calls the host's hook with `path` as a `bmalloc`'d `struct dstr`, and
/// reads back what it left.
///
/// # Safety
///
/// `hook` is safe to call with a `struct dstr` that `dstr_*` may grow.
unsafe fn call_add_extension(hook: unsafe extern "C" fn(*mut dstr), path: &mut Vec<u8>) {
    let len = path.len();
    let mut d = dstr {
        array: bstrdup(path),
        len,
        capacity: len.saturating_add(1),
    };
    // SAFETY: `d` is a valid `dstr` over a `bmalloc`'d array.
    unsafe { hook(&raw mut d) };
    path.clear();
    if !d.array.is_null() {
        // SAFETY: the hook leaves `d` a valid `dstr`: `len` bytes, then NUL.
        unsafe {
            path.extend_from_slice(core::slice::from_raw_parts(d.array.cast::<u8>(), d.len));
            bfree(d.array.cast());
        }
    }
}

/// Same contract as C `obs_module_paths_data_dir`.
///
/// # Safety
///
/// `module_name` and `data_dir` are null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_paths_data_dir(
    module_name: *const c_char,
    data_dir: *const c_char,
) -> *mut c_char {
    // SAFETY: the caller guarantees NUL-terminated strings.
    let (name, data) = unsafe { (bytes(module_name), bytes(data_dir)) };
    module_paths::data_dir(&LibobsFs, name, data).map_or(ptr::null_mut(), |p| bstrdup(&p))
}

/// Same contract as C `obs_module_paths_find_file`.
///
/// # Safety
///
/// `data_path` and `file` are null or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_paths_find_file(
    data_path: *const c_char,
    file: *const c_char,
) -> *mut c_char {
    // SAFETY: the caller guarantees NUL-terminated strings.
    let (data, file) = unsafe { (bytes(data_path), bytes(file)) };
    module_paths::find_file(&LibobsFs, data, file).map_or(ptr::null_mut(), |p| bstrdup(&p))
}

/// Same contract as C `obs_module_paths_find`.
///
/// # Safety
///
/// `bin`, `data` and `ext` are null or NUL-terminated. `add_extension` and
/// `callback` are null or safe to call (with `param`) during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_paths_find(
    bin: *const c_char,
    data: *const c_char,
    module_type: c_int,
    ext: *const c_char,
    add_extension: obs_module_add_extension_t,
    callback: obs_find_module_callback2_t,
    param: *mut c_void,
) {
    let Some(callback) = callback else {
        return;
    };
    // SAFETY: the caller guarantees NUL-terminated strings.
    let (bin, data, ext) = unsafe { (bytes(bin), bytes(data), bytes(ext)) };
    let mut add = |path: &mut Vec<u8>| {
        if let Some(hook) = add_extension {
            // SAFETY: the caller's hook, with a valid `dstr`.
            unsafe { call_add_extension(hook, path) };
        }
    };
    let mut report = |info: &ModuleInfo<'_>| {
        let bin_path = info.bin_path.and_then(c_path);
        let data_path = c_path(info.data_path);
        let name = info.name.and_then(c_path);
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
    module_paths::find(
        &LibobsFs,
        bin,
        data,
        ext.unwrap_or_default(),
        &mut add,
        &mut report,
    );
}
