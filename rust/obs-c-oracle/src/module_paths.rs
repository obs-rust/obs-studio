//! `oracle_*` build of `libobs/obs-module-paths.c.h`
//! (oracle/module_paths.c).
//!
//! It searches with the real `os_file_exists`, `os_glob` and `os_globfree`
//! (obs-util's under cargo test), and its paths are `bmalloc`'d by the real
//! `bmalloc`, so the caller `bfree`s them as the shims' paths.

use core::ffi::{c_char, c_int, c_void};

/// `struct obs_module_info2`, as the oracle's callback gets it.
#[repr(C)]
pub struct OracleModuleInfo2 {
    pub bin_path: *const c_char,
    pub data_path: *const c_char,
    pub name: *const c_char,
    pub module_type: c_int,
}

/// `obs_find_module_callback2_t`.
pub type OracleFindModuleCallback2 =
    Option<unsafe extern "C" fn(param: *mut c_void, info: *const OracleModuleInfo2)>;

/// `obs_module_add_extension_t`: takes a `struct dstr *`.
pub type OracleAddExtension = Option<unsafe extern "C" fn(bin_path: *mut c_void)>;

unsafe extern "C" {
    pub fn oracle_obs_module_paths_data_dir(
        module_name: *const c_char,
        data_dir: *const c_char,
    ) -> *mut c_char;
    pub fn oracle_obs_module_paths_find(
        bin: *const c_char,
        data: *const c_char,
        module_type: c_int,
        ext: *const c_char,
        add_extension: OracleAddExtension,
        callback: OracleFindModuleCallback2,
        param: *mut c_void,
    );
    pub fn oracle_obs_module_paths_find_file(
        data_path: *const c_char,
        file: *const c_char,
    ) -> *mut c_char;
    /// 0: size, 1: alignment, 2..5: offsets of `bin_path`, `data_path`,
    /// `name` and `type`; 6: the size of `enum obs_runtime_module_type`.
    pub fn oracle_obs_module_info2_layout(which: c_int) -> usize;
}
