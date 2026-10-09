//! Port of `libobs/util/lexer.c` (not `cf-lexer.c` / `cf-parser.c`).
//!
//! Safe core on byte slices. The C original works on NUL-terminated strings
//! and `struct strref` segments; here a C string is a `&[u8]` that stops at
//! its first NUL, and a `strref` is `Option<&[u8]>` (`None` is a NULL
//! `array`; the slice is already cut to `len`). The `#[repr(C)]` structs and
//! the exported symbols live in [`crate::ffi::lexer`].
//!
//! Quirks of the C code are kept on purpose (characterized, not endorsed):
//! - comparisons use the platform `char` ordering (signed on x86), so bytes
//!   >= 0x80 sort below ASCII where `char` is signed;
//! - `valid_float_str` rejects `1e-3` but accepts `1e3-`.

use core::cmp::Ordering;
use core::ffi::c_char;

/// Cuts `bytes` at its first NUL (C string semantics).
fn c_str(bytes: &[u8]) -> &[u8] {
    match bytes.iter().position(|&b| b == 0) {
        Some(n) => &bytes[..n],
        None => bytes,
    }
}

/// C `char` view of a byte, so ordering follows the platform's `char`.
fn ch(b: u8) -> c_char {
    b as c_char
}

/// C `toupper` in the "C" locale, applied to a `char`.
fn upper(b: u8) -> u8 {
    b.to_ascii_uppercase()
}

