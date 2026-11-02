//! Tier 3: `obs_core::nix_system_info`'s distribution, Flatpak and desktop
//! log rules against the original C.
//!
//! `obs-c-oracle` compiles the unchanged `libobs/obs-nix-log-rules.c.h` with
//! its five functions renamed `oracle_*`. Every test runs the Rust shim, the
//! C and the safe core on the same input and compares the results exactly: a
//! returned pointer as its offset into the string (or NULL), a static string
//! by its bytes, `*key` and `*len` as written (or untouched).
//!
//! - every os-release line of up to three pieces of the keys, their near
//!   misses, both quotes, CR, LF and other bytes: the key, where its value
//!   starts, and the value's span and quote cut read from every position;
//! - the version fallback for every distro name of up to two pieces, with
//!   and without a `BUILD_ID` (and a NULL name with one, which neither side
//!   reads);
//! - every extension entry of up to four pieces of `=`, names and hashes;
//! - all four desktop pairs;
//! - `read_distribution_info()`'s loop, with the shims and with the C, over
//!   generated files with CRLF, no last newline and NUL bytes inside lines:
//!   the same distro and version;
//! - proptests (20,000 cases each) over lines, values, files and entries.
//!
//! The intentional difference (#3473): an extension entry with an empty
//! name (one that starts with `=`, or is empty) has no name in the port, so
//! it logs no line. There the C gives the first non-empty piece (the commit
//! hash) or `"(null)"`, which the tests check from an independent model,
//! and the shim must give NULL with `*len` untouched. Every other entry is
//! compared exactly.

use core::ffi::{CStr, c_char, c_int};
use std::ffi::CString;
use std::ptr;

use obs_c_oracle::nix_log_rules as oracle;
use obs_core::ffi::nix_system_info as shim;
use obs_core::nix_system_info as core_rules;
use proptest::prelude::*;

/// The pieces os-release lines are made of.
const PIECES: &[&[u8]] = &[
    b"NAME=",
    b"VERSION_ID=",
    b"BUILD_ID=",
    b"NAME",
    b"NAM",
    b"VERSION_ID",
    b"BUILD_ID",
    b"PRETTY_NAME=",
    b"name=",
    b"=",
    b" ",
    b"\"",
    b"'",
    b"\r",
    b"\n",
    b"\\",
    b"$",
    b"#",
    b"x",
    b"Unknown",
    b"\xc3\xa9",
    b"\xff",
];

/// The pieces extension entries are made of.
const EXT_PIECES: &[&[u8]] = &[
    b"=",
    b"a",
    b"org.freedesktop.Platform.GL.default",
    b"4c1f8b2a",
    b" ",
    b".",
    b";",
    b"\xc3\xa9",
    b"\xff",
];

/// A key or length the functions must leave alone when they return NULL.
const UNTOUCHED: usize = 0x5a5a;
const UNTOUCHED_KEY: c_int = 0x5a5a;

/// The offset of `p` in `base`, or `None` for NULL.
fn offset(base: *const c_char, p: *const c_char) -> Option<usize> {
    (!p.is_null()).then(|| p as usize - base as usize)
}

/// Every string of up to `max` pieces.
fn combos(pieces: &[&[u8]], max: usize) -> Vec<Vec<u8>> {
    let mut out = vec![Vec::new()];
    let mut layer = vec![Vec::new()];
    for _ in 0..max {
        let mut next = Vec::new();
        for prefix in &layer {
            for piece in pieces {
                let mut s = prefix.clone();
                s.extend_from_slice(piece);
                next.push(s);
            }
        }
        out.extend(next.iter().cloned());
        layer = next;
    }
    out
}

/// A C string of the bytes before the first NUL, as the C sees them.
fn c_string(bytes: &[u8]) -> CString {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    CString::new(&bytes[..end]).expect("no NUL")
}

/// What `_os_release_field()` gives: the value's offset and `*key`.
fn field_c(line: &CStr) -> (Option<usize>, c_int) {
    let mut key = UNTOUCHED_KEY;
    // SAFETY: a NUL-terminated line and a writable key.
    let p = unsafe { oracle::oracle_obs_nix_system_info_os_release_field(line.as_ptr(), &mut key) };
    (offset(line.as_ptr(), p), key)
}

fn field_shim(line: &CStr) -> (Option<usize>, c_int) {
    let mut key = UNTOUCHED_KEY;
    // SAFETY: as above.
    let p = unsafe { shim::obs_nix_system_info_os_release_field(line.as_ptr(), &mut key) };
    (offset(line.as_ptr(), p), key)
}

