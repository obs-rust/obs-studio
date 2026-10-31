//! C ABI shim for `libobs/util/platform-windows.c`.
//!
//! The shared `os_*` names also exist in `platform_nix.rs` inside
//! `#[cfg(unix)]`. This file is the Windows copy, so `rust.cmake` emits
//! `/EXPORT` for them. `get_plugin_info`, `get_win_release_id`, and
//! `DllMain` are hidden (`rust-hidden.d`).
//!
//! A null `os_glob` out-pointer returns -1 and writes nothing. C would
//! write through it. Null directory and module handles are no-ops.

use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use core::slice;

use crate::platform::windows::host;

#[repr(C)]
pub struct os_dirent {
    pub d_name: [c_char; 256],
    pub directory: bool,
}

#[repr(C)]
pub struct os_globent {
    pub path: *mut c_char,
    pub directory: bool,
}

#[repr(C)]
pub struct os_glob_info {
    pub gl_pathc: usize,
    pub gl_pathv: *mut os_globent,
}

#[repr(C)]
pub struct os_proc_memory_usage {
    pub resident_size: u64,
    pub virtual_size: u64,
}

#[repr(C)]
pub struct win_version_info {
    pub major: c_int,
    pub minor: c_int,
    pub build: c_int,
    pub revis: c_int,
}

#[repr(C)]
pub struct reg_dword {
    pub status: i32,
    pub size: u32,
    pub return_value: u32,
}

fn cstr<'a>(p: *const c_char) -> Option<&'a [u8]> {
    if p.is_null() {
        return None;
    }
    // SAFETY: `p` is a C string when non-null.
    Some(unsafe { core::ffi::CStr::from_ptr(p) }.to_bytes())
}

fn wstr<'a>(p: *const u16) -> &'a [u16] {
    if p.is_null() {
        return &[];
    }
    let mut n = 0usize;
    // SAFETY: `p` is a wide C string. The scan stops at the first NUL.
    while unsafe { *p.add(n) } != 0 {
        n += 1;
    }
    // SAFETY: `n` units plus the NUL were just read.
    unsafe { slice::from_raw_parts(p, n + 1) }
}

/// `bmalloc` of `bytes` plus a NUL.
unsafe fn alloc_bytes(bytes: &[u8]) -> *mut c_char {
    // SAFETY: the size is `len + 1`, never zero.
    let p = unsafe { crate::ffi::bmem::bmalloc(bytes.len() + 1) }.cast::<u8>();
    // SAFETY: `bmalloc` returned `len + 1` bytes.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        *p.add(bytes.len()) = 0;
    }
    p.cast()
}

fn copy_out(dst: *mut c_char, size: usize, bytes: &[u8]) -> c_int {
    if dst.is_null() || size == 0 || bytes.len() >= size {
        return -1;
    }
    // SAFETY: `dst` holds `size` bytes and `bytes` fits with a NUL.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), dst.cast(), bytes.len());
        *dst.add(bytes.len()) = 0;
    }
    bytes.len() as c_int
}

struct DirHandle {
    dir: host::Dir,
    out: os_dirent,
}

fn store_name(dst: &mut [c_char; 256], name: &[u8]) {
    *dst = [0; 256];
    let n = name.len().min(dst.len() - 1);
    for (i, b) in name.iter().take(n).enumerate() {
        dst[i] = *b as c_char;
    }
}

