//! C layout and adapters for cf-parser. The preprocessor remains C.

// c_char is i8 on most targets and u8 on some (aarch64 Linux,
// aarch64-pc-windows-msvc): the `as u8` conversions below are needed on the
// first and no-ops on the second, where clippy calls them unnecessary.
#![allow(clippy::unnecessary_cast)]
use super::cf_tokenizer::{cf_lexer, cf_token};
use super::darray::darray;
use super::lexer::{error_data, error_data_add};
use crate::cf_parser::{self as safe, Cursor};
use core::ffi::{CStr, c_char, c_int};

/// Layout-only mirror of the C preprocessor; Rust does not own its fields.
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct cf_preprocessor {
    pub lex: *mut cf_lexer,
    pub ed: *mut error_data,
    pub defines: darray,
    pub sys_include_dirs: darray,
    pub dependencies: darray,
    pub tokens: darray,
    pub ignore_state: bool,
}

/// Public parser layout from cf-parser.h.
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct cf_parser {
    pub lex: cf_lexer,
    pub pp: cf_preprocessor,
    pub error_list: error_data,
    pub cur_token: *mut cf_token,
}

// Constructed only by cf_pass_pair under its terminated-stream contract.
struct RawCursor(*mut cf_token);
impl Cursor for RawCursor {
    fn token(&self) -> (i32, u8) {
        // SAFETY: the caller supplies a live terminated token array. The
        // terminator's string may be null and is never dereferenced by C.
        unsafe {
            let t = &*self.0;
            (t.r#type, if t.r#type == 0 { 0 } else { *t.str.array as u8 })
        }
    }
    fn advance(&mut self) {
        // SAFETY: the core advances only a non-terminator in the array.
        self.0 = unsafe { self.0.add(1) };
    }
}

/// # Safety
/// `p` and its current token must be valid; tokens form a terminated C
/// lexer/preprocessor stream, with readable token strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cf_pass_pair(p: *mut cf_parser, open: c_char, close: c_char) -> bool {
    // SAFETY: the caller owns a valid parser and token stream.
    unsafe {
        let mut cursor = RawCursor((*p).cur_token);
        let result = safe::pass_pair(&mut cursor, open as u8, close as u8);
        (*p).cur_token = cursor.0;
        result
    }
}

/// # Safety
/// `p`, its current token and token lexer must be live. The unmerged token
/// points into that lexer's NUL-terminated source; message and values are
/// NUL-terminated (message and values may be null). The error list is initialized.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cf_adderror(
    p: *mut cf_parser,
    message: *const c_char,
    level: c_int,
    val1: *const c_char,
    val2: *const c_char,
    val3: *const c_char,
) {
    // SAFETY: all pointers and their relationships follow the C contract.
    unsafe {
        let token = &*(*p).cur_token;
        let lex = &*token.lex;
        let text = CStr::from_ptr(lex.base_lexer.text).to_bytes();
        let position = token.unmerged_str.array.offset_from(lex.base_lexer.text) as usize;
        let values = [val1, val2, val3].map(|v| {
            if v.is_null() {
                None
            } else {
                Some(CStr::from_ptr(v).to_bytes())
            }
        });
        let (row, column, formatted) = safe::nullable_diagnostic(
            text,
            position,
            if message.is_null() {
                None
            } else {
                Some(CStr::from_ptr(message).to_bytes())
            },
            values,
        );
        let formatted = formatted.map(|mut bytes| {
            bytes.push(0);
            bytes
        });
        error_data_add(
            &raw mut (*p).error_list,
            lex.file,
            row,
            column,
            formatted
                .as_ref()
                .map_or(core::ptr::null(), |bytes| bytes.as_ptr().cast()),
            level,
        );
    }
}
