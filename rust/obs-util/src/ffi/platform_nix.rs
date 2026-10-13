//! C ABI shim for `libobs/util/platform-nix.c`.
//!
//! macOS keeps config paths, the clock, CPU, memory, and sleep inhibit in
//! `platform-cocoa.m`. Those shims are compiled out here.
//!
//! `os_inhibit_sleep_*` has no shim on any platform. Linux and FreeBSD keep
//! it in C (`util/platform-nix-inhibit.c`, built in both lanes) because it
//! talks to the xdg-desktop-portal and session D-Bus inhibitors through GIO.

use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr;
use core::slice;

use crate::platform::unix::nix;

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

fn cstr<'a>(p: *const c_char) -> Option<&'a [u8]> {
    if p.is_null() {
        return None;
    }
    // SAFETY: `p` is a C string when non-null.
    Some(unsafe { CStr::from_ptr(p) }.to_bytes())
}

/// `bmalloc` of `bytes` plus a NUL. `bytes` is non-empty or the caller
/// still asks for at least one byte.
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

#[cfg(not(target_os = "macos"))]
fn write_out(dst: *mut c_char, size: usize, bytes: &[u8]) -> c_int {
    if dst.is_null() || size == 0 {
        return nix::snprintf_bytes(None, bytes);
    }
    // SAFETY: the caller guarantees `size` bytes at `dst`.
    let buf = unsafe { slice::from_raw_parts_mut(dst.cast::<u8>(), size) };
    nix::snprintf_bytes(Some(buf), bytes)
}

struct DirHandle {
    dir: nix::Dir,
    out: os_dirent,
}

#[cfg(unix)]
mod shims {
    use super::*;

