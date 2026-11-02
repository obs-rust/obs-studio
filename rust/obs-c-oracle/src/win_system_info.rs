//! `oracle_*` build of `libobs/obs-windows-system-info.c.h`
//! (oracle/win_system_info.c): the system info log rules of
//! `obs-windows.c.h`, renamed. The `bool` results are declared as `u8`, the
//! byte the C returns, so a parity test compares it without assuming it is 0
//! or 1.

use std::ffi::c_char;

unsafe extern "C" {
    /// `uint32_t obs_windows_system_info_version(uint32_t major, uint32_t
    /// minor)`.
    pub fn oracle_obs_windows_system_info_version(major: u32, minor: u32) -> u32;
    /// `uint32_t obs_windows_system_info_mb(uint64_t bytes)`.
    pub fn oracle_obs_windows_system_info_mb(bytes: u64) -> u32;
    /// `bool obs_windows_system_info_gaming_logged(uint32_t win_ver)`.
    pub fn oracle_obs_windows_system_info_gaming_logged(win_ver: u32) -> u8;
    /// `bool obs_windows_system_info_game_mode_retry(int32_t status)`.
    pub fn oracle_obs_windows_system_info_game_mode_retry(status: i32) -> u8;
    /// `bool obs_windows_system_info_gaming_found(int32_t status)`.
    pub fn oracle_obs_windows_system_info_gaming_found(status: i32) -> u8;
    /// `bool obs_windows_system_info_gaming_value_line(int feature, uint32_t
    /// value, const char **label, const char **text)`.
    pub fn oracle_obs_windows_system_info_gaming_value_line(
        feature: i32,
        value: u32,
        label: *mut *const c_char,
        text: *mut *const c_char,
    ) -> u8;
    /// `bool obs_windows_system_info_gaming_fallback(int feature)`.
    pub fn oracle_obs_windows_system_info_gaming_fallback(feature: i32) -> u8;
    /// `bool obs_windows_system_info_gaming_fallback_line(int feature,
    /// uint32_t win_build, const char **label, const char **text)`.
    pub fn oracle_obs_windows_system_info_gaming_fallback_line(
        feature: i32,
        win_build: u32,
        label: *mut *const c_char,
        text: *mut *const c_char,
    ) -> u8;
    /// `const char *obs_windows_system_info_product_state(int state)`.
    pub fn oracle_obs_windows_system_info_product_state(state: i32) -> *const c_char;
    /// `const char *obs_windows_system_info_product_type(int type)`.
    pub fn oracle_obs_windows_system_info_product_type(kind: i32) -> *const c_char;
}