fn field_core(line: &CStr) -> (Option<usize>, c_int) {
    match core_rules::os_release_field(line.to_bytes()) {
        Some((key, start)) => (
            Some(start),
            match key {
                core_rules::OsReleaseKey::Name => 0,
                core_rules::OsReleaseKey::VersionId => 1,
                core_rules::OsReleaseKey::BuildId => 2,
            },
        ),
        None => (None, UNTOUCHED_KEY),
    }
}

/// What `_os_release_value()` gives for the value at `start`: its offset in
/// the line and `*len`.
fn value_c(line: &CStr, start: usize) -> (Option<usize>, usize) {
    let mut len = UNTOUCHED;
    // SAFETY: `start` is at most the line's length, so the pointer is
    // inside the NUL-terminated line; `len` is writable.
    let p = unsafe {
        oracle::oracle_obs_nix_system_info_os_release_value(line.as_ptr().add(start), &mut len)
    };
    (offset(line.as_ptr(), p), len)
}

fn value_shim(line: &CStr, start: usize) -> (Option<usize>, usize) {
    let mut len = UNTOUCHED;
    // SAFETY: as above.
    let p =
        unsafe { shim::obs_nix_system_info_os_release_value(line.as_ptr().add(start), &mut len) };
    (offset(line.as_ptr(), p), len)
}

fn value_core(line: &CStr, start: usize) -> (Option<usize>, usize) {
    let range = core_rules::os_release_value(&line.to_bytes()[start..]);
    (Some(start + range.start), range.len())
}

/// The key, the value start and the value from every position of one line
/// agree on all three sides.
fn check_line(bytes: &[u8]) {
    let line = c_string(bytes);
    let c = field_c(&line);
    assert_eq!(field_shim(&line), c, "field {bytes:?}");
    assert_eq!(field_core(&line), c, "field (core) {bytes:?}");
    for start in 0..=line.to_bytes().len() {
        let c = value_c(&line, start);
        assert_eq!(value_shim(&line, start), c, "value {bytes:?} at {start}");
        assert_eq!(
            value_core(&line, start),
            c,
            "value (core) {bytes:?} at {start}"
        );
    }
}

#[test]
fn os_release_lines_match_c() {
    for line in combos(PIECES, 3) {
        check_line(&line);
    }
    // a NUL ends the line for every check
    check_line(b"NAME=a\0b\n");
    check_line(b"\0NAME=a\n");
    check_line(b"VERSION_ID=\"1\0\"\n");
    // every byte alone and after each key
    for b in 1..=u8::MAX {
        check_line(&[b]);
        for key in [&b"NAME="[..], b"VERSION_ID=", b"BUILD_ID="] {
            let mut line = key.to_vec();
            line.push(b);
            check_line(&line);
            line.push(b);
            check_line(&line);
        }
    }
}

/// The fallback as bytes: `None` for NULL ("copy the BUILD_ID").
fn fallback_c(has_build_id: bool, distro: *const c_char) -> Option<Vec<u8>> {
    // SAFETY: `distro` is NUL-terminated, or NULL with a BUILD_ID, where
    // the C does not read it.
    let p = unsafe { oracle::oracle_obs_nix_system_info_version_fallback(has_build_id, distro) };
    // SAFETY: a non-NULL result is a static NUL-terminated string.
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_bytes().to_vec())
}

fn fallback_shim(has_build_id: bool, distro: *const c_char) -> Option<Vec<u8>> {
    // SAFETY: as above.
    let p = unsafe { shim::obs_nix_system_info_version_fallback(has_build_id, distro) };
    // SAFETY: as above.
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_bytes().to_vec())
}

fn fallback_core(has_build_id: bool, distro: &CStr) -> Option<Vec<u8>> {
    match core_rules::version_fallback(has_build_id, || distro.to_bytes()) {
        core_rules::VersionFallback::BuildId => None,
        core_rules::VersionFallback::Rolling => Some(b"rolling".to_vec()),
        core_rules::VersionFallback::Unknown => Some(b"Unknown".to_vec()),
    }
}

fn check_fallback(bytes: &[u8]) {
    let distro = c_string(bytes);
    for has_build_id in [false, true] {
        let c = fallback_c(has_build_id, distro.as_ptr());
        assert_eq!(fallback_shim(has_build_id, distro.as_ptr()), c, "{bytes:?}");
        assert_eq!(fallback_core(has_build_id, &distro), c, "(core) {bytes:?}");
    }
}

