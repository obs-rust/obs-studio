//! C ABI shim for `libobs/util/lexer.h` (the `lexer.c` exports only).
//!
//! The header is the source of truth: the structs below keep its exact
//! layout, and its `static inline` helpers (`lexer_init`, `strref_set`,
//! `error_data_free`, ...) stay C and work on these structs directly. The
//! functions only convert to and from the safe core in [`crate::lexer`].

use core::ffi::{CStr, c_char, c_int};
use core::{mem, ptr, slice};

use super::darray::{bmalloc, darray, darray_ensure_capacity};
use crate::lexer::{self as core_lexer, ErrorItem, IgnoreWhitespace};

/// Mirrors `struct strref`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub struct strref {
    pub array: *const c_char,
    pub len: usize,
}

/// Mirrors `struct base_token`; `type` is an `enum base_token_type`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub struct base_token {
    pub text: strref,
    pub r#type: c_int,
    pub passed_whitespace: bool,
}

/// Mirrors `struct error_item`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub struct error_item {
    pub error: *mut c_char,
    pub file: *const c_char,
    pub row: u32,
    pub column: u32,
    pub level: c_int,
}

/// Mirrors `struct error_data` (`DARRAY(struct error_item) errors`).
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct error_data {
    pub errors: darray,
}

/// Mirrors `struct lexer`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub struct lexer {
    pub text: *mut c_char,
    pub offset: *const c_char,
}

/// The `strref` as a core slice (`None` for a NULL pointer or array).
///
/// # Safety
///
/// `s` must be null or point to a valid `strref` whose `array` is valid for
/// reads of `len` bytes.
unsafe fn strref_bytes<'a>(s: *const strref) -> Option<&'a [u8]> {
    if s.is_null() {
        return None;
    }
    // SAFETY: non-null and valid per the contract.
    let s = unsafe { &*s };
    if s.array.is_null() {
        return None;
    }
    // SAFETY: the caller guarantees `array` is readable for `len` bytes.
    Some(unsafe { slice::from_raw_parts(s.array.cast::<u8>(), s.len) })
}

/// A C string as bytes without the NUL (`None` for NULL).
///
/// # Safety
///
/// `p` must be null or point to a NUL-terminated string.
unsafe fn c_bytes<'a>(p: *const c_char) -> Option<&'a [u8]> {
    if p.is_null() {
        None
    } else {
        // SAFETY: non-null and NUL-terminated per the contract.
        Some(unsafe { CStr::from_ptr(p) }.to_bytes())
    }
}

/// Like [`c_bytes`] but reads at most `limit` bytes, so a string with a
/// small `n` is never scanned further than the C loop would read.
///
/// # Safety
///
/// `p` must be null or valid for reads up to its NUL or `limit` bytes.
unsafe fn c_bytes_limited<'a>(p: *const c_char, limit: usize) -> Option<&'a [u8]> {
    if p.is_null() {
        return None;
    }
    let mut len = 0;
    // SAFETY: reads stop at the first NUL or at `limit`.
    while len < limit && unsafe { *p.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: the `len` bytes just read are valid.
    Some(unsafe { slice::from_raw_parts(p.cast::<u8>(), len) })
}

/// # Safety
///
/// `str1` must be null or a valid `strref`; `str2` null or a C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strref_cmp(str1: *const strref, str2: *const c_char) -> c_int {
    // SAFETY: forwarded from the caller.
    unsafe { core_lexer::strref_cmp(strref_bytes(str1), c_bytes(str2)) }
}

/// # Safety
///
/// See [`strref_cmp`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strref_cmpi(str1: *const strref, str2: *const c_char) -> c_int {
    // SAFETY: forwarded from the caller.
    unsafe { core_lexer::strref_cmpi(strref_bytes(str1), c_bytes(str2)) }
}

/// # Safety
///
/// Both pointers must be null or valid `strref`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strref_cmp_strref(str1: *const strref, str2: *const strref) -> c_int {
    // SAFETY: forwarded from the caller.
    unsafe { core_lexer::strref_cmp_strref(strref_bytes(str1), strref_bytes(str2)) }
}

/// # Safety
///
/// See [`strref_cmp_strref`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strref_cmpi_strref(str1: *const strref, str2: *const strref) -> c_int {
    // SAFETY: forwarded from the caller.
    unsafe { core_lexer::strref_cmpi_strref(strref_bytes(str1), strref_bytes(str2)) }
}

/// How many bytes the C validators may read: `n == 0` means `strlen`, so the
/// whole string; otherwise at most the sign plus `n` chars plus the NUL check.
fn validator_limit(n: usize) -> usize {
    if n == 0 {
        usize::MAX
    } else {
        n.saturating_add(2)
    }
}

/// # Safety
///
/// `str` must be null or a string readable up to its NUL (or `n + 2` bytes
/// when `n != 0`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn valid_int_str(str: *const c_char, n: usize) -> bool {
    // SAFETY: forwarded from the caller; the core reads at most `n + 1`
    // bytes past the first, as the C loop does.
    unsafe { core_lexer::valid_int_str(c_bytes_limited(str, validator_limit(n)), n) }
}

