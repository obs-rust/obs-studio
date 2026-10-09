//! Tier 3: the Rust C ABI shim and safe core behave exactly like the original
//! C, compiled as an oracle.
//!
//! Intentional difference from C: a token whose opening quote is the last
//! character on its line (`Key="` directly followed by a newline or the end of
//! the file) makes the C length wrap to `SIZE_MAX` and crash. Rust treats such
//! a token as empty (which ends the parse, like `Key=""` in C). Those inputs
//! are excluded from the comparison by [`hits_c_crash`].
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::{CStr, CString, c_char, c_void};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use obs_c_oracle::text_lookup as c;
use obs_util::ffi::text_lookup as rs;
use obs_util::text_lookup::TextLookup;
use proptest::prelude::*;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A temp file that is removed on drop.
struct TempFile {
    path: PathBuf,
    cpath: CString,
}

impl TempFile {
    fn new(data: &[u8]) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "obs_rust_text_lookup_parity_{}_{}.ini",
            std::process::id(),
            n
        ));
        std::fs::write(&path, data).unwrap();
        let cpath = CString::new(path.to_str().unwrap()).unwrap();
        TempFile { path, cpath }
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// True if the C code would crash on `data` (the file contents): an opening
/// quote token directly followed by a newline or the end of the data.
///
/// This mirrors what `text_lookup_add` feeds the parser (BOM stripped, data
/// cut at the first NUL, CR turned into a space). An opening quote token can
/// only directly follow the start of the data, a whitespace token, `=` or a
/// closing `"`; any other preceding character makes the quote part of a
/// longer token, and a `#` always starts a comment (or sits inside a string
/// token), so a quote after it is never an opening token. The predicate is a
/// superset (it also rejects some closing quotes, e.g. `Key=""` at the end of
/// a line), never a subset.
fn hits_c_crash(data: &[u8]) -> bool {
    let body = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let end = body.iter().position(|&b| b == 0).unwrap_or(body.len());
    let body: Vec<u8> = body[..end]
        .iter()
        .map(|&b| if b == b'\r' { b' ' } else { b })
        .collect();

    body.iter().enumerate().any(|(i, &b)| {
        b == b'"'
            && body.get(i + 1).is_none_or(|&n| n == b'\n')
            && (i == 0 || matches!(body[i - 1], b' ' | b'\t' | b'\n' | b'=' | b'"'))
    })
}

const KEYS: &[&str] = &["Key", "Plain", "Esc", "A", "B1", "Dup", "Name", "x"];

fn piece() -> impl Strategy<Value = Vec<u8>> {
    let fixed: &[&'static [u8]] = &[
        b"=",
        b"\"",
        b"\\\"",
        b"\\n",
        b"\\t",
        b"\\r",
        b"\\\\",
        b"\n",
        b"\r\n",
        b"\r",
        b"\n\r",
        b" ",
        b"\t",
        b"#",
        b"# comment",
        b"12",
        b".",
        b",",
        b"\0",
        b"\xEF\xBB\xBF",
    ];
    prop_oneof![
        3 => prop::sample::select(fixed.to_vec()).prop_map(|b| b.to_vec()),
        3 => prop::sample::select(KEYS).prop_map(|k| k.as_bytes().to_vec()),
        2 => prop::collection::vec(any::<u8>(), 1..4),
        // a well-formed entry line
        4 => (
            prop::sample::select(KEYS),
            prop::collection::vec(
                any::<u8>().prop_filter("no quote/newline", |b| !matches!(b, b'"' | b'\n' | 0)),
                0..8,
            ),
        )
            .prop_map(|(k, v)| {
                let mut out = k.as_bytes().to_vec();
                out.extend_from_slice(b"=\"");
                out.extend_from_slice(&v);
                out.extend_from_slice(b"\"\n");
                out
            }),
    ]
}

fn contents() -> impl Strategy<Value = Vec<u8>> {
    (any::<bool>(), prop::collection::vec(piece(), 0..40)).prop_map(|(bom, pieces)| {
        let mut data = if bom {
            b"\xEF\xBB\xBF".to_vec()
        } else {
            Vec::new()
        };
        for p in pieces {
            data.extend_from_slice(&p);
        }
        data
    })
}

