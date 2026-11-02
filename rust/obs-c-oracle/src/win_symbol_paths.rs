//! `oracle_*` build of `libobs/obs-windows-symbol-paths.c.h`
//! (oracle/win_symbol_paths.c), the debug symbol search path rules of
//! `obs-windows.c.h`, renamed, on the C dstr oracle. The `bool` results are
//! declared as `u8`, the byte the C returns, so a parity test compares it
//! without assuming it is 0 or 1.

use core::ffi::c_char;

use crate::dstr::OracleDstr;

unsafe extern "C" {
    /// `bool obs_windows_symbol_paths_dir(const char *bin_path, struct dstr
    /// *dir)`.
    pub fn oracle_obs_windows_symbol_paths_dir(bin_path: *const c_char, dir: *mut OracleDstr)
    -> u8;
    /// `bool obs_windows_symbol_paths_listed(char *const *paths, size_t num,
    /// const char *abspath)`.
    pub fn oracle_obs_windows_symbol_paths_listed(
        paths: *const *mut c_char,
        num: usize,
        abspath: *const c_char,
    ) -> u8;
    /// `void obs_windows_symbol_paths_join(char *const *paths, size_t num,
    /// struct dstr *out)`.
    pub fn oracle_obs_windows_symbol_paths_join(
        paths: *const *mut c_char,
        num: usize,
        out: *mut OracleDstr,
    );
}
