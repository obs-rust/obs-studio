//! Safe core for `libobs/util/config-file.c`.
//!
//! The C ABI shim lives in [`crate::ffi::config_file`].
//!
//! Faithful quirks kept from the C implementation (characterized by
//! `test/cmocka/test_config_file.c`):
//! - Only leading whitespace on a line is skipped; inner whitespace stays in
//!   names and values. `#` starts a comment only as the first non-whitespace
//!   character of a line; `;` is not a comment character.
//! - A `[]` section header stops the entire parse. An unterminated `[name`
//!   header still creates the section. Lines before the first `[` are
//!   dropped. Duplicate sections and keys are all kept; lookups find the
//!   last-added duplicate.
//! - A line without `=` followed by a newline is dropped, but at EOF it
//!   becomes a key with an empty value (`success` in `config_parse_string`
//!   stays true at EOF).
//! - Values unescape `\\`, `\r`, `\n`; unknown escapes keep the backslash.
//!   On write, `\` -> `\\`, CR -> `\r`, LF -> `\n`.
//! - `config_set_default_*` also writes the user value when none exists.
//! - `set_double` uses `%.17g` and appends `.0` when no `.` or `e` appears;
//!   `set_default_double` uses `%g` (6 significant digits).

use std::ffi::CString;
use std::io::Write;
use std::sync::{Mutex, MutexGuard};

/// `CONFIG_SUCCESS` from `util/config-file.h`.
pub const CONFIG_SUCCESS: i32 = 0;
/// `CONFIG_FILENOTFOUND` from `util/config-file.h`.
pub const CONFIG_FILENOTFOUND: i32 = -1;
/// `CONFIG_ERROR` from `util/config-file.h`.
pub const CONFIG_ERROR: i32 = -2;

/// `enum config_open_type` from `util/config-file.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenType {
    Existing,
    Always,
}

/// C keeps `CONFIG_OPEN_EXISTING` for any value other than
/// `CONFIG_OPEN_ALWAYS`.
pub fn open_type_from_c(v: i32) -> OpenType {
    match v {
        1 => OpenType::Always,
        _ => OpenType::Existing,
    }
}

#[derive(Debug)]
struct Item {
    name: CString,
    value: CString,
}

#[derive(Debug)]
struct Section {
    name: CString,
    items: Vec<Item>,
}

#[derive(Debug, Default)]
struct Inner {
    file: Option<Vec<u8>>,
    sections: Vec<Section>,
    defaults: Vec<Section>,
}

/// `config_t` (`struct config_data`).
#[derive(Debug)]
pub struct Config {
    inner: Mutex<Inner>,
}

/// C stores `char *` keys and values; interior NULs cannot exist. A byte
/// slice passed to the safe API is truncated at its first NUL, matching
/// what a C string could hold.
fn to_cstring(bytes: &[u8]) -> CString {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    // The slice has no interior NUL left.
    CString::new(&bytes[..end]).expect("slice is NUL-free after truncation")
}

/// Key form for lookups: C compares `char *`, so a NUL truncates the key.
fn ckey(bytes: &[u8]) -> &[u8] {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    &bytes[..end]
}

/// `HASH_FIND_STR` on uthash returns the most recently added entry when a
/// key was added twice (`HASH_ADD_STR` prepends within a bucket).
fn find_item<'a>(sections: &'a [Section], section: &[u8], name: &[u8]) -> Option<&'a Item> {
    let sec = sections
        .iter()
        .rev()
        .find(|s| s.name.as_bytes() == section)?;
    sec.items.iter().rev().find(|i| i.name.as_bytes() == name)
}

fn set_item(sections: &mut Vec<Section>, section: &[u8], name: &[u8], value: Vec<u8>) {
    let sec = match sections
        .iter_mut()
        .rev()
        .find(|s| s.name.as_bytes() == section)
    {
        Some(sec) => sec,
        None => {
            sections.push(Section {
                name: to_cstring(section),
                items: Vec::new(),
            });
            sections.last_mut().expect("just pushed")
        }
    };
    match sec
        .items
        .iter_mut()
        .rev()
        .find(|i| i.name.as_bytes() == name)
    {
        Some(item) => item.value = to_cstring(&value),
        None => sec.items.push(Item {
            name: to_cstring(name),
            value: to_cstring(&value),
        }),
    }
}

/* --------------------------------------------------------------------- */
/* basetoken lexer (util/lexer.c `lexer_getbasetoken`, PARSE_WHITESPACE)   */

