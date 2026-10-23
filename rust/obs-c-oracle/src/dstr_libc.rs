//! Oracle bindings for `libobs/util/dstr-libc.c` (`oracle/dstr_libc.c`,
//! the original under `oracle_*` names) and for its printf family as the
//! hybrid libobs compiles it, `libobs/util/dstr-variadic.c`
//! (`oracle/dstr_variadic.c`, under `variadic_*` names).

use core::ffi::{c_char, c_int};

use crate::dstr::OracleDstr;

/// `wchar_t`.
#[cfg(windows)]
pub type OracleWChar = u16;
/// `wchar_t`.
#[cfg(not(windows))]
pub type OracleWChar = i32;

unsafe extern "C" {
    pub fn oracle_wstrcmpi(str1: *const OracleWChar, str2: *const OracleWChar) -> c_int;
    pub fn oracle_wstrcmp_n(str1: *const OracleWChar, str2: *const OracleWChar, n: usize) -> c_int;
    pub fn oracle_wstrcmpi_n(str1: *const OracleWChar, str2: *const OracleWChar, n: usize)
    -> c_int;
    pub fn oracle_wstrstri(str: *const OracleWChar, find: *const OracleWChar) -> *mut OracleWChar;
    pub fn oracle_wcsdepad(str: *mut OracleWChar) -> *mut OracleWChar;

    pub fn oracle_dstr_from_mbs(dst: *mut OracleDstr, mbstr: *const c_char);
    pub fn oracle_dstr_to_mbs(str: *const OracleDstr) -> *mut c_char;
    pub fn oracle_dstr_to_wcs(str: *const OracleDstr) -> *mut OracleWChar;
    pub fn oracle_dstr_from_wcs(dst: *mut OracleDstr, wstr: *const OracleWChar);
    pub fn oracle_dstr_to_upper(str: *mut OracleDstr);
    pub fn oracle_dstr_to_lower(str: *mut OracleDstr);

    pub fn oracle_dstr_printf(dst: *mut OracleDstr, format: *const c_char, ...);
    pub fn oracle_dstr_catf(dst: *mut OracleDstr, format: *const c_char, ...);

    pub fn variadic_dstr_printf(dst: *mut OracleDstr, format: *const c_char, ...);
    pub fn variadic_dstr_catf(dst: *mut OracleDstr, format: *const c_char, ...);
}