unsafe fn store_glob(ents: &[host::GlobEnt]) -> *mut os_glob_info {
    // SAFETY: the info struct is non-zero.
    let info =
        unsafe { crate::ffi::bmem::bmalloc(size_of::<os_glob_info>()) }.cast::<os_glob_info>();
    let pathv = if ents.is_empty() {
        ptr::null_mut()
    } else {
        // SAFETY: `ents` is non-empty, so the size is non-zero.
        unsafe { crate::ffi::bmem::bmalloc(ents.len() * size_of::<os_globent>()) }
            .cast::<os_globent>()
    };
    for (i, ent) in ents.iter().enumerate() {
        // SAFETY: `alloc_bytes` returns a NUL-terminated copy.
        let path = unsafe { alloc_bytes(&ent.path) };
        // SAFETY: `i` is inside the `pathv` allocation.
        unsafe {
            (*pathv.add(i)).path = path;
            (*pathv.add(i)).directory = ent.directory;
        }
    }
    // SAFETY: `info` is the block just allocated.
    unsafe {
        (*info).gl_pathc = ents.len();
        (*info).gl_pathv = pathv;
    }
    info
}

fn ver_of(ver: host::WinVer) -> win_version_info {
    win_version_info {
        major: ver.major,
        minor: ver.minor,
        build: ver.build,
        revis: ver.revis,
    }
}

#[cfg(windows)]
mod shims {
    use super::{
        DirHandle, alloc_bytes, c_char, c_int, c_void, copy_out, cstr, host, os_dirent,
        os_glob_info, os_proc_memory_usage, ptr, reg_dword, store_glob, store_name, ver_of,
        win_version_info, wstr,
    };

