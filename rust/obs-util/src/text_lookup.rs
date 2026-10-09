//! Safe core for `libobs/util/text-lookup.c`.
//!
//! Parses INI-like `Key="value"` data exactly like `lookup_addfiledata`,
//! `lookup_gettoken`, `lookup_getstringtoken` and `lookup_goto_nextline`,
//! including the base-token rules of `lexer_getbasetoken` they rely on. The
//! lexer is reproduced here on purpose so this module does not depend on the
//! lexer port.
//!
//! Intentional difference from C: when the opening quote of a token is the
//! last character on its line (`Key="` directly followed by a newline or the
//! end of the data), the C code computes `token->len--` twice on a length of
//! one and wraps to `SIZE_MAX`, then crashes. Rust treats the token as an
//! empty string instead. As with `Key=""` in C, an empty token ends the
//! parse of the rest of the file (entries already parsed are kept).

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::path::Path;

/// Mirrors `enum base_token_type` (without `NONE`, which is `Option::None`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum BaseTokenType {
    Alpha,
    Digit,
    Whitespace,
    Other,
}

fn is_whitespace(ch: u8) -> bool {
    matches!(ch, b' ' | b'\r' | b'\t' | b'\n')
}

fn is_newline(ch: u8) -> bool {
    ch == b'\r' || ch == b'\n'
}

fn is_newline_pair(ch1: u8, ch2: u8) -> bool {
    (ch1 == b'\r' && ch2 == b'\n') || (ch1 == b'\n' && ch2 == b'\r')
}

fn char_token_type(ch: u8) -> BaseTokenType {
    if is_whitespace(ch) {
        BaseTokenType::Whitespace
    } else if ch.is_ascii_digit() {
        BaseTokenType::Digit
    } else if ch.is_ascii_alphabetic() {
        BaseTokenType::Alpha
    } else {
        BaseTokenType::Other
    }
}

/// A `struct strref`: `start` is `None` for a NULL array.
#[derive(Clone, Copy, Default)]
struct StrRef {
    start: Option<usize>,
    len: usize,
}

/// Replacement for `struct lexer` over NUL-free data.
struct Lexer<'a> {
    data: &'a [u8],
    offset: usize,
}

impl Lexer<'_> {
    /// Byte at `i`, or 0 past the end (the C string terminator).
    fn at(&self, i: usize) -> u8 {
        self.data.get(i).copied().unwrap_or(0)
    }

    /// `lexer_getbasetoken(lex, &token, PARSE_WHITESPACE)`.
    fn get_base_token(&mut self) -> Option<(usize, usize, BaseTokenType)> {
        let mut offset = self.offset;
        let mut token_start = None;
        let mut ty: Option<BaseTokenType> = None;

        while self.at(offset) != 0 {
            let ch = self.at(offset);
            offset += 1;
            let new_type = char_token_type(ch);

            match ty {
                None => {
                    token_start = Some(offset - 1);
                    ty = Some(new_type);

                    if new_type != BaseTokenType::Digit && new_type != BaseTokenType::Alpha {
                        if is_newline(ch) && is_newline_pair(ch, self.at(offset)) {
                            offset += 1;
                        }
                        break;
                    }
                }
                Some(t) if t != new_type => {
                    offset -= 1;
                    break;
                }
                Some(_) => {}
            }
        }

        self.offset = offset;

        match (token_start, ty) {
            (Some(start), Some(t)) if offset > start => Some((start, offset - start, t)),
            _ => None,
        }
    }

    /// `lookup_getstringtoken`.
    fn get_string_token(&mut self, token: &mut StrRef) {
        let offset = self.offset;
        let mut temp = offset;
        let mut was_backslash = false;

        while self.at(temp) != 0 && self.at(temp) != b'\n' {
            if !was_backslash {
                if self.at(temp) == b'\\' {
                    was_backslash = true;
                } else if self.at(temp) == b'"' {
                    temp += 1;
                    break;
                }
            } else {
                was_backslash = false;
            }

            temp += 1;
        }

        token.len += temp - offset;

        if let Some(start) = token.start
            && self.at(start) == b'"'
        {
            token.start = Some(start + 1);
            token.len -= 1;

            // C reads `temp[-1]` unconditionally; with `temp == offset` that
            // is the opening quote itself and the length wraps. Guarded here.
            if temp > offset && self.at(temp - 1) == b'"' {
                token.len -= 1;
            }
        }

        self.offset = temp;
    }

    /// `lookup_gettoken`.
    fn get_token(&mut self, out: &mut StrRef) -> bool {
        *out = StrRef::default();

        while let Some((start, len, ty)) = self.get_base_token() {
            let ch = self.at(start);

            if out.start.is_none() {
                /* comments are designated with a #, and end at LF */
                if ch == b'#' {
                    while self.at(self.offset) != b'\n' && self.at(self.offset) != 0 {
                        self.offset += 1;
                    }
                } else if ty == BaseTokenType::Whitespace {
                    *out = StrRef {
                        start: Some(start),
                        len,
                    };
                    break;
                } else {
                    *out = StrRef {
                        start: Some(start),
                        len,
                    };
                    if ch == b'"' {
                        self.get_string_token(out);
                        break;
                    } else if ch == b'=' {
                        break;
                    }
                }
            } else {
                if ty == BaseTokenType::Whitespace || ch == b'=' {
                    self.offset -= len;
                    break;
                }

                if ch == b'#' {
                    self.offset -= 1;
                    break;
                }

                out.len += len;
            }
        }

        out.len != 0
    }

    /// `lookup_goto_nextline`.
    fn goto_next_line(&mut self) -> bool {
        let mut val = StrRef::default();

        loop {
            if !self.get_token(&mut val) {
                return false;
            }
            if val.start.is_some_and(|s| self.at(s) == b'\n') {
                return true;
            }
        }
    }
}

