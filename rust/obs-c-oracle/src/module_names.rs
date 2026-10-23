//! `oracle_*` build of `libobs/obs-module-names.c.h`
//! (oracle/module_names.c).
//!
//! It looks with the real `os_file_exists` (obs-util's under cargo test),
//! makes data paths with the oracle's own `obs_module_paths_data_dir`, and
//! its strings are `bmalloc`'d by the real `bmalloc`, so the caller
//! `bfree`s them as the shims' strings.

use core::ffi::{c_char, c_int, c_void};

use crate::module_paths::OracleFindModuleCallback2;

unsafe extern "C" {
    pub fn oracle_obs_module_names_file_name(path: *const c_char) -> *const c_char;
    pub fn oracle_obs_module_names_module_name(
        file: *const c_char,
        ext: *const c_char,
    ) -> *mut c_char;
    pub fn oracle_obs_module_names_config_path(
        config_path: *const c_char,
        module_name: *const c_char,
        file: *const c_char,
    ) -> *mut c_char;
    pub fn oracle_obs_module_names_manifest_path(data_path: *const c_char) -> *mut c_char;
    pub fn oracle_obs_module_names_listed(
        names: *const *mut c_char,
        count: usize,
        name: *const c_char,
    ) -> bool;
    pub fn oracle_obs_module_names_find_core(
        binary: *const c_char,
        name: *const c_char,
        data: *const c_char,
        module_type: c_int,
        ext: *const c_char,
        callback: OracleFindModuleCallback2,
        param: *mut c_void,
    ) -> bool;
}