/// # Safety
///
/// See [`valid_int_str`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn valid_float_str(str: *const c_char, n: usize) -> bool {
    // SAFETY: as for `valid_int_str`.
    unsafe { core_lexer::valid_float_str(c_bytes_limited(str, validator_limit(n)), n) }
}

/// `bstrdup` from `dstr.c`, kept local so this shim does not depend on how
/// `dstr.c` is built.
///
/// # Safety
///
/// `s` must be null or a C string.
unsafe fn bstrdup(s: *const c_char) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: `s` is a C string; the copy includes the NUL.
    unsafe {
        let bytes = CStr::from_ptr(s).to_bytes_with_nul();
        let p = bmalloc(bytes.len()).cast::<c_char>();
        ptr::copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), p, bytes.len());
        p
    }
}

/// # Safety
///
/// `data` must be null or a valid `error_data` (as set up by
/// `error_data_init`); `file` and `msg` null or C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn error_data_add(
    data: *mut error_data,
    file: *const c_char,
    row: u32,
    column: u32,
    msg: *const c_char,
    level: c_int,
) {
    if data.is_null() {
        return;
    }
    // SAFETY: `data` is valid per the contract; this is `da_push_back`.
    unsafe {
        let item = error_item {
            error: bstrdup(msg),
            file,
            row,
            column,
            level,
        };
        let errors = &raw mut (*data).errors;
        (*errors).num += 1;
        darray_ensure_capacity(mem::size_of::<error_item>(), errors, (*errors).num);
        (*errors)
            .array
            .cast::<error_item>()
            .add((*errors).num - 1)
            .write(item);
    }
}

/// # Safety
///
/// `ed` must point to a valid `error_data` whose items hold valid strings.
/// The result is `bmalloc`ed (NULL when there are no items): free with `bfree`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn error_data_buildstring(ed: *mut error_data) -> *mut c_char {
    // SAFETY: `ed` is valid per the contract.
    let (array, num) = unsafe { ((*ed).errors.array, (*ed).errors.num) };
    if array.is_null() || num == 0 {
        return ptr::null_mut();
    }
    // SAFETY: the array holds `num` items.
    let items = unsafe { slice::from_raw_parts(array.cast::<error_item>(), num) };
    let core_items: Vec<ErrorItem<'_>> = items
        .iter()
        .map(|it| ErrorItem {
            // SAFETY: item strings are valid C strings or NULL.
            file: unsafe { c_bytes(it.file) },
            row: it.row,
            column: it.column,
            // SAFETY: as above.
            error: unsafe { c_bytes(it.error) },
        })
        .collect();
    let Some(out) = core_lexer::build_error_string(&core_items) else {
        return ptr::null_mut();
    };
    // SAFETY: plain allocation of `len + 1` (> 0) bytes, then a copy.
    unsafe {
        let p = bmalloc(out.len() + 1).cast::<u8>();
        ptr::copy_nonoverlapping(out.as_ptr(), p, out.len());
        *p.add(out.len()) = 0;
        p.cast::<c_char>()
    }
}

/// # Safety
///
/// `lex` and `token` must be valid; `lex.offset` null or a C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lexer_getbasetoken(
    lex: *mut lexer,
    token: *mut base_token,
    iws: c_int,
) -> bool {
    // SAFETY: valid per the contract.
    let (lex, token) = unsafe { (&mut *lex, &mut *token) };
    let base = lex.offset;
    if base.is_null() {
        return false;
    }
    let at = |i: usize| -> u8 {
        // SAFETY: the scan never reads past the NUL terminator (see
        // `scan_base_token`), so every index asked for is in the string.
        unsafe { *base.add(i).cast::<u8>() }
    };
    let (moved, found) = core_lexer::scan_base_token(at, IgnoreWhitespace::from_c(iws));
    // SAFETY: `moved` stays within the string.
    lex.offset = unsafe { base.add(moved) };
    match found {
        Some(t) => {
            token.text = strref {
                // SAFETY: the token lies within the string.
                array: unsafe { base.add(t.start) },
                len: t.len,
            };
            token.r#type = t.kind as c_int;
            true
        }
        None => false,
    }
}

/// # Safety
///
/// `lex` must be valid; `str` null or a pointer into (or just past) `lex.text`;
/// `row` and `col` writable when `str` is non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lexer_getstroffset(
    lex: *const lexer,
    str: *const c_char,
    row: *mut u32,
    col: *mut u32,
) {
    if str.is_null() {
        return;
    }
    // SAFETY: `lex` is valid per the contract.
    let text = unsafe { (*lex).text };
    // A pointer before the text never enters the C loop: (1, 1).
    let pos = (str as usize).saturating_sub(text as usize);
    let at = |i: usize| -> u8 {
        // SAFETY: only indices below `pos` (plus one lookahead after a
        // newline) are read, all inside the text as in C.
        unsafe { *text.add(i).cast::<u8>() }
    };
    let (r, c) = core_lexer::str_offset(at, pos);
    // SAFETY: writable per the contract.
    unsafe {
        *row = r;
        *col = c;
    }
}