    /// # Safety
    /// `path` is a C string or null. Null returns null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_dlopen(path: *const c_char) -> *mut c_void {
        let Some(path) = cstr(path) else {
            return ptr::null_mut();
        };
        nix::dlopen(path).unwrap_or(ptr::null_mut())
    }

    /// # Safety
    /// `module` is a `dlopen` handle or null. `func` is a C string or null.
    /// A null `func` returns null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_dlsym(module: *mut c_void, func: *const c_char) -> *mut c_void {
        let Some(func) = cstr(func) else {
            return ptr::null_mut();
        };
        // SAFETY: `module` is a dlopen handle or null, and `func` is a C string.
        unsafe { nix::dlsym(module, func) }
    }

    /// # Safety
    /// `module` is a `dlopen` handle or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_dlclose(module: *mut c_void) {
        // SAFETY: `module` is a dlopen handle or null.
        unsafe { nix::dlclose(module) };
    }

    /// # Safety
    /// `path` is unused. Null is accepted.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_is_obs_plugin(path: *const c_char) -> bool {
        nix::is_obs_plugin(cstr(path).unwrap_or(b""))
    }

    /// # Safety
    /// No pointer arguments.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_sleepto_ns(time_target: u64) -> bool {
        nix::sleepto_ns(time_target)
    }

    /// # Safety
    /// No pointer arguments.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_sleepto_ns_fast(time_target: u64) -> bool {
        nix::sleepto_ns_fast(time_target)
    }

    /// # Safety
    /// No pointer arguments.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_sleep_ms(duration: u32) {
        nix::sleep_ms(duration);
    }

    /// # Safety
    /// `path` is a C string or null. Null returns false.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_file_exists(path: *const c_char) -> bool {
        cstr(path).is_some_and(nix::file_exists)
    }

    /// # Safety
    /// `path` is a C string or null. `abspath` is `size` bytes or null.
    /// A null `abspath` returns 0.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_abs_path(
        path: *const c_char,
        abspath: *mut c_char,
        size: usize,
    ) -> usize {
        if abspath.is_null() {
            return 0;
        }
        let Some(path) = cstr(path) else {
            return 0;
        };
        if size == 0 {
            return nix::abs_path_into(path, Some(&mut []));
        }
        // SAFETY: the caller guarantees `size` bytes at `abspath`.
        let dst = unsafe { slice::from_raw_parts_mut(abspath.cast::<u8>(), size) };
        nix::abs_path_into(path, Some(dst))
    }

    /// # Safety
    /// `path` is a C string or null. The result is `bmalloc`'d, or null.
    /// Free it with `bfree`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_get_abs_path_ptr(path: *const c_char) -> *mut c_char {
        let Some(path) = cstr(path) else {
            return ptr::null_mut();
        };
        // SAFETY: 512 bytes, matching the C allocation.
        let buf = unsafe { crate::ffi::bmem::bmalloc(512) }.cast::<u8>();
        // SAFETY: `buf` holds 512 bytes.
        let dst = unsafe { slice::from_raw_parts_mut(buf, 512) };
        if nix::abs_path_into(path, Some(dst)) == 0 {
            // SAFETY: `buf` came from `bmalloc`.
            unsafe { crate::ffi::bmem::bfree(buf.cast()) };
            return ptr::null_mut();
        }
        buf.cast()
    }

    /// # Safety
    /// `path` is a C string or null. The handle is opaque. Close it with
    /// `os_closedir`. The path is copied, so the caller may free it.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_opendir(path: *const c_char) -> *mut c_void {
        let Some(path) = cstr(path) else {
            return ptr::null_mut();
        };
        match nix::opendir(path) {
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
        dir.out.d_name = [0; 256];
        for (i, b) in ent.name.iter().enumerate() {
            dir.out.d_name[i] = *b as c_char;
        }
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
    /// On success `*pglob` is freed with `os_globfree`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_glob(
        pattern: *const c_char,
        _flags: c_int,
        pglob: *mut *mut os_glob_info,
    ) -> c_int {
        let Some(pattern) = cstr(pattern) else {
            if !pglob.is_null() {
                // SAFETY: `pglob` is non-null.
                unsafe { *pglob = ptr::null_mut() };
            }
            return libc::GLOB_NOMATCH;
        };
        match nix::glob(pattern) {
            Err(rc) => {
                if !pglob.is_null() {
                    // SAFETY: `pglob` is non-null.
                    unsafe { *pglob = ptr::null_mut() };
                }
                rc
            }
            Ok(ents) => {
                if pglob.is_null() {
                    return 0;
                }
                // SAFETY: `pglob` is non-null. Each path is `bmalloc`'d.
                unsafe { *pglob = store_glob(&ents) };
                0
            }
        }
    }

    unsafe fn store_glob(ents: &[nix::GlobEnt]) -> *mut os_glob_info {
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
            Some(path) => nix::unlink(path),
            None => -1,
        }
    }

    /// # Safety
    /// `path` is a C string or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_rmdir(path: *const c_char) -> c_int {
        match cstr(path) {
            Some(path) => nix::rmdir(path),
            None => -1,
        }
    }

    /// # Safety
    /// `path` is a C string or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_mkdir(path: *const c_char) -> c_int {
        match cstr(path) {
            Some(path) => nix::mkdir(path),
            None => -1,
        }
    }

    /// # Safety
    /// Both paths are C strings or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_rename(old_path: *const c_char, new_path: *const c_char) -> c_int {
        match (cstr(old_path), cstr(new_path)) {
            (Some(old_path), Some(new_path)) => nix::rename(old_path, new_path),
            _ => -1,
        }
    }

    /// # Safety
    /// `target` and `from` are C strings or null. `backup` null skips the
    /// backup rename.
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
        nix::safe_replace(target, from, backup)
    }

    /// # Safety
    /// Both paths are C strings or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_copyfile(file_in: *const c_char, file_out: *const c_char) -> c_int {
        match (cstr(file_in), cstr(file_out)) {
            (Some(file_in), Some(file_out)) => nix::copy_file(file_in, file_out),
            _ => -1,
        }
    }

    /// # Safety
    /// `path` is `size` bytes or null. A null buffer is the allocating
    /// `getcwd` form. Free that pointer with `free`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_getcwd(path: *mut c_char, size: usize) -> *mut c_char {
        // SAFETY: same contract as `getcwd(3)`. A null `path` allocates with
        // `malloc` on macOS and Linux. The caller frees that pointer with
        // `free`, not `bfree`.
        unsafe { libc::getcwd(path, size) }
    }

    /// # Safety
    /// `path` is a C string or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_chdir(path: *const c_char) -> c_int {
        match cstr(path) {
            Some(path) => nix::chdir(path),
            None => -1,
        }
    }

    /// # Safety
    /// Raises `SIGTRAP`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_breakpoint() {
        nix::breakpoint();
    }

    /// # Safety
    /// Raises `SIGTRAP`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_oom() {
        nix::oom();
    }

    /// # Safety
    /// The result is `bmalloc`'d. Free it with `bfree`.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn os_generate_uuid() -> *mut c_char {
        let text = nix::generate_uuid();
        // SAFETY: 36 bytes plus the NUL `alloc_bytes` adds.
        unsafe { alloc_bytes(&text) }
    }

    #[cfg(not(target_os = "macos"))]
    mod non_apple {
        use super::*;

        unsafe extern "C" {
            fn bcrash(format: *const c_char, ...) -> !;
        }

        /// # Safety
        /// The result is opaque. Destroy it with `os_cpu_usage_info_destroy`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_cpu_usage_info_start() -> *mut c_void {
            Box::into_raw(Box::new(nix::cpu_start())).cast()
        }

        /// # Safety
        /// `info` came from `os_cpu_usage_info_start` or is null.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_cpu_usage_info_query(info: *mut c_void) -> f64 {
            if info.is_null() {
                return 0.0;
            }
            // SAFETY: `info` is a live `CpuUsage`.
            unsafe { &mut *info.cast::<nix::CpuUsage>() }.query()
        }

        /// # Safety
        /// `info` came from `os_cpu_usage_info_start` or is null.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_cpu_usage_info_destroy(info: *mut c_void) {
            if info.is_null() {
                return;
            }
            // SAFETY: `info` is a live `CpuUsage` from `Box::into_raw`.
            drop(unsafe { Box::from_raw(info.cast::<nix::CpuUsage>()) });
        }

        /// # Safety
        /// No pointer arguments.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_gettime_ns() -> u64 {
            nix::gettime_ns()
        }

        /// # Safety
        /// `dst` is `size` bytes or null. `name` is a C string or null.
        /// A missing `$HOME` calls `bcrash`, as in C.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_get_config_path(
            dst: *mut c_char,
            size: usize,
            name: *const c_char,
        ) -> c_int {
            let bytes = config_bytes(name, false);
            write_out(dst, size, &bytes)
        }

        /// # Safety
        /// `name` is a C string or null. The result is `bmalloc`'d.
        /// A missing `$HOME` calls `bcrash`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_get_config_path_ptr(name: *const c_char) -> *mut c_char {
            let bytes = config_bytes(name, true);
            // SAFETY: the path is non-empty (`/` is always present in the ptr form).
            unsafe { alloc_bytes(&bytes) }
        }

        fn config_bytes(name: *const c_char, ptr_form: bool) -> Vec<u8> {
            let name = cstr(name);
            let result = if ptr_form {
                nix::config_path_ptr(name)
            } else {
                nix::config_path(name)
            };
            match result {
                Ok(bytes) => bytes,
                Err(nix::MissingHome) => {
                    // SAFETY: the format matches the C crash.
                    unsafe { bcrash(c"Could not get $HOME\n".as_ptr()) }
                }
            }
        }

        /// # Safety
        /// `dst` is `size` bytes or null. `name` is a C string or null.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_get_program_data_path(
            dst: *mut c_char,
            size: usize,
            name: *const c_char,
        ) -> c_int {
            write_out(dst, size, &nix::program_data_path(cstr(name)))
        }

        /// # Safety
        /// `name` is a C string or null. The result is `bmalloc`'d.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_get_program_data_path_ptr(name: *const c_char) -> *mut c_char {
            let bytes = nix::program_data_path(cstr(name));
            // SAFETY: the prefix is non-empty.
            unsafe { alloc_bytes(&bytes) }
        }

        /// # Safety
        /// `name` is a C string or null. The result is `bmalloc`'d, or null
        /// when the executable path cannot be read.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_get_executable_path_ptr(name: *const c_char) -> *mut c_char {
            let Some(bytes) = nix::executable_path(cstr(name)) else {
                return ptr::null_mut();
            };
            // SAFETY: the path contains at least the trailing slash.
            unsafe { alloc_bytes(&bytes) }
        }

        /// # Safety
        /// No pointer arguments.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_get_emulation_status() -> bool {
            nix::emulation_status()
        }

        /// # Safety
        /// `path` is a C string or null.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_get_free_space(path: *const c_char) -> i64 {
            match cstr(path) {
                Some(path) => nix::free_space(path),
                None => -1,
            }
        }

        /// # Safety
        /// `reason` is unused. Always returns null.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_request_high_performance(
            _reason: *const c_char,
        ) -> *mut c_void {
            ptr::null_mut()
        }

        /// # Safety
        /// `token` is unused.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_end_high_performance(_token: *mut c_void) {}

        /// # Safety
        /// No pointer arguments.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_get_physical_cores() -> c_int {
            nix::physical()
        }

        /// # Safety
        /// No pointer arguments.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_get_logical_cores() -> c_int {
            nix::logical()
        }

        /// # Safety
        /// No pointer arguments.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_get_sys_free_size() -> u64 {
            nix::sys_free()
        }

        /// # Safety
        /// `usage` is an out-pointer or null. Null returns false.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_get_proc_memory_usage(
            usage: *mut os_proc_memory_usage,
        ) -> bool {
            if usage.is_null() {
                return false;
            }
            let Some(mem) = nix::proc_memory() else {
                return false;
            };
            // SAFETY: `usage` is non-null.
            unsafe {
                (*usage).resident_size = mem.resident;
                (*usage).virtual_size = mem.virtual_size;
            }
            true
        }

        /// # Safety
        /// No pointer arguments.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_get_proc_resident_size() -> u64 {
            nix::proc_memory().map(|m| m.resident).unwrap_or(0)
        }

        /// # Safety
        /// No pointer arguments. The value is the raw `statm` field.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_get_proc_virtual_size() -> u64 {
            nix::proc_memory().map(|m| m.virtual_size).unwrap_or(0)
        }

        /// # Safety
        /// No pointer arguments.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_get_sys_total_size() -> u64 {
            nix::sys_total()
        }

        /// # Safety
        /// `dir` is a C string or null.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn os_get_free_disk_space(dir: *const c_char) -> u64 {
            match cstr(dir) {
                Some(dir) => nix::free_disk_space(dir),
                None => 0,
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub use non_apple::*;
}

#[cfg(unix)]
pub use shims::*;

// Not a libobs export. obs-module.c calls it inside the library.
// Kept outside `mod shims`: a symbol in that module has to be listed in
// rust-exports.d, and this one is hidden instead (rust-hidden.txt).
/// # Safety
/// `is_obs_plugin` is a `bool` out-pointer or null. Null returns.
/// C would write through it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_plugin_info(path: *const c_char, is_obs_plugin: *mut bool) {
    if is_obs_plugin.is_null() {
        return;
    }
    let path = cstr(path).unwrap_or(b"");
    // SAFETY: `is_obs_plugin` is non-null.
    unsafe { *is_obs_plugin = nix::is_obs_plugin(path) };
}