fn is_ws(c: u8) -> bool {
    c == b' ' || c == b'\r' || c == b'\t' || c == b'\n'
}

fn is_newline(c: u8) -> bool {
    c == b'\r' || c == b'\n'
}

fn is_newline_pair(a: u8, b: u8) -> bool {
    (a == b'\r' && b == b'\n') || (a == b'\n' && b == b'\r')
}

/// Word is the merge of C's BASETOKEN_ALPHA and BASETOKEN_DIGIT runs; the
/// split never affects parse results since tokens are concatenated.
#[derive(PartialEq)]
enum Tok {
    Ws,
    Word,
    Other,
}

struct Token<'a> {
    kind: Tok,
    bytes: &'a [u8],
}

impl Token<'_> {
    fn newline(&self) -> bool {
        is_newline(self.bytes[0])
    }
}

fn next_token<'a>(buf: &'a [u8], pos: &mut usize) -> Option<Token<'a>> {
    if *pos >= buf.len() {
        return None;
    }
    let start = *pos;
    let c = buf[start];
    *pos += 1;
    let kind = if is_ws(c) {
        if is_newline(c) && *pos < buf.len() && is_newline_pair(c, buf[*pos]) {
            *pos += 1;
        }
        Tok::Ws
    } else if c.is_ascii_alphanumeric() {
        while *pos < buf.len() && buf[*pos].is_ascii_alphanumeric() {
            *pos += 1;
        }
        Tok::Word
    } else {
        Tok::Other
    };
    Some(Token {
        kind,
        bytes: &buf[start..*pos],
    })
}

