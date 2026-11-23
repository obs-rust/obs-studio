//! The libobs version getters of `libobs/obs-version.c.h`.
//!
//! The numbers are read at compile time from `libobs/obs-config.h`, the
//! public header the C takes `LIBOBS_API_VER` from, so a version bump there
//! reaches the Rust without a second edit.

/// `libobs/obs-config.h`, the one source of the API version numbers.
pub const OBS_CONFIG_H: &str = include_str!("../../../libobs/obs-config.h");

/// `MAKE_SEMANTIC_VERSION(major, minor, patch)`: `major << 24 | minor << 16 | patch`.
///
/// Bits shifted past the top are dropped and the fields are not masked, as
/// the C macro does with unsigned operands.
#[must_use]
pub const fn make_semantic_version(major: u32, minor: u32, patch: u32) -> u32 {
    (major << 24) | (minor << 16) | patch
}

/// The value of `name` in `obs-config.h`; a missing or malformed define
/// stops the build.
const fn config_value(name: &str) -> u32 {
    match define_value(OBS_CONFIG_H, name) {
        Some(value) => value,
        None => panic!("libobs/obs-config.h has no decimal define of an API version number"),
    }
}

/// `LIBOBS_API_MAJOR_VER` from `obs-config.h`.
pub const LIBOBS_API_MAJOR_VER: u32 = config_value("LIBOBS_API_MAJOR_VER");
/// `LIBOBS_API_MINOR_VER` from `obs-config.h`.
pub const LIBOBS_API_MINOR_VER: u32 = config_value("LIBOBS_API_MINOR_VER");
/// `LIBOBS_API_PATCH_VER` from `obs-config.h`.
pub const LIBOBS_API_PATCH_VER: u32 = config_value("LIBOBS_API_PATCH_VER");

/// `LIBOBS_API_VER`, what `obs_get_version()` returns.
pub const LIBOBS_API_VER: u32 = make_semantic_version(
    LIBOBS_API_MAJOR_VER,
    LIBOBS_API_MINOR_VER,
    LIBOBS_API_PATCH_VER,
);

const fn is_blank(byte: u8) -> bool {
    byte == b' ' || byte == b'\t'
}

/// Whether `text[at..]` starts with `prefix`.
const fn starts_with_at(text: &[u8], at: usize, prefix: &[u8]) -> bool {
    if text.len() - at < prefix.len() {
        return false;
    }
    let mut i = 0;
    while i < prefix.len() {
        if text[at + i] != prefix[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// The decimal value of the define on the line `text[start..end]`, if the
/// line is `#define <name> <decimal>` with only blanks around the parts.
const fn line_value(text: &[u8], start: usize, end: usize, name: &[u8]) -> Option<u32> {
    const DEFINE: &[u8] = b"#define";
    if end - start < DEFINE.len() || !starts_with_at(text, start, DEFINE) {
        return None;
    }
    let mut at = start + DEFINE.len();
    if at == end || !is_blank(text[at]) {
        return None;
    }
    while at < end && is_blank(text[at]) {
        at += 1;
    }
    if end - at < name.len() || !starts_with_at(text, at, name) {
        return None;
    }
    at += name.len();
    if at == end || !is_blank(text[at]) {
        return None;
    }
    while at < end && is_blank(text[at]) {
        at += 1;
    }
    let mut value: u32 = 0;
    let mut digits = 0;
    while at < end && text[at].is_ascii_digit() {
        value = match value.checked_mul(10) {
            Some(tens) => match tens.checked_add((text[at] - b'0') as u32) {
                Some(next) => next,
                None => return None,
            },
            None => return None,
        };
        digits += 1;
        at += 1;
    }
    while at < end && (is_blank(text[at]) || text[at] == b'\r') {
        at += 1;
    }
    if digits == 0 || at != end {
        return None;
    }
    Some(value)
}

/// The value of the line `#define <name> <decimal>` in `header`, or `None`
/// when there is no such line or its value is not a decimal `u32`. The
/// first such line wins.
#[must_use]
pub const fn define_value(header: &str, name: &str) -> Option<u32> {
    let text = header.as_bytes();
    let name = name.as_bytes();
    let mut start = 0;
    while start < text.len() {
        let mut end = start;
        while end < text.len() && text[end] != b'\n' {
            end += 1;
        }
        if let Some(value) = line_value(text, start, end, name) {
            return Some(value);
        }
        start = end + 1;
    }
    None
}