fn byte(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

/// `strref_is_empty`: NULL, zero length, or a leading NUL.
pub fn strref_is_empty(s: Option<&[u8]>) -> bool {
    s.is_none_or(|s| s.first().is_none_or(|&b| b == 0))
}

fn sign(ord: Ordering) -> i32 {
    match ord {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Shared body of `strref_cmp` / `strref_cmpi`.
fn cmp_c(str1: Option<&[u8]>, str2: Option<&[u8]>, fold: bool) -> i32 {
    let str2 = str2.map(c_str);
    if strref_is_empty(str1) {
        return if str2.is_none_or(<[u8]>::is_empty) {
            0
        } else {
            -1
        };
    }
    let a = str1.unwrap_or(&[]);
    let b = str2.unwrap_or(&[]);
    let mut i = 0;
    loop {
        let (mut c1, mut c2) = (byte(a, i), byte(b, i));
        if fold {
            c1 = upper(c1);
            c2 = upper(c2);
        }
        let ord = ch(c1).cmp(&ch(c2));
        if ord != Ordering::Equal {
            return sign(ord);
        }
        // `i++ < str1->len && *str2++`
        if !(i < a.len() && i < b.len()) {
            return 0;
        }
        i += 1;
    }
}

/// Shared body of `strref_cmp_strref` / `strref_cmpi_strref`. An empty
/// strref sorts before any non-empty one in both argument orders (the C fix
/// for #48, PR #60).
fn cmp_strref_c(str1: Option<&[u8]>, str2: Option<&[u8]>, fold: bool) -> i32 {
    if strref_is_empty(str1) {
        return if strref_is_empty(str2) { 0 } else { -1 };
    }
    if strref_is_empty(str2) {
        return 1;
    }
    let a = str1.unwrap_or(&[]);
    let b = str2.unwrap_or(&[]);
    let mut i = 0;
    loop {
        let (mut c1, mut c2) = (byte(a, i), byte(b, i));
        if fold {
            c1 = upper(c1);
            c2 = upper(c2);
        }
        let ord = ch(c1).cmp(&ch(c2));
        if ord != Ordering::Equal {
            return sign(ord);
        }
        i += 1;
        if !(i <= a.len() && i <= b.len()) {
            return 0;
        }
    }
}

/// `strref_cmp`: `str2` is a C string (`None` is NULL).
pub fn strref_cmp(str1: Option<&[u8]>, str2: Option<&[u8]>) -> i32 {
    cmp_c(str1, str2, false)
}

/// `strref_cmpi`: like [`strref_cmp`], ASCII case-insensitive.
pub fn strref_cmpi(str1: Option<&[u8]>, str2: Option<&[u8]>) -> i32 {
    cmp_c(str1, str2, true)
}

/// `strref_cmp_strref`.
pub fn strref_cmp_strref(str1: Option<&[u8]>, str2: Option<&[u8]>) -> i32 {
    cmp_strref_c(str1, str2, false)
}

/// `strref_cmpi_strref`.
pub fn strref_cmpi_strref(str1: Option<&[u8]>, str2: Option<&[u8]>) -> i32 {
    cmp_strref_c(str1, str2, true)
}

/// `valid_int_str`: `s` is a C string (`None` is NULL); `n == 0` means the
/// whole string. `n` is not consumed by a leading sign.
pub fn valid_int_str(s: Option<&[u8]>, n: usize) -> bool {
    let Some(s) = s.map(c_str) else {
        return false;
    };
    if s.is_empty() {
        return false;
    }
    let mut n = if n == 0 { s.len() } else { n };
    let mut i = usize::from(matches!(s[0], b'-' | b'+'));
    // C sets `found_num` before every loop test, so reaching the end of the
    // `do { } while` means at least one digit was seen.
    loop {
        if !byte(s, i).is_ascii_digit() {
            return false;
        }
        i += 1;
        // `*++str && --n`
        if i >= s.len() {
            break;
        }
        n -= 1;
        if n == 0 {
            break;
        }
    }
    true
}

/// `valid_float_str`: see [`valid_int_str`] for `s` and `n`.
pub fn valid_float_str(s: Option<&[u8]>, n: usize) -> bool {
    let Some(s) = s.map(c_str) else {
        return false;
    };
    if s.is_empty() {
        return false;
    }
    let mut n = if n == 0 { s.len() } else { n };
    let mut i = usize::from(matches!(s[0], b'-' | b'+'));
    let mut found_num = false;
    let mut found_exp = false;
    let mut found_dec = false;
    loop {
        match byte(s, i) {
            b'.' => {
                if found_dec || found_exp || !found_num {
                    return false;
                }
                found_dec = true;
            }
            b'e' => {
                if found_exp || !found_num {
                    return false;
                }
                found_exp = true;
                found_num = false;
            }
            b'-' | b'+' => {
                if !found_exp || !found_num {
                    return false;
                }
            }
            c if c.is_ascii_digit() => found_num = true,
            _ => return false,
        }
        i += 1;
        if i >= s.len() {
            break;
        }
        n -= 1;
        if n == 0 {
            break;
        }
    }
    found_num
}

/// `is_whitespace` from `lexer.h`.
pub fn is_whitespace(c: u8) -> bool {
    matches!(c, b' ' | b'\r' | b'\t' | b'\n')
}

/// `is_newline` from `lexer.h`.
pub fn is_newline(c: u8) -> bool {
    matches!(c, b'\r' | b'\n')
}

/// `is_newline_pair` from `lexer.h`.
pub fn is_newline_pair(c1: u8, c2: u8) -> bool {
    (c1 == b'\r' && c2 == b'\n') || (c1 == b'\n' && c2 == b'\r')
}

/// `enum base_token_type`; discriminants match the C enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum BaseTokenType {
    None = 0,
    Alpha = 1,
    Digit = 2,
    Whitespace = 3,
    Other = 4,
}

/// `enum ignore_whitespace`; discriminants match the C enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum IgnoreWhitespace {
    Parse = 0,
    Ignore = 1,
}

impl IgnoreWhitespace {
    /// C compares `iws == IGNORE_WHITESPACE`, so any other value parses.
    pub fn from_c(v: i32) -> Self {
        if v == IgnoreWhitespace::Ignore as i32 {
            Self::Ignore
        } else {
            Self::Parse
        }
    }
}

/// `get_char_token_type`.
pub fn get_char_token_type(c: u8) -> BaseTokenType {
    if is_whitespace(c) {
        BaseTokenType::Whitespace
    } else if c.is_ascii_digit() {
        BaseTokenType::Digit
    } else if c.is_ascii_alphabetic() {
        BaseTokenType::Alpha
    } else {
        BaseTokenType::Other
    }
}

/// A token found by [`scan_base_token`], relative to the scan start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaseToken {
    pub start: usize,
    pub len: usize,
    pub kind: BaseTokenType,
}

