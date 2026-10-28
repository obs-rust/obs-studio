//! `oracle_*` build of `libobs/obs-nix-system-info.c.h`
//! (oracle/nix_system_info.c): the system info parsing of `obs-nix.c.h`,
//! renamed. The `bool` results are declared as `u8`, the byte the C returns,
//! so a parity test compares it without assuming it is 0 or 1; a returned
//! position is a pointer into the line passed in.

use std::os::raw::{c_char, c_int};

unsafe extern "C" {
    /// `bool obs_nix_system_info_is_name(const char *line)`.
    pub fn oracle_obs_nix_system_info_is_name(line: *const c_char) -> u8;
    /// `bool obs_nix_system_info_is_physical_id(const char *line)`.
    pub fn oracle_obs_nix_system_info_is_physical_id(line: *const c_char) -> u8;
    /// `bool obs_nix_system_info_is_speed(const char *line)`.
    pub fn oracle_obs_nix_system_info_is_speed(line: *const c_char) -> u8;
    /// `const char *obs_nix_system_info_value(const char *line)`.
    pub fn oracle_obs_nix_system_info_value(line: *const c_char) -> *const c_char;
    /// `int obs_nix_system_info_physical_id(const char *value)`.
    pub fn oracle_obs_nix_system_info_physical_id(value: *const c_char) -> c_int;
    /// `bool obs_nix_system_info_prints(const char *line, int physical_id,
    /// int last_physical_id)`.
    pub fn oracle_obs_nix_system_info_prints(
        line: *const c_char,
        physical_id: c_int,
        last_physical_id: c_int,
    ) -> u8;
    /// `bool obs_nix_system_info_is_dmesg_cpu(const char *line)`.
    pub fn oracle_obs_nix_system_info_is_dmesg_cpu(line: *const c_char) -> u8;
    /// `const char *obs_nix_system_info_dmesg_speed(const char *line, size_t
    /// *len)`.
    pub fn oracle_obs_nix_system_info_dmesg_speed(
        line: *const c_char,
        len: *mut usize,
    ) -> *const c_char;
    /// `uint64_t obs_nix_system_info_total_mb(uint64_t totalram, uint32_t
    /// mem_unit)`.
    pub fn oracle_obs_nix_system_info_total_mb(totalram: u64, mem_unit: u32) -> u64;
    /// `uint64_t obs_nix_system_info_free_mb(uint64_t freeram, uint64_t
    /// bufferram, uint32_t mem_unit)`.
    pub fn oracle_obs_nix_system_info_free_mb(freeram: u64, bufferram: u64, mem_unit: u32) -> u64;
    /// `int64_t obs_nix_system_info_physmem_mb(int64_t mem)`.
    pub fn oracle_obs_nix_system_info_physmem_mb(mem: i64) -> i64;
}