/// Every key the content could define, plus the fixed pool.
fn candidate_keys(data: &[u8], extra: &[String]) -> Vec<CString> {
    let mut keys: Vec<Vec<u8>> = KEYS.iter().map(|k| k.as_bytes().to_vec()).collect();
    keys.extend(
        data.split(|b| !b.is_ascii_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(<[u8]>::to_vec),
    );
    keys.extend(extra.iter().map(|s| s.as_bytes().to_vec()));
    keys.push(b"key".to_vec()); // case sensitivity
    keys.into_iter()
        .filter_map(|k| CString::new(k).ok())
        .collect()
}

/// # Safety
///
/// `lookup` and `lookup_val` must be valid for the respective implementation.
unsafe fn getstr_both(
    rs_lookup: *mut c_void,
    c_lookup: *mut c_void,
    key: &CStr,
) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let mut rs_out: *const c_char = core::ptr::null();
    let mut c_out: *const c_char = core::ptr::null();
    // SAFETY: both lookups are live and `key` is NUL-terminated.
    let (ok_rs, ok_c) = unsafe {
        (
            rs::text_lookup_getstr(rs_lookup, key.as_ptr(), &mut rs_out),
            c::oracle_text_lookup_getstr(c_lookup, key.as_ptr(), &mut c_out),
        )
    };
    // *out is only written on success.
    assert_eq!(ok_rs, !rs_out.is_null());
    assert_eq!(ok_c, !c_out.is_null());
    // SAFETY: on success the pointers are NUL-terminated and live until destroy.
    let value =
        |ok: bool, p: *const c_char| ok.then(|| unsafe { CStr::from_ptr(p) }.to_bytes().to_vec());
    (value(ok_rs, rs_out), value(ok_c, c_out))
}

proptest! {
    #![proptest_config(ProptestConfig {
        max_global_rejects: 1_000_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn matches_c_oracle(
        first in contents(),
        second in contents(),
        extra in prop::collection::vec("[A-Za-z0-9 ]{0,6}", 0..6),
    ) {
        // Intentional difference: skip inputs where C crashes.
        prop_assume!(!hits_c_crash(&first) && !hits_c_crash(&second));

        let f1 = TempFile::new(&first);
        let f2 = TempFile::new(&second);

        // SAFETY: the paths are valid NUL-terminated strings.
        let (rs_lookup, c_lookup) = unsafe {
            (
                rs::text_lookup_create(f1.cpath.as_ptr()),
                c::oracle_text_lookup_create(f1.cpath.as_ptr()),
            )
        };
        prop_assert_eq!(rs_lookup.is_null(), c_lookup.is_null());

        // The safe core agrees with the shim.
        let mut safe = TextLookup::new();
        prop_assert_eq!(safe.add_file(&f1.path), !c_lookup.is_null());

        let mut keys = candidate_keys(&first, &extra);
        keys.extend(candidate_keys(&second, &extra));

        if !c_lookup.is_null() {
            for key in &keys {
                // SAFETY: both lookups are live.
                let (a, b) = unsafe { getstr_both(rs_lookup, c_lookup, key) };
                prop_assert_eq!(&a, &b, "key {:?}", key);
                prop_assert_eq!(safe.get(key).map(|v| v.to_bytes().to_vec()), b);
            }

            // SAFETY: both lookups are live; paths are NUL-terminated.
            let (add_rs, add_c) = unsafe {
                (
                    rs::text_lookup_add(rs_lookup, f2.cpath.as_ptr()),
                    c::oracle_text_lookup_add(c_lookup, f2.cpath.as_ptr()),
                )
            };
            prop_assert_eq!(add_rs, add_c);
            prop_assert_eq!(safe.add_file(&f2.path), add_c);

            for key in &keys {
                // SAFETY: both lookups are live.
                let (a, b) = unsafe { getstr_both(rs_lookup, c_lookup, key) };
                prop_assert_eq!(&a, &b, "key {:?} after add", key);
                prop_assert_eq!(safe.get(key).map(|v| v.to_bytes().to_vec()), b);
            }
        }

        // A missing path fails identically.
        let missing = CString::new("/nonexistent-obs-rust-dir/none.ini").unwrap();
        // SAFETY: valid NUL-terminated path; destroy accepts NULL and live lookups.
        unsafe {
            prop_assert_eq!(
                rs::text_lookup_create(missing.as_ptr()).is_null(),
                c::oracle_text_lookup_create(missing.as_ptr()).is_null()
            );
            rs::text_lookup_destroy(rs_lookup);
            c::oracle_text_lookup_destroy(c_lookup);
        }
    }
}

/// Tier 2: NULL handling of the shim.
#[test]
fn shim_null_lookup() {
    let key = CString::new("Key").unwrap();
    let mut out: *const c_char = core::ptr::null();
    // SAFETY: NULL lookups are accepted; `key` is NUL-terminated.
    unsafe {
        assert!(!rs::text_lookup_getstr(
            core::ptr::null_mut(),
            key.as_ptr(),
            &mut out
        ));
        assert!(out.is_null());
        rs::text_lookup_destroy(core::ptr::null_mut());
        assert!(!c::oracle_text_lookup_getstr(
            core::ptr::null_mut(),
            key.as_ptr(),
            &mut out
        ));
        assert!(out.is_null());
        c::oracle_text_lookup_destroy(core::ptr::null_mut());
    }
}