/// `dstr_replace`: leftmost, non-overlapping, single pass.
fn replace_all(input: &[u8], find: &[u8], replace: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i..].starts_with(find) {
            out.extend_from_slice(replace);
            i += find.len();
        } else {
            out.push(input[i]);
            i += 1;
        }
    }
    out
}

/// `convert_string`.
fn convert_string(s: &[u8]) -> Vec<u8> {
    let s = replace_all(s, b"\\n", b"\n");
    let s = replace_all(&s, b"\\t", b"\t");
    let s = replace_all(&s, b"\\r", b"\r");
    replace_all(&s, b"\\\"", b"\"")
}

/// A set of text lookups (`struct text_lookup`).
#[derive(Debug, Default)]
pub struct TextLookup {
    items: HashMap<Vec<u8>, CString>,
}

impl TextLookup {
    /// An empty lookup.
    pub fn new() -> Self {
        Self::default()
    }

    /// `lookup_addfiledata`: parses `data` (up to the first NUL) and adds its
    /// entries, replacing existing keys.
    pub fn add_data(&mut self, data: &[u8]) {
        let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
        let mut lex = Lexer {
            data: &data[..end],
            offset: 0,
        };
        let mut name = StrRef::default();
        let mut value = StrRef::default();

        'outer: while lex.get_token(&mut name) {
            let name_start = name.start.unwrap_or(0);
            if lex.at(name_start) == b'\n' {
                continue;
            }

            let mut got_eq = false;
            loop {
                if !lex.get_token(&mut value) {
                    break 'outer;
                }
                let value_start = value.start.unwrap_or(0);
                let first = lex.at(value_start);
                if first == b'\n' {
                    continue 'outer;
                } else if !got_eq && first == b'=' {
                    got_eq = true;
                    continue;
                }
                break;
            }

            let key = lex.data[name_start..name_start + name.len].to_vec();
            let value_start = value.start.unwrap_or(0);
            let converted = convert_string(&lex.data[value_start..value_start + value.len]);
            // Tokens never contain NUL, so this cannot fail.
            if let Ok(cvalue) = CString::new(converted) {
                self.items.insert(key, cvalue);
            }

            if !lex.goto_next_line() {
                break;
            }
        }
    }

    /// `text_lookup_add`: reads `path` like `os_fopen` + `os_fread_utf8`
    /// (strips a UTF-8 BOM) and adds its entries. Returns false if the file
    /// cannot be read or has no content after the BOM.
    pub fn add_file(&mut self, path: &Path) -> bool {
        let Ok(bytes) = std::fs::read(path) else {
            return false;
        };

        let body: &[u8] = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
        if body.is_empty() {
            return false;
        }

        // `dstr_replace(&file_str, "\r", " ")`
        let normalised = replace_all(body, b"\r", b" ");
        self.add_data(&normalised);
        true
    }

    /// `text_lookup_getstr`: case-sensitive lookup.
    pub fn get(&self, key: &CStr) -> Option<&CStr> {
        self.items.get(key.to_bytes()).map(CString::as_c_str)
    }
}
