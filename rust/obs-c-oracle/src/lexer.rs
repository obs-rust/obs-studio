//! Oracle bindings for `libobs/util/lexer.c` (`oracle/lexer.c`). Declared
//! as `pub mod lexer;` in `lib.rs` next to the other oracles.
//!
//! The structs are independent declarations of the `lexer.h` structs, so
//! this crate does not depend on `obs-util`.

use core::ffi::{c_char, c_int, c_void};

/// Independent declaration of `struct strref`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OracleStrref {
    pub array: *const c_char,
    pub len: usize,
}

/// Independent declaration of `struct base_token`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OracleBaseToken {
    pub text: OracleStrref,
    pub r#type: c_int,
    pub passed_whitespace: bool,
}

/// Independent declaration of `struct error_item`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OracleErrorItem {
    pub error: *mut c_char,
    pub file: *const c_char,
    pub row: u32,
    pub column: u32,
    pub level: c_int,
}

/// Independent declaration of `struct darray` as embedded in `error_data`.
#[repr(C)]
#[derive(Debug)]
pub struct OracleErrorArray {
    pub array: *mut c_void,
    pub num: usize,
    pub capacity: usize,
}

/// Independent declaration of `struct error_data`.
#[repr(C)]
#[derive(Debug)]
pub struct OracleErrorData {
    pub errors: OracleErrorArray,
}

/// Independent declaration of `struct lexer`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OracleLexer {
    pub text: *mut c_char,
    pub offset: *const c_char,
}

unsafe extern "C" {
    pub fn oracle_strref_cmp(str1: *const OracleStrref, str2: *const c_char) -> c_int;
    pub fn oracle_strref_cmpi(str1: *const OracleStrref, str2: *const c_char) -> c_int;
    pub fn oracle_strref_cmp_strref(str1: *const OracleStrref, str2: *const OracleStrref) -> c_int;
    pub fn oracle_strref_cmpi_strref(str1: *const OracleStrref, str2: *const OracleStrref)
    -> c_int;
    pub fn oracle_valid_int_str(str: *const c_char, n: usize) -> bool;
    pub fn oracle_valid_float_str(str: *const c_char, n: usize) -> bool;
    pub fn oracle_error_data_add(
        data: *mut OracleErrorData,
        file: *const c_char,
        row: u32,
        column: u32,
        msg: *const c_char,
        level: c_int,
    );
    pub fn oracle_error_data_buildstring(ed: *mut OracleErrorData) -> *mut c_char;
    pub fn oracle_lexer_getbasetoken(
        lex: *mut OracleLexer,
        token: *mut OracleBaseToken,
        iws: c_int,
    ) -> bool;
    pub fn oracle_lexer_getstroffset(
        lex: *const OracleLexer,
        str: *const c_char,
        row: *mut u32,
        col: *mut u32,
    );

    pub fn oracle_strref_size() -> usize;
    pub fn oracle_strref_align() -> usize;
    pub fn oracle_strref_offset_array() -> usize;
    pub fn oracle_strref_offset_len() -> usize;

    pub fn oracle_base_token_size() -> usize;
    pub fn oracle_base_token_align() -> usize;
    pub fn oracle_base_token_offset_text() -> usize;
    pub fn oracle_base_token_offset_type() -> usize;
    pub fn oracle_base_token_offset_passed_whitespace() -> usize;

    pub fn oracle_error_item_size() -> usize;
    pub fn oracle_error_item_align() -> usize;
    pub fn oracle_error_item_offset_error() -> usize;
    pub fn oracle_error_item_offset_file() -> usize;
    pub fn oracle_error_item_offset_row() -> usize;
    pub fn oracle_error_item_offset_column() -> usize;
    pub fn oracle_error_item_offset_level() -> usize;

    pub fn oracle_error_data_size() -> usize;
    pub fn oracle_error_data_align() -> usize;
    pub fn oracle_error_data_offset_errors() -> usize;

    pub fn oracle_lexer_size() -> usize;
    pub fn oracle_lexer_align() -> usize;
    pub fn oracle_lexer_offset_text() -> usize;
    pub fn oracle_lexer_offset_offset() -> usize;

    pub fn oracle_base_token_type_size() -> usize;
    pub fn oracle_basetoken_value(which: c_int) -> c_int;
    pub fn oracle_ignore_whitespace_value(ignore: c_int) -> c_int;
}