/// `unescape` from config-file.c.
fn unescape(v: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len());
    let mut i = 0;
    while i < v.len() {
        let c = v[i];
        if c == b'\\' && i + 1 < v.len() {
            match v[i + 1] {
                b'\\' => {
                    out.push(b'\\');
                    i += 2;
                    continue;
                }
                b'r' => {
                    out.push(b'\r');
                    i += 2;
                    continue;
                }
                b'n' => {
                    out.push(b'\n');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Value escaping on write (`config_save`): `\` -> `\\`, CR -> `\r`,
/// LF -> `\n`. Names and section names are written raw.
fn escape(v: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len());
    for &c in v {
        match c {
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\n' => out.extend_from_slice(b"\\n"),
            _ => out.push(c),
        }
    }
    out
}

/// `config_parse_section`: items of one section until `[` or EOF.
fn parse_section(buf: &[u8], pos: &mut usize, items: &mut Vec<Item>) {
    while let Some(tok) = next_token(buf, pos) {
        if tok.kind == Tok::Ws {
            continue;
        }
        if tok.kind == Tok::Other {
            match tok.bytes[0] {
                b'#' => {
                    while let Some(t) = next_token(buf, pos) {
                        if t.newline() {
                            break;
                        }
                    }
                    continue;
                }
                b'[' => {
                    *pos -= 1;
                    return;
                }
                _ => {}
            }
        }
        let mut name = tok.bytes.to_vec();
        let mut eq = true;
        loop {
            match next_token(buf, pos) {
                None => break,
                Some(t) => {
                    if t.bytes[0] == b'=' {
                        break;
                    }
                    if t.newline() {
                        eq = false;
                        break;
                    }
                    name.extend_from_slice(t.bytes);
                }
            }
        }
        if !eq {
            continue;
        }
        let mut value = Vec::new();
        while let Some(t) = next_token(buf, pos) {
            if t.newline() {
                break;
            }
            value.extend_from_slice(t.bytes);
        }
        items.push(Item {
            name: to_cstring(&name),
            value: to_cstring(&unescape(&value)),
        });
    }
}

/// `parse_config_data`. The C lexer walks a `char *`, so parsing stops at
/// the first NUL byte.
fn parse_config_data(buf: &[u8], sections: &mut Vec<Section>) {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    let buf = &buf[..end];
    let mut pos = 0;
    while let Some(tok) = next_token(buf, &mut pos) {
        if tok.kind == Tok::Ws {
            continue;
        }
        if tok.bytes[0] != b'[' {
            while let Some(t) = next_token(buf, &mut pos) {
                if t.newline() {
                    break;
                }
            }
            continue;
        }
        let mut name = Vec::new();
        loop {
            match next_token(buf, &mut pos) {
                None => break,
                Some(t) => {
                    if t.bytes[0] == b']' || t.newline() {
                        break;
                    }
                    name.extend_from_slice(t.bytes);
                }
            }
        }
        if name.is_empty() {
            return;
        }
        sections.push(Section {
            name: to_cstring(&name),
            items: Vec::new(),
        });
        let sec = sections.last_mut().expect("just pushed");
        parse_section(buf, &mut pos, &mut sec.items);
    }
}

/* --------------------------------------------------------------------- */
/* C library parsing helpers (strtoll / strtoull / strtod)                 */

fn is_c_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r')
}

fn skip_space(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && is_c_space(b[i]) {
        i += 1;
    }
    i
}

/// `strtoll(str, NULL, base)` for base 10/16: leading space, optional sign,
/// digit run, saturates to i64::MIN/MAX. No digits -> 0.
fn str_to_i64_impl(b: &[u8], hex: bool) -> i64 {
    let mut i = skip_space(b, 0);
    let neg = match b.get(i) {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };
    let base = if hex { 16u128 } else { 10u128 };
    let mut mag: u128 = 0;
    let mut any = false;
    let mut overflow = false;
    while let Some(&c) = b.get(i) {
        let d = match c {
            b'0'..=b'9' => u128::from(c - b'0'),
            b'a'..=b'f' if hex => u128::from(c - b'a' + 10),
            b'A'..=b'F' if hex => u128::from(c - b'A' + 10),
            _ => break,
        };
        any = true;
        match mag.checked_mul(base).and_then(|m| m.checked_add(d)) {
            Some(m) => mag = m,
            None => {
                overflow = true;
                mag = u128::MAX;
            }
        }
        i += 1;
    }
    if !any {
        return 0;
    }
    if neg {
        if overflow || mag > (i64::MAX as u128) + 1 {
            i64::MIN
        } else {
            (mag as i128).wrapping_neg() as i64
        }
    } else if overflow || mag > i64::MAX as u128 {
        i64::MAX
    } else {
        mag as i64
    }
}

/// `strtoull(str, NULL, base)`: like [`str_to_i64_impl`] but saturates to
/// u64::MAX and negates modulo 2^64 on `-`.
fn str_to_u64_impl(b: &[u8], hex: bool) -> u64 {
    let mut i = skip_space(b, 0);
    let neg = match b.get(i) {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };
    let base = if hex { 16u64 } else { 10u64 };
    let mut mag: u64 = 0;
    let mut any = false;
    let mut overflow = false;
    while let Some(&c) = b.get(i) {
        let d = match c {
            b'0'..=b'9' => u64::from(c - b'0'),
            b'a'..=b'f' if hex => u64::from(c - b'a' + 10),
            b'A'..=b'F' if hex => u64::from(c - b'A' + 10),
            _ => break,
        };
        any = true;
        match mag.checked_mul(base).and_then(|m| m.checked_add(d)) {
            Some(m) => mag = m,
            None => {
                overflow = true;
                mag = u64::MAX;
            }
        }
        i += 1;
    }
    if !any {
        return 0;
    }
    if overflow {
        return u64::MAX;
    }
    if neg { mag.wrapping_neg() } else { mag }
}

/// `str_to_int64` from config-file.c: `0x` prefix selects base 16 on the
/// raw string; anything else is base 10.
fn str_to_int64(b: &[u8]) -> i64 {
    if b.is_empty() {
        return 0;
    }
    if b.starts_with(b"0x") {
        str_to_i64_impl(&b[2..], true)
    } else {
        str_to_i64_impl(b, false)
    }
}

/// `str_to_uint64` from config-file.c.
fn str_to_uint64(b: &[u8]) -> u64 {
    if b.is_empty() {
        return 0;
    }
    if b.starts_with(b"0x") {
        str_to_u64_impl(&b[2..], true)
    } else {
        str_to_u64_impl(b, false)
    }
}

fn starts_ci(b: &[u8], pat: &[u8]) -> bool {
    b.len() >= pat.len() && b[..pat.len()].eq_ignore_ascii_case(pat)
}

fn is_hex_digit(c: u8) -> bool {
    c.is_ascii_digit() || (b'a'..=b'f').contains(&c) || (b'A'..=b'F').contains(&c)
}

fn hex_val(c: u8) -> u64 {
    match c {
        b'0'..=b'9' => u64::from(c - b'0'),
        b'a'..=b'f' => u64::from(c - b'a' + 10),
        _ => u64::from(c - b'A' + 10),
    }
}

/// `strtod`: parse the valid prefix like the C library. Covers leading
/// space, sign, `inf`/`infinity`, `nan(...)`, `0x` hex floats with `p`
/// exponent, and decimal floats with `e` exponent. No conversion -> 0.
fn parse_double(b: &[u8]) -> f64 {
    let mut i = skip_space(b, 0);
    let neg = match b.get(i) {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };
    let rest = &b[i..];
    let sign = if neg { -1.0 } else { 1.0 };
    if starts_ci(rest, b"infinity") {
        return sign * f64::INFINITY;
    }
    if starts_ci(rest, b"inf") {
        return sign * f64::INFINITY;
    }
    if starts_ci(rest, b"nan") {
        return if neg { -f64::NAN } else { f64::NAN };
    }
    if rest.len() >= 2 && rest[0] == b'0' && (rest[1] == b'x' || rest[1] == b'X') {
        let has_digits = match rest.get(2) {
            Some(&c) if is_hex_digit(c) => true,
            Some(b'.') => rest.get(3).is_some_and(|&c| is_hex_digit(c)),
            _ => false,
        };
        if has_digits {
            return sign * parse_hex_float(&rest[2..]);
        }
        // "0x" without hex digits falls back to parsing "0".
    }
    // decimal: int digits [. frac digits] [eE [sign] exp digits]
    let mut j = 0;
    let mut int_digits = 0usize;
    while j < rest.len() && rest[j].is_ascii_digit() {
        j += 1;
        int_digits += 1;
    }
    let mut frac_digits = 0usize;
    if rest.get(j) == Some(&b'.') {
        j += 1;
        while j < rest.len() && rest[j].is_ascii_digit() {
            j += 1;
            frac_digits += 1;
        }
    }
    if int_digits == 0 && frac_digits == 0 {
        return 0.0;
    }
    let mant_end = j;
    if matches!(rest.get(j), Some(b'e') | Some(b'E')) {
        let mut k = j + 1;
        if matches!(rest.get(k), Some(b'+') | Some(b'-')) {
            k += 1;
        }
        let exp_start = k;
        while k < rest.len() && rest[k].is_ascii_digit() {
            k += 1;
        }
        if k > exp_start {
            j = k;
        } else {
            j = mant_end;
        }
    }
    let text = std::str::from_utf8(&rest[..j]).unwrap_or("");
    text.parse::<f64>()
        .map_or(0.0, |v| if neg { -v } else { v })
}

/// Value of a `0x` hex float body (digits past `0x`): hex digits with an
/// optional `.` and an optional `p`/`P` signed binary exponent.
fn parse_hex_float(b: &[u8]) -> f64 {
    let mut j = 0;
    let mut mantissa: u128 = 0;
    let mut shift = 0i64;
    let mut extra = 0i64;
    let mut saw_digit = false;
    loop {
        match b.get(j) {
            Some(&c) if is_hex_digit(c) => {
                saw_digit = true;
                if mantissa <= (u128::MAX >> 4) {
                    mantissa = (mantissa << 4) | u128::from(hex_val(c));
                } else {
                    extra += 4;
                }
                j += 1;
            }
            Some(b'.') => {
                j += 1;
                while let Some(&c) = b.get(j) {
                    if !is_hex_digit(c) {
                        break;
                    }
                    saw_digit = true;
                    if mantissa <= (u128::MAX >> 4) {
                        mantissa = (mantissa << 4) | u128::from(hex_val(c));
                        shift -= 4;
                    } else {
                        extra += 4;
                        shift -= 4;
                    }
                    j += 1;
                }
                break;
            }
            _ => break,
        }
    }
    if !saw_digit {
        return 0.0;
    }
    let mut exp: i64 = 0;
    if matches!(b.get(j), Some(b'p') | Some(b'P')) {
        let mut k = j + 1;
        let eneg = match b.get(k) {
            Some(b'-') => {
                k += 1;
                true
            }
            Some(b'+') => {
                k += 1;
                false
            }
            _ => false,
        };
        let estart = k;
        let mut e: i64 = 0;
        while let Some(&c) = b.get(k) {
            if !c.is_ascii_digit() {
                break;
            }
            e = (e * 10 + i64::from(c - b'0')).min(1_000_000);
            k += 1;
        }
        if k > estart {
            exp = if eneg { -e } else { e };
        }
    }
    // value = mantissa * 2^(shift + extra + exp); the u128 -> f64 cast
    // rounds half-even, matching strtod's rounding of the full value.
    (mantissa as f64) * 2f64.powi((shift + extra + exp) as i32)
}

/// `astrcmpi(value, "true") == 0 || str_to_uint64(value) != 0`.
fn bool_value(b: &[u8]) -> bool {
    b.eq_ignore_ascii_case(b"true") || str_to_uint64(b) != 0
}

/* --------------------------------------------------------------------- */
/* printf-format helpers                                                   */

/// `snprintf("%.*g", prec, v)` pieces: the `%.{prec-1}e` mantissa/exponent,
/// deciding `%e` vs `%f` by C's rule (exp < -4 or >= prec), then stripped
/// of trailing zeros.
fn g_parts(v: f64, prec: usize) -> (String, i64, bool) {
    let p = prec.max(1);
    let sci = format!("{:.*e}", p - 1, v);
    let epos = sci.find('e').expect("e-format always has e");
    let exp: i64 = sci[epos + 1..].parse().expect("e-format exponent");
    if exp < -4 || exp >= p as i64 {
        (strip_zeros(&sci[..epos]).to_owned(), exp, true)
    } else {
        let decimals = (p as i64 - 1 - exp).max(0) as usize;
        (
            strip_zeros(&format!("{:.*}", decimals, v)).to_owned(),
            exp,
            false,
        )
    }
}

fn nan_inf(v: f64) -> Option<Vec<u8>> {
    if v.is_nan() {
        Some(if v.is_sign_negative() {
            b"-nan".to_vec()
        } else {
            b"nan".to_vec()
        })
    } else if v.is_infinite() {
        Some(if v < 0.0 {
            b"-inf".to_vec()
        } else {
            b"inf".to_vec()
        })
    } else {
        None
    }
}

/// `snprintf("%.*g", prec, v)` for prec > 0: `%e`/`%f` chosen by the C
/// rule, trailing zeros and a bare `.` stripped, exponent signed with at
/// least two digits (`1e+06`, `1e-05`).
fn g_format(v: f64, prec: usize) -> Vec<u8> {
    if let Some(s) = nan_inf(v) {
        return s;
    }
    let (mant, exp, e_form) = g_parts(v, prec);
    if e_form {
        format!(
            "{}e{}{:02}",
            mant,
            if exp < 0 { '-' } else { '+' },
            exp.abs()
        )
        .into_bytes()
    } else {
        mant.into_bytes()
    }
}

/// `%g` zero stripping: drop trailing zeros after a `.`, then a bare `.`.
fn strip_zeros(s: &str) -> &str {
    if !s.contains('.') {
        return s;
    }
    let s = s.trim_end_matches('0');
    s.strip_suffix('.').unwrap_or(s)
}

/// `os_dtostr`: `%.17g`, `.0` appended when no `.`/`e`, then the C
/// exponent normalization simulated byte-for-byte — including its quirk:
/// `memmove(start, end, length - (end - dst))` never reaches the NUL, so
/// a `+` exponent's tail digits stay behind and the last digit is
/// duplicated (`1e+18` -> `1e188`, `1e+151` -> `1e1511`). `-` exponents
/// lose leading zeros cleanly (`1e-05` -> `1e-5`).
fn dtostr(v: f64) -> Vec<u8> {
    if let Some(s) = nan_inf(v) {
        return s;
    }
    let mut s = g_format(v, 17);
    if !s.contains(&b'.') && !s.contains(&b'e') {
        s.extend_from_slice(b".0");
    }
    let Some(ei) = s.iter().position(|&b| b == b'e') else {
        return s;
    };
    let mut start = ei + 1;
    let mut end = start + 1;
    if s[start] == b'-' {
        start += 1;
    }
    while end < s.len() && s[end] == b'0' {
        end += 1;
    }
    if end != start {
        // C does not NUL-terminate the shifted span; leftover tail bytes
        // remain part of the string, hence the duplicated last digit.
        let n = s.len() - end;
        s.copy_within(end..end + n, start);
    }
    s
}

/* --------------------------------------------------------------------- */
/* paths (byte paths; C treats them as fopen(3) strings)                   */

#[cfg(unix)]
fn path_from_bytes(b: &[u8]) -> std::path::PathBuf {
    use std::os::unix::ffi::OsStrExt;
    std::path::PathBuf::from(std::ffi::OsStr::from_bytes(b))
}

#[cfg(not(unix))]
fn path_from_bytes(b: &[u8]) -> std::path::PathBuf {
    // C treats paths as UTF-8 for _wfopen on Windows.
    std::path::PathBuf::from(String::from_utf8_lossy(b).into_owned())
}

/// `file.ext` construction from `config_save_safe`: a `.` is inserted
/// unless the extension already starts with one.
fn with_ext(file: &[u8], ext: &[u8]) -> Vec<u8> {
    let mut out = file.to_vec();
    if ext.first() != Some(&b'.') {
        out.push(b'.');
    }
    out.extend_from_slice(ext);
    out
}

/// `os_fread_utf8` semantics: whole file minus a leading UTF-8 BOM; None
/// when the file cannot be opened.
fn read_file(path: &std::path::Path) -> Option<Vec<u8>> {
    let data = std::fs::read(path).ok()?;
    Some(match data.strip_prefix(b"\xEF\xBB\xBF") {
        Some(rest) => rest.to_vec(),
        None => data,
    })
}

/// `config_parse_file` for one table: reads `file` (creating it on
/// `always_open`), strips the BOM, parses.
fn parse_file_into(sections: &mut Vec<Section>, file: &[u8], always_open: bool) -> i32 {
    let path = path_from_bytes(file);
    let data = match read_file(&path) {
        Some(data) => Some(data),
        None if always_open => {
            // "w+": create an empty file, then read back nothing.
            match std::fs::File::create(&path) {
                Ok(_) => None,
                Err(_) => return CONFIG_FILENOTFOUND,
            }
        }
        None => return CONFIG_FILENOTFOUND,
    };
    if let Some(data) = data {
        parse_config_data(&data, sections);
    }
    CONFIG_SUCCESS
}

impl Config {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `config_open_string`. `file` is NULL; `config_save` will fail.
    pub fn open_string(text: &[u8]) -> Self {
        let mut sections = Vec::new();
        parse_config_data(text, &mut sections);
        Self {
            inner: Mutex::new(Inner {
                file: None,
                sections,
                defaults: Vec::new(),
            }),
        }
    }

    /// `config_open`. The stored file name is kept verbatim.
    pub fn open(file: &[u8], open_type: OpenType) -> Result<Self, i32> {
        let mut sections = Vec::new();
        let code = parse_file_into(&mut sections, file, open_type == OpenType::Always);
        if code != CONFIG_SUCCESS {
            return Err(code);
        }
        Ok(Self {
            inner: Mutex::new(Inner {
                file: Some(file.to_vec()),
                sections,
                defaults: Vec::new(),
            }),
        })
    }

    /// `config_create`: truncates/creates `file` and does not read it.
    pub fn create(file: &[u8]) -> Option<Self> {
        std::fs::File::create(path_from_bytes(file)).ok()?;
        Some(Self {
            inner: Mutex::new(Inner {
                file: Some(file.to_vec()),
                sections: Vec::new(),
                defaults: Vec::new(),
            }),
        })
    }

    /// `config_open_defaults`: parses `file` into the defaults table.
    pub fn open_defaults(&self, file: &[u8]) -> i32 {
        parse_file_into(&mut self.lock().defaults, file, false)
    }

    /// `config_save`: whole serialized config to `file` (truncate).
    /// `os_fopen(.., "wb")` failure -> `CONFIG_FILENOTFOUND`; any write or
    /// close failure -> `CONFIG_ERROR`.
    pub fn save(&self) -> i32 {
        let inner = self.lock();
        let Some(file) = &inner.file else {
            return CONFIG_ERROR;
        };
        self.write_to(file, &inner)
    }

    fn write_to(&self, file: &[u8], inner: &Inner) -> i32 {
        let path = path_from_bytes(file);
        let mut f = match std::fs::File::create(&path) {
            Ok(f) => f,
            Err(_) => return CONFIG_FILENOTFOUND,
        };
        let mut out = Vec::new();
        #[cfg(windows)]
        out.extend_from_slice(b"\xEF\xBB\xBF"); // C writes a BOM on Windows.
        for (i, sec) in inner.sections.iter().enumerate() {
            if i > 0 {
                out.push(b'\n');
            }
            out.push(b'[');
            out.extend_from_slice(sec.name.as_bytes());
            out.extend_from_slice(b"]\n");
            for item in &sec.items {
                out.extend_from_slice(item.name.as_bytes());
                out.push(b'=');
                out.extend_from_slice(&escape(item.value.as_bytes()));
                out.push(b'\n');
            }
        }
        if f.write_all(&out).is_err() || f.flush().is_err() {
            return CONFIG_ERROR;
        }
        drop(f);
        CONFIG_SUCCESS
    }

    /// `config_save_safe` -> `os_safe_replace` on POSIX: write to
    /// `file.temp_ext`, rename `file` to `file.backup_ext` if it exists,
    /// then rename the temp into place.
    pub fn save_safe(&self, temp_ext: Option<&[u8]>, backup_ext: Option<&[u8]>) -> i32 {
        let temp_ext = match temp_ext {
            Some(e) if !e.is_empty() => e,
            _ => return CONFIG_ERROR,
        };
        let inner = self.lock();
        let Some(file) = inner.file.clone() else {
            return CONFIG_ERROR;
        };
        let temp = with_ext(&file, temp_ext);
        let ret = self.write_to(&temp, &inner);
        if ret != CONFIG_SUCCESS {
            return ret;
        }
        let backup = backup_ext
            .filter(|e| !e.is_empty())
            .map(|e| with_ext(&file, e));
        let target = path_from_bytes(&file);
        let from = path_from_bytes(&temp);
        if let Some(backup) = &backup
            && target.exists()
            && std::fs::rename(&target, path_from_bytes(backup)).is_err()
        {
            return CONFIG_ERROR;
        }
        if std::fs::rename(&from, &target).is_err() {
            return CONFIG_ERROR;
        }
        CONFIG_SUCCESS
    }

    /// `config_num_sections`.
    pub fn num_sections(&self) -> usize {
        self.lock().sections.len()
    }

    /// `config_get_section` by insertion index.
    pub fn get_section(&self, idx: usize) -> Option<Vec<u8>> {
        self.lock()
            .sections
            .get(idx)
            .map(|s| s.name.as_bytes().to_vec())
    }

    /// `config_get_string`: user value, else default value.
    pub fn get_string(&self, section: &[u8], name: &[u8]) -> Option<Vec<u8>> {
        self.get_string_ptr(ckey(section), ckey(name))
            .map(|p| unsafe { std::ffi::CStr::from_ptr(p) }.to_bytes().to_vec())
    }

    fn get_string_ptr(&self, section: &[u8], name: &[u8]) -> Option<*const std::ffi::c_char> {
        let inner = self.lock();
        find_item(&inner.sections, section, name)
            .or_else(|| find_item(&inner.defaults, section, name))
            .map(|i| i.value.as_ptr())
    }

    /// `config_get_int` (`str_to_int64` semantics).
    pub fn get_int(&self, section: &[u8], name: &[u8]) -> i64 {
        self.get_string(section, name)
            .map_or(0, |v| str_to_int64(&v))
    }

    /// `config_get_uint` (`str_to_uint64` semantics).
    pub fn get_uint(&self, section: &[u8], name: &[u8]) -> u64 {
        self.get_string(section, name)
            .map_or(0, |v| str_to_uint64(&v))
    }

    /// `config_get_bool` (`astrcmpi(.., "true") || str_to_uint64 != 0`).
    pub fn get_bool(&self, section: &[u8], name: &[u8]) -> bool {
        self.get_string(section, name)
            .is_some_and(|v| bool_value(&v))
    }

    /// `config_get_double` (`os_strtod` semantics).
    pub fn get_double(&self, section: &[u8], name: &[u8]) -> f64 {
        self.get_string(section, name)
            .map_or(0.0, |v| parse_double(&v))
    }

    /// `config_set_string`; a None value is stored as "" like C.
    pub fn set_string(&self, section: &[u8], name: &[u8], value: &[u8]) {
        let value = value.to_vec();
        let mut inner = self.lock();
        set_item(&mut inner.sections, ckey(section), ckey(name), value);
    }

    /// `config_set_int` (`%" PRId64 "`).
    pub fn set_int(&self, section: &[u8], name: &[u8], value: i64) {
        self.set_string(section, name, value.to_string().as_bytes());
    }

    /// `config_set_uint` (`%" PRIu64 "`).
    pub fn set_uint(&self, section: &[u8], name: &[u8], value: u64) {
        self.set_string(section, name, value.to_string().as_bytes());
    }

    /// `config_set_bool` (`"true"`/`"false"`).
    pub fn set_bool(&self, section: &[u8], name: &[u8], value: bool) {
        self.set_string(section, name, if value { b"true" } else { b"false" });
    }

    /// `config_set_double` (`os_dtostr`: `%.17g` plus `.0`).
    pub fn set_double(&self, section: &[u8], name: &[u8], value: f64) {
        self.set_string(section, name, &dtostr(value));
    }

    /// `config_remove_value` on the user table.
    pub fn remove_value(&self, section: &[u8], name: &[u8]) -> bool {
        let (section, name) = (ckey(section), ckey(name));
        let mut inner = self.lock();
        let Some(sec) = inner
            .sections
            .iter_mut()
            .rev()
            .find(|s| s.name.as_bytes() == section)
        else {
            return false;
        };
        match sec.items.iter().rposition(|i| i.name.as_bytes() == name) {
            Some(idx) => {
                sec.items.remove(idx);
                true
            }
            None => false,
        }
    }

    fn set_default(&self, section: &[u8], name: &[u8], value: Vec<u8>) {
        let (section, name) = (ckey(section), ckey(name));
        let mut inner = self.lock();
        set_item(&mut inner.defaults, section, name, value.clone());
        if find_item(&inner.sections, section, name).is_none() {
            set_item(&mut inner.sections, section, name, value);
        }
    }

    /// `config_set_default_string`; also writes the user value when absent.
    pub fn set_default_string(&self, section: &[u8], name: &[u8], value: &[u8]) {
        self.set_default(section, name, value.to_vec());
    }

    /// `config_set_default_int`.
    pub fn set_default_int(&self, section: &[u8], name: &[u8], value: i64) {
        self.set_default(section, name, value.to_string().into_bytes());
    }

    /// `config_set_default_uint`.
    pub fn set_default_uint(&self, section: &[u8], name: &[u8], value: u64) {
        self.set_default(section, name, value.to_string().into_bytes());
    }

    /// `config_set_default_bool`.
    pub fn set_default_bool(&self, section: &[u8], name: &[u8], value: bool) {
        self.set_default(
            section,
            name,
            if value {
                b"true".to_vec()
            } else {
                b"false".to_vec()
            },
        );
    }

    /// `config_set_default_double` (plain `%g`, 6 significant digits).
    pub fn set_default_double(&self, section: &[u8], name: &[u8], value: f64) {
        self.set_default(section, name, g_format(value, 6));
    }

    /// `config_get_default_string`.
    pub fn get_default_string(&self, section: &[u8], name: &[u8]) -> Option<Vec<u8>> {
        let (section, name) = (ckey(section), ckey(name));
        let inner = self.lock();
        find_item(&inner.defaults, section, name).map(|i| i.value.to_bytes().to_vec())
    }

    /// `config_get_default_int`.
    pub fn get_default_int(&self, section: &[u8], name: &[u8]) -> i64 {
        self.get_default_string(section, name)
            .map_or(0, |v| str_to_int64(&v))
    }

    /// `config_get_default_uint`.
    pub fn get_default_uint(&self, section: &[u8], name: &[u8]) -> u64 {
        self.get_default_string(section, name)
            .map_or(0, |v| str_to_uint64(&v))
    }

    /// `config_get_default_bool`.
    pub fn get_default_bool(&self, section: &[u8], name: &[u8]) -> bool {
        self.get_default_string(section, name)
            .is_some_and(|v| bool_value(&v))
    }

    /// `config_get_default_double`.
    pub fn get_default_double(&self, section: &[u8], name: &[u8]) -> f64 {
        self.get_default_string(section, name)
            .map_or(0.0, |v| parse_double(&v))
    }

    /// `config_has_user_value`.
    pub fn has_user_value(&self, section: &[u8], name: &[u8]) -> bool {
        let (section, name) = (ckey(section), ckey(name));
        find_item(&self.lock().sections, section, name).is_some()
    }

    /// `config_has_default_value`.
    pub fn has_default_value(&self, section: &[u8], name: &[u8]) -> bool {
        let (section, name) = (ckey(section), ckey(name));
        find_item(&self.lock().defaults, section, name).is_some()
    }

    /* ---- borrowed-pointer views used by the FFI shim ---- */

    /// Borrowed section name, valid while the section lives (as in C).
    pub(crate) fn section_ptr(&self, idx: usize) -> Option<*const std::ffi::c_char> {
        self.lock().sections.get(idx).map(|s| s.name.as_ptr())
    }

    /// Borrowed user-or-default value (the C `config_get_string` contract).
    pub(crate) fn string_ptr(
        &self,
        section: &[u8],
        name: &[u8],
    ) -> Option<*const std::ffi::c_char> {
        self.get_string_ptr(ckey(section), ckey(name))
    }

    /// Borrowed default value (the C `config_get_default_string` contract).
    pub(crate) fn default_string_ptr(
        &self,
        section: &[u8],
        name: &[u8],
    ) -> Option<*const std::ffi::c_char> {
        let (section, name) = (ckey(section), ckey(name));
        let inner = self.lock();
        find_item(&inner.defaults, section, name).map(|i| i.value.as_ptr())
    }
}
