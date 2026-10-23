//! `oracle_*` build of `libobs/obs-module-source-rules.c.h`
//! (oracle/module_source_rules.c).
//!
//! The enums are 32-bit integers, taken and returned as `u32` as the shims
//! do. The versioned id is printed by the original `dstr_printf` (the dstr
//! oracle's) and `bmalloc`'d by the real `bmalloc`, so the caller `bfree`s
//! it as the shims' ids.

use core::ffi::c_char;

unsafe extern "C" {
    pub fn oracle_obs_module_source_rules_list_of(source_type: u32) -> u32;
    pub fn oracle_obs_module_source_rules_size_supported(size: usize, supported: usize) -> bool;
    pub fn oracle_obs_module_source_rules_output_flags(source_type: u32, output_flags: u32) -> u32;
    pub fn oracle_obs_module_source_rules_check(
        source_type: u32,
        info_flags: u32,
        flags: u32,
        callbacks: u32,
    ) -> u32;
    pub fn oracle_obs_module_source_rules_versioned_id(
        id: *const c_char,
        version: u32,
    ) -> *mut c_char;
}
