//! Oracle bindings for `libobs/util/dstr.c` (the part the Rust port
//! replaces), plus the C-measured layout of `struct dstr`.

use core::ffi::{c_char, c_int};

/// Independent declaration of `struct dstr`. Intentionally not shared with
/// `obs-util`.
#[repr(C)]
#[derive(Debug)]
pub struct OracleDstr {
    pub array: *mut c_char,
    pub len: usize,
    pub capacity: usize,
}

/// Independent declaration of `struct strref`.
#[repr(C)]
#[derive(Debug)]
pub struct OracleStrref {
    pub array: *const c_char,
    pub len: usize,
}

unsafe extern "C" {
    pub fn oracle_astrcmpi(str1: *const c_char, str2: *const c_char) -> c_int;
    pub fn oracle_astrcmp_n(str1: *const c_char, str2: *const c_char, n: usize) -> c_int;
    pub fn oracle_astrcmpi_n(str1: *const c_char, str2: *const c_char, n: usize) -> c_int;
    pub fn oracle_astrstri(str: *const c_char, find: *const c_char) -> *mut c_char;
    pub fn oracle_strdepad(str: *mut c_char) -> *mut c_char;
    pub fn oracle_strlist_split(
        str: *const c_char,
        split_ch: c_char,
        include_empty: bool,
    ) -> *mut *mut c_char;
    pub fn oracle_strlist_free(strlist: *mut *mut c_char);

    pub fn oracle_dstr_init_copy_strref(dst: *mut OracleDstr, src: *const OracleStrref);
    pub fn oracle_dstr_copy(dst: *mut OracleDstr, array: *const c_char);
    pub fn oracle_dstr_copy_strref(dst: *mut OracleDstr, src: *const OracleStrref);
    pub fn oracle_dstr_ncopy(dst: *mut OracleDstr, array: *const c_char, len: usize);
    pub fn oracle_dstr_ncopy_dstr(dst: *mut OracleDstr, src: *const OracleDstr, len: usize);
    pub fn oracle_dstr_cat_dstr(dst: *mut OracleDstr, str: *const OracleDstr);
    pub fn oracle_dstr_cat_strref(dst: *mut OracleDstr, str: *const OracleStrref);
    pub fn oracle_dstr_ncat(dst: *mut OracleDstr, array: *const c_char, len: usize);
    pub fn oracle_dstr_ncat_dstr(dst: *mut OracleDstr, str: *const OracleDstr, len: usize);
    pub fn oracle_dstr_insert(dst: *mut OracleDstr, idx: usize, array: *const c_char);
    pub fn oracle_dstr_insert_dstr(dst: *mut OracleDstr, idx: usize, str: *const OracleDstr);
    pub fn oracle_dstr_insert_ch(dst: *mut OracleDstr, idx: usize, ch: c_char);
    pub fn oracle_dstr_remove(dst: *mut OracleDstr, idx: usize, count: usize);
    pub fn oracle_dstr_safe_printf(
        dst: *mut OracleDstr,
        format: *const c_char,
        val1: *const c_char,
        val2: *const c_char,
        val3: *const c_char,
        val4: *const c_char,
    );
    pub fn oracle_dstr_replace(str: *mut OracleDstr, find: *const c_char, replace: *const c_char);
    pub fn oracle_dstr_depad(dst: *mut OracleDstr);
    pub fn oracle_dstr_left(dst: *mut OracleDstr, str: *const OracleDstr, pos: usize);
    pub fn oracle_dstr_mid(
        dst: *mut OracleDstr,
        str: *const OracleDstr,
        start: usize,
        count: usize,
    );
    pub fn oracle_dstr_right(dst: *mut OracleDstr, str: *const OracleDstr, pos: usize);

    pub fn oracle_sizeof_dstr() -> usize;
    pub fn oracle_alignof_dstr() -> usize;
    pub fn oracle_offsetof_dstr_array() -> usize;
    pub fn oracle_offsetof_dstr_len() -> usize;
    pub fn oracle_offsetof_dstr_capacity() -> usize;
}