#[test]
fn version_fallbacks_match_c() {
    for distro in combos(PIECES, 2) {
        check_fallback(&distro);
    }
    for distro in [
        &b"Unknown"[..],
        b"unknown",
        b"Unknown ",
        b" Unknown",
        b"Unknow",
        b"",
    ] {
        check_fallback(distro);
    }
    // with a BUILD_ID neither side reads the name
    assert_eq!(fallback_c(true, ptr::null()), None);
    assert_eq!(fallback_shim(true, ptr::null()), None);
}

/// The C's extension name, from an independent model of
/// `strlist_split(ext, '=', false)`'s first entry: the first non-empty
/// piece, or `"(null)"` when there is none.
fn model_c_name(ext: &[u8]) -> Vec<u8> {
    ext.split(|&b| b == b'=')
        .find(|piece| !piece.is_empty())
        .map_or_else(|| b"(null)".to_vec(), <[u8]>::to_vec)
}

/// What `_extension_name()` gives: the name's bytes and `*len`, or `None`
/// for NULL with `*len`.
fn name_c(ext: &CStr) -> (Option<Vec<u8>>, usize) {
    let mut len = UNTOUCHED;
    // SAFETY: a NUL-terminated entry and a writable length.
    let p = unsafe { oracle::oracle_obs_nix_system_info_extension_name(ext.as_ptr(), &mut len) };
    // SAFETY: a non-NULL result points to `len` readable bytes.
    let name =
        (!p.is_null()).then(|| unsafe { std::slice::from_raw_parts(p.cast::<u8>(), len) }.to_vec());
    (name, len)
}

fn name_shim(ext: &CStr) -> (Option<usize>, usize) {
    let mut len = UNTOUCHED;
    // SAFETY: as above.
    let p = unsafe { shim::obs_nix_system_info_extension_name(ext.as_ptr(), &mut len) };
    (offset(ext.as_ptr(), p), len)
}

fn check_entry(bytes: &[u8]) {
    let ext = c_string(bytes);
    let entry = ext.to_bytes();
    let (c_name, c_len) = name_c(&ext);
    // the C side, against the model
    assert_eq!(
        c_name.as_deref(),
        Some(&model_c_name(entry)[..]),
        "{bytes:?}"
    );

    let shim = name_shim(&ext);
    let core = core_rules::extension_name(entry);
    if entry.first().is_none_or(|&b| b == b'=') {
        // the intentional difference (#3473): no name, so no line
        assert_eq!(shim, (None, UNTOUCHED), "{bytes:?}");
        assert_eq!(core, None, "(core) {bytes:?}");
    } else {
        // the name starts the entry, so it is the C's
        assert_eq!(shim, (Some(0), c_len), "{bytes:?}");
        assert_eq!(core, Some(0..c_len), "(core) {bytes:?}");
        assert_eq!(&entry[..c_len], &c_name.expect("a name")[..]);
    }
}

#[test]
fn extension_names_match_c_but_empty_names() {
    for entry in combos(EXT_PIECES, 4) {
        check_entry(&entry);
    }
    for b in 1..=u8::MAX {
        check_entry(&[b]);
        check_entry(&[b, b'=', b]);
        check_entry(&[b'=', b]);
    }
}

#[test]
fn desktop_forms_match_c() {
    for has_current in [false, true] {
        for has_session in [false, true] {
            // SAFETY: plain values.
            let c = unsafe {
                oracle::oracle_obs_nix_system_info_desktop_form(has_current, has_session)
            };
            assert_eq!(
                shim::obs_nix_system_info_desktop_form(has_current, has_session),
                c
            );
            let core = match core_rules::desktop_form(has_current, has_session) {
                core_rules::DesktopForm::None => 0,
                core_rules::DesktopForm::One => 1,
                core_rules::DesktopForm::Both => 2,
            };
            assert_eq!(core, c);
        }
    }
}

type FieldFn = unsafe extern "C" fn(*const c_char, *mut c_int) -> *const c_char;
type ValueFn = unsafe extern "C" fn(*const c_char, *mut usize) -> *const c_char;
type FallbackFn = unsafe extern "C" fn(bool, *const c_char) -> *const c_char;