    /// # Safety
    /// `path` is a C string or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_dlopen(path: *const c_char) -> *mut c_void {
        let Some(path) = cstr(path) else {
            return ptr::null_mut();
        };
        host::dlopen(path).unwrap_or(ptr::null_mut())
    }

    /// # Safety
    /// `module` is a `LoadLibrary` handle or null. `func` is a C string or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_dlsym(module: *mut c_void, func: *const c_char) -> *mut c_void {
        let Some(func) = cstr(func) else {
            return ptr::null_mut();
        };
        // SAFETY: `module` is a handle or null. `dlsym` does not dereference it.
        unsafe { host::dlsym(module, func) }
    }

    /// # Safety
    /// `module` is a `LoadLibrary` handle or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_dlclose(module: *mut c_void) {
        // SAFETY: `module` is a handle or null. `dlclose` does not dereference it.
        unsafe { host::dlclose(module) };
    }

    /// # Safety
    /// `path` is a C string or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_is_obs_plugin(path: *const c_char) -> bool {
        match cstr(path) {
            Some(path) => host::is_obs_plugin(path),
            None => false,
        }
    }

    /// # Safety
    /// `time_target` is a nanosecond deadline. `0` does not sleep.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_sleepto_ns(time_target: u64) -> bool {
        host::sleepto_ns(time_target)
    }

    /// # Safety
    /// `time_target` is a nanosecond deadline. `0` does not sleep.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_sleepto_ns_fast(time_target: u64) -> bool {
        host::sleepto_ns_fast(time_target)
    }

    /// # Safety
    /// `duration` is a millisecond count. Windows 8+ shortens a positive value by 1.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_sleep_ms(duration: u32) {
        host::sleep_ms(duration);
    }

    /// # Safety
    /// `path` is a C string or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_file_exists(path: *const c_char) -> bool {
        match cstr(path) {
            Some(path) => host::file_exists(path),
            None => false,
        }
    }

    /// # Safety
    /// `path` is a C string or null. `abspath` is `size` bytes or null.
    /// A null buffer returns 0.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_abs_path(
        path: *const c_char,
        abspath: *mut c_char,
        size: usize,
    ) -> usize {
        if abspath.is_null() || size == 0 {
            return 0;
        }
        let Some(path) = cstr(path) else {
            return 0;
        };
        let Some(abs) = host::abs_path(path) else {
            return 0;
        };
        if abs.len() >= size {
            return 0;
        }
        // SAFETY: `abspath` holds `size` bytes and `abs` fits with a NUL.
        unsafe {
            ptr::copy_nonoverlapping(abs.as_ptr(), abspath.cast(), abs.len());
            *abspath.add(abs.len()) = 0;
        }
        abs.len()
    }

    /// # Safety
    /// `path` is a C string or null. The result is `bmalloc`'d, or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_abs_path_ptr(path: *const c_char) -> *mut c_char {
        let Some(path) = cstr(path) else {
            return ptr::null_mut();
        };
        let Some(abs) = host::abs_path(path) else {
            return ptr::null_mut();
        };
        // SAFETY: `abs` is non-empty when `abs_path` returns it.
        unsafe { alloc_bytes(&abs) }
    }

    /// # Safety
    /// `path` is a C string or null. The handle is opaque. Close it with
    /// `os_closedir`. A null path searches `/*.*`, as C does.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_opendir(path: *const c_char) -> *mut c_void {
        let path = cstr(path).unwrap_or(b"");
        match host::opendir(path) {
            Some(dir) => Box::into_raw(Box::new(DirHandle {
                dir,
                out: os_dirent {
                    d_name: [0; 256],
                    directory: false,
                },
            }))
            .cast(),
            None => ptr::null_mut(),
        }
    }

    /// # Safety
    /// `dir` came from `os_opendir` or is null. The returned `os_dirent` is
    /// overwritten by the next call and freed by `os_closedir`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_readdir(dir: *mut c_void) -> *mut os_dirent {
        if dir.is_null() {
            return ptr::null_mut();
        }
        // SAFETY: `dir` is a live `DirHandle`.
        let dir = unsafe { &mut *dir.cast::<DirHandle>() };
        let Some(ent) = dir.dir.read() else {
            return ptr::null_mut();
        };
        store_name(&mut dir.out.d_name, &ent.name);
        dir.out.directory = ent.directory;
        &mut dir.out
    }

    /// # Safety
    /// `dir` came from `os_opendir` or is null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_closedir(dir: *mut c_void) {
        if dir.is_null() {
            return;
        }
        // SAFETY: `dir` is a live `DirHandle` from `Box::into_raw`.
        drop(unsafe { Box::from_raw(dir.cast::<DirHandle>()) });
    }

    /// # Safety
    /// `pattern` is a C string or null. `pglob` is an out-pointer or null.
    /// A null `pglob` returns -1 and writes nothing. On success `*pglob`
    /// is freed with `os_globfree`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_glob(
        pattern: *const c_char,
        _flags: c_int,
        pglob: *mut *mut os_glob_info,
    ) -> c_int {
        if pglob.is_null() {
            return -1;
        }
        let Some(pattern) = cstr(pattern) else {
            // SAFETY: `pglob` is non-null.
            unsafe { *pglob = ptr::null_mut() };
            return -1;
        };
        match host::glob(pattern) {
            None => {
                // SAFETY: `pglob` is non-null.
                unsafe { *pglob = ptr::null_mut() };
                -1
            }
            Some(ents) => {
                // SAFETY: `pglob` is non-null. Each path is `bmalloc`'d.
                unsafe { *pglob = store_glob(&ents) };
                0
            }
        }
    }

    /// # Safety
    /// `pglob` came from `os_glob` or is null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_globfree(pglob: *mut os_glob_info) {
        if pglob.is_null() {
            return;
        }
        // SAFETY: `pglob` is a live glob from `store_glob`.
        unsafe {
            let n = (*pglob).gl_pathc;
            let pathv = (*pglob).gl_pathv;
            if !pathv.is_null() {
                for i in 0..n {
                    crate::ffi::bmem::bfree((*pathv.add(i)).path.cast());
                }
                crate::ffi::bmem::bfree(pathv.cast());
            }
            crate::ffi::bmem::bfree(pglob.cast());
        }
    }

    /// # Safety
    /// `path` is a C string or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_unlink(path: *const c_char) -> c_int {
        match cstr(path) {
            Some(path) => host::unlink(path),
            None => -1,
        }
    }

    /// # Safety
    /// `path` is a C string or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_rmdir(path: *const c_char) -> c_int {
        match cstr(path) {
            Some(path) => host::rmdir(path),
            None => -1,
        }
    }

    /// # Safety
    /// `path` is a C string or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_mkdir(path: *const c_char) -> c_int {
        match cstr(path) {
            Some(path) => host::mkdir(path),
            None => -1,
        }
    }

    /// # Safety
    /// Both paths are C strings or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_rename(old_path: *const c_char, new_path: *const c_char) -> c_int {
        match (cstr(old_path), cstr(new_path)) {
            (Some(old_path), Some(new_path)) => host::rename(old_path, new_path),
            _ => -1,
        }
    }

    /// # Safety
    /// `target` and `from` are C strings or null. `backup` null skips the backup.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_safe_replace(
        target: *const c_char,
        from: *const c_char,
        backup: *const c_char,
    ) -> c_int {
        let (Some(target), Some(from)) = (cstr(target), cstr(from)) else {
            return -1;
        };
        let backup = if backup.is_null() { None } else { cstr(backup) };
        host::safe_replace(target, from, backup)
    }

    /// # Safety
    /// Both paths are C strings or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_copyfile(file_in: *const c_char, file_out: *const c_char) -> c_int {
        match (cstr(file_in), cstr(file_out)) {
            (Some(file_in), Some(file_out)) => host::copy_file(file_in, file_out),
            _ => -1,
        }
    }

    /// # Safety
    /// `path` is `size` bytes or null. A null buffer returns null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_getcwd(path: *mut c_char, size: usize) -> *mut c_char {
        if path.is_null() || size == 0 {
            return ptr::null_mut();
        }
        let Some(cwd) = host::getcwd() else {
            return ptr::null_mut();
        };
        let n = cwd.len().min(size - 1);
        // SAFETY: `path` holds `size` bytes.
        unsafe {
            ptr::copy_nonoverlapping(cwd.as_ptr(), path.cast(), n);
            *path.add(n) = 0;
        }
        path
    }

    /// # Safety
    /// `path` is a C string or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_chdir(path: *const c_char) -> c_int {
        match cstr(path) {
            Some(path) => host::chdir(path),
            None => -1,
        }
    }

    /// # Safety
    /// No pointer arguments. Debug breaks. A release build raises
    /// `ERROR_OUTOFMEMORY`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_breakpoint() {
        host::breakpoint();
    }

    /// # Safety
    /// No pointer arguments. Debug breaks. A release build raises
    /// `ERROR_OUTOFMEMORY`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_oom() {
        host::oom();
    }

    /// # Safety
    /// The result is `bmalloc`'d. Two calls do not return the same text.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_generate_uuid() -> *mut c_char {
        let text = host::generate_uuid();
        // SAFETY: the formatted UUID is 36 bytes.
        unsafe { alloc_bytes(text.as_bytes()) }
    }

    /// # Safety
    /// The handle is opaque. Destroy it with `os_cpu_usage_info_destroy`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_cpu_usage_info_start() -> *mut c_void {
        Box::into_raw(Box::new(host::cpu_start())).cast()
    }

    /// # Safety
    /// `info` came from `os_cpu_usage_info_start` or is null. Null returns 0.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_cpu_usage_info_query(info: *mut c_void) -> f64 {
        if info.is_null() {
            return 0.0;
        }
        // SAFETY: `info` is a live `CpuInfo`.
        host::cpu_query(unsafe { &mut *info.cast::<host::CpuInfo>() })
    }

    /// # Safety
    /// `info` came from `os_cpu_usage_info_start` or is null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_cpu_usage_info_destroy(info: *mut c_void) {
        if info.is_null() {
            return;
        }
        // SAFETY: `info` is a live `CpuInfo` from `Box::into_raw`.
        drop(unsafe { Box::from_raw(info.cast::<host::CpuInfo>()) });
    }

    /// # Safety
    /// No pointer arguments.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_gettime_ns() -> u64 {
        host::gettime_ns()
    }

    /// # Safety
    /// `dst` is `size` bytes or null. `name` is a C string or null.
    /// A null `name` is the folder with no extra component.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_config_path(
        dst: *mut c_char,
        size: usize,
        name: *const c_char,
    ) -> c_int {
        let name = cstr(name).unwrap_or(b"");
        match host::config_path(name, size) {
            Some(bytes) => copy_out(dst, size, &bytes),
            None => -1,
        }
    }

    /// # Safety
    /// `name` is a C string or null. The result is `bmalloc`'d.
    /// A null `name` leaves the trailing backslash, as `dstr_cat` does.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_config_path_ptr(name: *const c_char) -> *mut c_char {
        let name = cstr(name).unwrap_or(b"");
        let bytes = host::config_path_ptr(name);
        // SAFETY: the path contains the folder or a backslash.
        unsafe { alloc_bytes(&bytes) }
    }

    /// # Safety
    /// `dst` is `size` bytes or null. `name` is a C string or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_program_data_path(
        dst: *mut c_char,
        size: usize,
        name: *const c_char,
    ) -> c_int {
        let name = cstr(name).unwrap_or(b"");
        match host::program_data_path(name, size) {
            Some(bytes) => copy_out(dst, size, &bytes),
            None => -1,
        }
    }

    /// # Safety
    /// `name` is a C string or null. The result is `bmalloc`'d.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_program_data_path_ptr(name: *const c_char) -> *mut c_char {
        let name = cstr(name).unwrap_or(b"");
        let bytes = host::program_data_path_ptr(name);
        // SAFETY: the path contains the folder or a backslash.
        unsafe { alloc_bytes(&bytes) }
    }

    /// # Safety
    /// `name` is a C string or null. The result is `bmalloc`'d.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_executable_path_ptr(name: *const c_char) -> *mut c_char {
        let name = cstr(name).unwrap_or(b"");
        let bytes = host::executable_path_ptr(name);
        // SAFETY: the path is the module directory, possibly empty plus `name`.
        unsafe { alloc_bytes(&bytes) }
    }

    /// # Safety
    /// No pointer arguments.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_emulation_status() -> bool {
        host::emulation_status()
    }

    /// # Safety
    /// `path` is a C string or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_free_space(path: *const c_char) -> i64 {
        match cstr(path) {
            Some(path) => host::free_space(path),
            None => -1,
        }
    }

    /// # Safety
    /// `reason` is ignored. The result is null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_request_high_performance(_reason: *const c_char) -> *mut c_void {
        host::high_performance_is_null();
        ptr::null_mut()
    }

    /// # Safety
    /// `token` is ignored.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_end_high_performance(_token: *mut c_void) {}

    /// # Safety
    /// No pointer arguments.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_physical_cores() -> c_int {
        host::physical_cores()
    }

    /// # Safety
    /// No pointer arguments.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_logical_cores() -> c_int {
        host::logical_cores()
    }

    /// # Safety
    /// No pointer arguments.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_sys_free_size() -> u64 {
        host::sys_free()
    }

    /// # Safety
    /// `usage` is a writable `os_proc_memory_usage` or null. Null returns false.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_proc_memory_usage(usage: *mut os_proc_memory_usage) -> bool {
        if usage.is_null() {
            return false;
        }
        let Some((resident, virtual_size)) = host::proc_memory() else {
            return false;
        };
        // SAFETY: `usage` is non-null.
        unsafe {
            (*usage).resident_size = resident;
            (*usage).virtual_size = virtual_size;
        }
        true
    }

    /// # Safety
    /// No pointer arguments.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_proc_resident_size() -> u64 {
        host::proc_resident()
    }

    /// # Safety
    /// No pointer arguments.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_proc_virtual_size() -> u64 {
        host::proc_virtual()
    }

    /// # Safety
    /// No pointer arguments.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_sys_total_size() -> u64 {
        host::sys_total()
    }

    /// # Safety
    /// `dir` is a C string or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_free_disk_space(dir: *const c_char) -> u64 {
        match cstr(dir) {
            // Bounded: a hung share must not stall the caller (#1345).
            Some(dir) => crate::bounded_free_space::shared(host::free_disk_space).get(dir),
            None => 0,
        }
    }

    /// # Safety
    /// `reason` is ignored. Destroy the result with `os_inhibit_sleep_destroy`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_inhibit_sleep_create(_reason: *const c_char) -> *mut c_void {
        Box::into_raw(Box::new(host::inhibit_create())).cast()
    }

    /// # Safety
    /// `info` came from `os_inhibit_sleep_create`. Null returns false.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_inhibit_sleep_set_active(info: *mut c_void, active: bool) -> bool {
        if info.is_null() {
            return false;
        }
        // SAFETY: `info` is a live `Inhibit`.
        host::inhibit_set(unsafe { &mut *info.cast::<host::Inhibit>() }, active)
    }

    /// # Safety
    /// `info` came from `os_inhibit_sleep_create` or is null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_inhibit_sleep_destroy(info: *mut c_void) {
        if info.is_null() {
            return;
        }
        // SAFETY: `info` is a live `Inhibit` from `Box::into_raw`.
        let info = unsafe { Box::from_raw(info.cast::<host::Inhibit>()) };
        host::inhibit_destroy(*info);
    }

    /// # Safety
    /// No pointer arguments.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn is_64_bit_windows() -> bool {
        host::is_64_bit_windows()
    }

    /// # Safety
    /// No pointer arguments.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn is_arm64_windows() -> bool {
        host::is_arm64_windows()
    }

    /// # Safety
    /// `lib` is a wide C string or null. `info` is writable or null.
    /// A null argument returns false.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn get_dll_ver(lib: *const u16, info: *mut win_version_info) -> bool {
        if lib.is_null() || info.is_null() {
            return false;
        }
        let Some(ver) = host::dll_ver(wstr(lib)) else {
            return false;
        };
        // SAFETY: `info` is non-null.
        unsafe { *info = ver_of(ver) };
        true
    }

    /// # Safety
    /// `info` is writable or null. Null returns. `sub_key` and `value_name`
    /// are wide C strings.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn get_reg_dword(
        key: *mut c_void,
        sub_key: *const u16,
        value_name: *const u16,
        info: *mut reg_dword,
    ) {
        if info.is_null() {
            return;
        }
        let got = host::reg_dword(key as isize, wstr(sub_key), wstr(value_name));
        // SAFETY: `info` is non-null.
        unsafe {
            (*info).status = got.status;
            (*info).size = got.size;
            (*info).return_value = got.value;
        }
    }

    /// # Safety
    /// `info` is writable or null. Null returns.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn get_win_ver(info: *mut win_version_info) {
        if info.is_null() {
            return;
        }
        // SAFETY: `info` is non-null.
        unsafe { *info = ver_of(host::win_ver()) };
    }

    /// # Safety
    /// No pointer arguments.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn get_win_ver_int() -> u32 {
        host::win_ver_int()
    }
}

#[cfg(windows)]
pub use shims::*;

/// # Safety
/// `is_obs_plugin` is a `bool` out-pointer or null. Null returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_plugin_info(path: *const c_char, is_obs_plugin: *mut bool) {
    if is_obs_plugin.is_null() {
        return;
    }
    let path = cstr(path).unwrap_or(b"");
    // SAFETY: `is_obs_plugin` is non-null.
    unsafe { *is_obs_plugin = host::is_obs_plugin(path) };
}

/// # Safety
/// The pointer is the process-lifetime release-id buffer. Do not free it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_win_release_id() -> *const c_char {
    host::win_release_id().cast()
}

/// # Safety
/// Called by the loader. `reason` is a `DLL_*` value. The pthread
/// static-lib hooks are not called: libobs does not define `PTW32_STATIC_LIB`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllMain(
    _hinst: *mut c_void,
    reason: u32,
    _reserved: *mut c_void,
) -> i32 {
    host::dllmain(reason);
    1
}
