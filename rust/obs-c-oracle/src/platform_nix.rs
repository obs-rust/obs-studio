//! Oracle entry points for `libobs/util/platform-nix.c`.

use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    pub fn oracle_os_dlopen(path: *const c_char) -> *mut c_void;
    pub fn oracle_os_dlsym(module: *mut c_void, func: *const c_char) -> *mut c_void;
    pub fn oracle_os_dlclose(module: *mut c_void);
    pub fn oracle_get_plugin_info(path: *const c_char, is_obs_plugin: *mut bool);
    pub fn oracle_os_is_obs_plugin(path: *const c_char) -> bool;
    pub fn oracle_os_sleepto_ns(time_target: u64) -> bool;
    pub fn oracle_os_sleepto_ns_fast(time_target: u64) -> bool;
    pub fn oracle_os_sleep_ms(duration: u32);
    pub fn oracle_os_file_exists(path: *const c_char) -> bool;
    pub fn oracle_os_get_abs_path(path: *const c_char, abspath: *mut c_char, size: usize) -> usize;
    pub fn oracle_os_get_abs_path_ptr(path: *const c_char) -> *mut c_char;
    pub fn oracle_os_opendir(path: *const c_char) -> *mut c_void;
    pub fn oracle_os_readdir(dir: *mut c_void) -> *mut OracleDirent;
    pub fn oracle_os_closedir(dir: *mut c_void);
    pub fn oracle_os_glob(
        pattern: *const c_char,
        flags: c_int,
        pglob: *mut *mut OracleGlob,
    ) -> c_int;
    pub fn oracle_os_globfree(pglob: *mut OracleGlob);
    pub fn oracle_os_unlink(path: *const c_char) -> c_int;
    pub fn oracle_os_rmdir(path: *const c_char) -> c_int;
    pub fn oracle_os_mkdir(path: *const c_char) -> c_int;
    pub fn oracle_os_rename(old_path: *const c_char, new_path: *const c_char) -> c_int;
    pub fn oracle_os_safe_replace(
        target: *const c_char,
        from: *const c_char,
        backup: *const c_char,
    ) -> c_int;
    pub fn oracle_os_copyfile(file_in: *const c_char, file_out: *const c_char) -> c_int;
    pub fn oracle_os_getcwd(path: *mut c_char, size: usize) -> *mut c_char;
    pub fn oracle_os_chdir(path: *const c_char) -> c_int;
    pub fn oracle_os_generate_uuid() -> *mut c_char;

    pub fn oracle_os_dirent_size() -> usize;
    pub fn oracle_os_dirent_align() -> usize;
    pub fn oracle_os_dirent_offset_d_name() -> usize;
    pub fn oracle_os_dirent_offset_directory() -> usize;
    pub fn oracle_os_globent_size() -> usize;
    pub fn oracle_os_globent_align() -> usize;
    pub fn oracle_os_globent_offset_path() -> usize;
    pub fn oracle_os_globent_offset_directory() -> usize;
    pub fn oracle_os_glob_info_size() -> usize;
    pub fn oracle_os_glob_info_align() -> usize;
    pub fn oracle_os_glob_info_offset_gl_pathc() -> usize;
    pub fn oracle_os_glob_info_offset_gl_pathv() -> usize;
    pub fn oracle_os_proc_memory_usage_size() -> usize;
    pub fn oracle_os_proc_memory_usage_align() -> usize;
    pub fn oracle_os_proc_memory_usage_offset_resident_size() -> usize;
    pub fn oracle_os_proc_memory_usage_offset_virtual_size() -> usize;
}

#[cfg(not(target_os = "macos"))]
unsafe extern "C" {
    pub fn oracle_os_cpu_usage_info_start() -> *mut c_void;
    pub fn oracle_os_cpu_usage_info_query(info: *mut c_void) -> f64;
    pub fn oracle_os_cpu_usage_info_destroy(info: *mut c_void);
    pub fn oracle_os_gettime_ns() -> u64;
    pub fn oracle_os_get_config_path(dst: *mut c_char, size: usize, name: *const c_char) -> c_int;
    pub fn oracle_os_get_config_path_ptr(name: *const c_char) -> *mut c_char;
    pub fn oracle_os_get_program_data_path(
        dst: *mut c_char,
        size: usize,
        name: *const c_char,
    ) -> c_int;
    pub fn oracle_os_get_program_data_path_ptr(name: *const c_char) -> *mut c_char;
    pub fn oracle_os_get_executable_path_ptr(name: *const c_char) -> *mut c_char;
    pub fn oracle_os_get_emulation_status() -> bool;
    pub fn oracle_os_get_free_space(path: *const c_char) -> i64;
    pub fn oracle_os_request_high_performance(reason: *const c_char) -> *mut c_void;
    pub fn oracle_os_end_high_performance(token: *mut c_void);
    pub fn oracle_os_get_physical_cores() -> c_int;
    pub fn oracle_os_get_logical_cores() -> c_int;
    pub fn oracle_os_get_sys_free_size() -> u64;
    pub fn oracle_os_get_proc_memory_usage(usage: *mut OracleProcMem) -> bool;
    pub fn oracle_os_get_proc_resident_size() -> u64;
    pub fn oracle_os_get_proc_virtual_size() -> u64;
    pub fn oracle_os_get_sys_total_size() -> u64;
    pub fn oracle_os_get_free_disk_space(dir: *const c_char) -> u64;
}

/// `struct os_dirent` as the oracle returns it from `os_readdir`.
#[repr(C)]
pub struct OracleDirent {
    pub d_name: [c_char; 256],
    pub directory: bool,
}

/// `struct os_globent`.
#[repr(C)]
pub struct OracleGlobEnt {
    pub path: *mut c_char,
    pub directory: bool,
}

/// `struct os_glob_info`.
#[repr(C)]
pub struct OracleGlob {
    pub gl_pathc: usize,
    pub gl_pathv: *mut OracleGlobEnt,
}

/// `struct os_proc_memory_usage`.
#[repr(C)]
pub struct OracleProcMem {
    pub resident_size: u64,
    pub virtual_size: u64,
}