/// `read_distribution_info()`'s loop through one side's functions: the
/// lines as `getline()` returns them, each up to its NUL, a value copied
/// only when its length is not zero, and the fallback.
fn run_distribution(
    text: &[u8],
    field: FieldFn,
    value: ValueFn,
    fallback: FallbackFn,
) -> (Vec<u8>, Vec<u8>) {
    let mut distro = CString::new("Unknown").expect("no NUL");
    let mut version = Vec::new();
    let mut build_id = Vec::new();
    for raw in text.split_inclusive(|&b| b == b'\n') {
        let line = c_string(raw);
        let mut key = UNTOUCHED_KEY;
        // SAFETY: a NUL-terminated line and a writable key.
        let start = unsafe { field(line.as_ptr(), &mut key) };
        if start.is_null() {
            continue;
        }
        let mut len = UNTOUCHED;
        // SAFETY: `start` points into the line.
        let p = unsafe { value(start, &mut len) };
        if len == 0 {
            continue;
        }
        // SAFETY: `p` points to `len` bytes of the line.
        let text = unsafe { std::slice::from_raw_parts(p.cast::<u8>(), len) }.to_vec();
        match key {
            0 => distro = CString::new(text).expect("no NUL"),
            1 => version = text,
            2 => build_id = text,
            other => panic!("key {other}"),
        }
    }
    if version.is_empty() {
        // SAFETY: a NUL-terminated name.
        let p = unsafe { fallback(!build_id.is_empty(), distro.as_ptr()) };
        version = if p.is_null() {
            build_id
        } else {
            // SAFETY: a static NUL-terminated string.
            unsafe { CStr::from_ptr(p) }.to_bytes().to_vec()
        };
    }
    (distro.into_bytes(), version)
}

fn check_distribution(text: &[u8]) {
    let c = run_distribution(
        text,
        oracle::oracle_obs_nix_system_info_os_release_field,
        oracle::oracle_obs_nix_system_info_os_release_value,
        oracle::oracle_obs_nix_system_info_version_fallback,
    );
    let rust = run_distribution(
        text,
        shim::obs_nix_system_info_os_release_field,
        shim::obs_nix_system_info_os_release_value,
        shim::obs_nix_system_info_version_fallback,
    );
    assert_eq!(rust, c, "{text:?}");
}

#[test]
fn distribution_files_match_c() {
    for text in [
        &b"NAME=\"Arch Linux\"\nBUILD_ID=rolling\n"[..],
        b"NAME='x'\n",
        b"NAME=\"unterminated\n",
        b"NAME=Ubuntu\r\nVERSION_ID=\"26.04\"\r\n",
        b"NAME=Fedora\nVERSION_ID=43",
        b"NAME=\n",
        b"NAME=\"\"\n",
        b"NAME=\"\n",
        b"PRETTY_NAME=\"Arch Linux\"\nNAME =x\n",
        b"NAME=a\nVERSION_ID=1\nVERSION_ID=2\nVERSION_ID=\n",
        b"NAME=Arch\nBUILD_ID=20261013\n",
        b"BUILD_ID=b\n",
        b"",
        b"NAME=Unknown\n",
        b"NAME=a\0b\nVERSION_ID=\"1\0\"\n",
        b"\0NAME=a\n",
    ] {
        check_distribution(text);
    }
    // every file of two lines from three pieces each
    let lines = combos(&PIECES[..14], 2);
    for a in &lines {
        for b in [&b"BUILD_ID=x\n"[..], b"VERSION_ID=\n", b"NAME=y\r\n", b""] {
            let mut text = a.clone();
            text.push(b'\n');
            text.extend_from_slice(b);
            check_distribution(&text);
        }
    }
}

fn bytes_of(pieces: &'static [&'static [u8]], max: usize) -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(prop::sample::select(pieces), 0..max).prop_map(|p| p.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn random_lines_match_c(line in bytes_of(PIECES, 8)) {
        check_line(&line);
    }

    #[test]
    fn random_values_match_c(line in prop::collection::vec(any::<u8>(), 0..24)) {
        check_line(&line);
        check_fallback(&line);
    }

    #[test]
    fn random_files_match_c(
        lines in prop::collection::vec(bytes_of(PIECES, 6), 0..10),
        newline_at_end: bool,
    ) {
        let mut text = lines.join(&b'\n');
        if newline_at_end {
            text.push(b'\n');
        }
        check_distribution(&text);
    }

    #[test]
    fn random_entries_match_c_but_empty_names(ext in bytes_of(EXT_PIECES, 10)) {
        check_entry(&ext);
    }
}