/// Core of `lexer_getbasetoken` over an accessor `at(i)` that returns the
/// byte `i` positions after the cursor, `0` at the end of the string. It
/// never asks for a position past the first `0` it has seen (plus none),
/// except the single lookahead after a newline, as in C.
///
/// Returns how far the cursor moved and the token, if any. The cursor moves
/// even when no token is found.
pub fn scan_base_token(
    at: impl Fn(usize) -> u8,
    iws: IgnoreWhitespace,
) -> (usize, Option<BaseToken>) {
    let ignore_whitespace = iws == IgnoreWhitespace::Ignore;
    let mut offset = 0;
    let mut token_start = None;
    let mut ty = BaseTokenType::None;

    while at(offset) != 0 {
        let c = at(offset);
        offset += 1;
        let new_type = get_char_token_type(c);

        if ty == BaseTokenType::None {
            if new_type == BaseTokenType::Whitespace && ignore_whitespace {
                continue;
            }
            token_start = Some(offset - 1);
            ty = new_type;

            if ty != BaseTokenType::Digit && ty != BaseTokenType::Alpha {
                if is_newline(c) && is_newline_pair(c, at(offset)) {
                    offset += 1;
                }
                break;
            }
        } else if ty != new_type {
            offset -= 1;
            break;
        }
    }

    match token_start {
        Some(start) if offset > start => (
            offset,
            Some(BaseToken {
                start,
                len: offset - start,
                kind: ty,
            }),
        ),
        _ => (offset, None),
    }
}

/// Cursor over a text, as `struct lexer` with `offset` an index.
#[derive(Debug, Clone)]
pub struct Lexer<'a> {
    text: &'a [u8],
    offset: usize,
}

impl<'a> Lexer<'a> {
    /// `lexer_start` (the text ends at its first NUL).
    pub fn new(text: &'a [u8]) -> Self {
        Self {
            text: c_str(text),
            offset: 0,
        }
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    /// `lexer_reset`.
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    /// `lexer_getbasetoken`; the token's `start` is an index into the text.
    pub fn get_base_token(&mut self, iws: IgnoreWhitespace) -> Option<BaseToken> {
        let (text, base) = (self.text, self.offset);
        let (moved, token) = scan_base_token(|i| byte(text, base + i), iws);
        self.offset = base + moved;
        token.map(|t| BaseToken {
            start: base + t.start,
            ..t
        })
    }

    /// `lexer_getstroffset` for the character at index `pos`.
    pub fn str_offset(&self, pos: usize) -> (u32, u32) {
        str_offset_in(self.text, pos)
    }
}

/// `newline_size` over an accessor.
fn newline_size(at: &impl Fn(usize) -> u8, i: usize) -> usize {
    let (a, b) = (at(i), at(i + 1));
    if a != b'\r' && a != b'\n' {
        0
    } else if is_newline_pair(a, b) {
        2
    } else {
        1
    }
}

/// Core of `lexer_getstroffset`: 1-based `(row, column)` of the character at
/// index `pos`, reading bytes through `at(i)`. A CRLF or LFCR pair counts as
/// one newline, and `pos` pointing at the second byte of a pair counts the
/// whole pair as already passed (the C loop steps over it).
pub fn str_offset(at: impl Fn(usize) -> u8, pos: usize) -> (u32, u32) {
    let (mut row, mut col) = (1u32, 1u32);
    let mut i = 0;
    while i < pos {
        if is_newline(at(i)) {
            i += newline_size(&at, i) - 1;
            col = 1;
            row = row.wrapping_add(1);
        } else {
            col = col.wrapping_add(1);
        }
        i += 1;
    }
    (row, col)
}

/// [`str_offset`] over a slice (bytes past its end read as NUL).
pub fn str_offset_in(text: &[u8], pos: usize) -> (u32, u32) {
    str_offset(|i| byte(text, i), pos)
}

/// One entry for [`build_error_string`]; `None` is a NULL pointer.
#[derive(Debug, Clone, Copy)]
pub struct ErrorItem<'a> {
    pub file: Option<&'a [u8]>,
    pub row: u32,
    pub column: u32,
    pub error: Option<&'a [u8]>,
}

/// `error_data_buildstring`: one `"%s (%u, %u): %s\n"` line per item. `None`
/// (C returns NULL) when there are no items. A NULL string prints as
/// `(null)`, as the C library's `%s` does.
pub fn build_error_string(items: &[ErrorItem<'_>]) -> Option<Vec<u8>> {
    if items.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for item in items {
        out.extend_from_slice(item.file.map_or(&b"(null)"[..], c_str));
        out.extend_from_slice(format!(" ({}, {}): ", item.row, item.column).as_bytes());
        out.extend_from_slice(item.error.map_or(&b"(null)"[..], c_str));
        out.push(b'\n');
    }
    Some(out)
}
