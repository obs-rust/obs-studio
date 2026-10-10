//! Tier 3: the config-file C ABI against the original C, compiled as an
//! oracle.
//!
//! Exclusions, not generated here:
//! - A null `config`/`config_t **` to any function. C dereferences or
//!   stores through it unconditionally (e.g. `config_num_sections` calls
//!   `HASH_COUNT` on it). The shim returns the type's zero value; the
//!   cmocka tests always pass a live config.
//! - A null `file` to `config_create`/`config_open`. `os_fopen(NULL)`
//!   returns NULL in C; the shim mirrors the failure. A null `file` to
//!   `config_open` with a non-null `config **` reaches `bstrdup(NULL)`
//!   only on success paths that never happen.
//! - A null `section`/`name` to the getters/setters. C's `config_find_*`
//!   hash lookups take NULL keys harmlessly in most cases but
//!   `config_set_*` would store them; the shim treats null as abort.
//! - Windows path encoding: the core writes the UTF-8 BOM and hands wide
//!   paths to the OS like C. `path_from_bytes` substitutes U+FFFD like C
//!   (MultiByteToWideChar(CP_UTF8, 0) at libobs/util/utf8.c:47 has no
//!   MB_ERR_INVALID_CHARS), but via a different maximal-invalid-subpart
//!   rule, so a pre-existing file named after the resulting U+FFFD
//!   sequence could open on one side and not the other; both return
//!   CONFIG_FILENOTFOUND in the normal case. The oracle host stub uses
//!   the stricter MB_ERR_INVALID_CHARS, which is not libobs behavior.
//!   The oracle runs the POSIX side; paths here are byte strings.
//! - `PTHREAD_MUTEX_RECURSIVE` reentrancy: C locks around calls that can
//!   nest (set_default_* call the setters). Rust holds `Inner` mutably and
//!   never re-locks, which is strictly stronger.
//!
//! `struct config_data` is opaque to C, so there is no public-struct
//! layout to check.

use core::ffi::{CStr, c_char, c_int};
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use obs_c_oracle::config_file::{
    OracleConfig, oracle_config_close, oracle_config_get_bool, oracle_config_get_default_bool,
    oracle_config_get_default_double, oracle_config_get_default_int,
    oracle_config_get_default_string, oracle_config_get_default_uint, oracle_config_get_double,
    oracle_config_get_int, oracle_config_get_section, oracle_config_get_string,
    oracle_config_get_uint, oracle_config_has_default_value, oracle_config_has_user_value,
    oracle_config_num_sections, oracle_config_open, oracle_config_open_defaults,
    oracle_config_open_string, oracle_config_remove_value, oracle_config_save,
    oracle_config_save_safe, oracle_config_set_bool, oracle_config_set_default_bool,
    oracle_config_set_default_double, oracle_config_set_default_int,
    oracle_config_set_default_string, oracle_config_set_default_uint, oracle_config_set_double,
    oracle_config_set_int, oracle_config_set_string, oracle_config_set_uint,
};
use obs_util::ffi::config_file::{
    config_close, config_data, config_get_bool, config_get_default_bool, config_get_default_double,
    config_get_default_int, config_get_default_string, config_get_default_uint, config_get_double,
    config_get_int, config_get_section, config_get_string, config_get_uint,
    config_has_default_value, config_has_user_value, config_num_sections, config_open,
    config_open_defaults, config_open_string, config_remove_value, config_save, config_save_safe,
    config_set_bool, config_set_default_bool, config_set_default_double, config_set_default_int,
    config_set_default_string, config_set_default_uint, config_set_double, config_set_int,
    config_set_string, config_set_uint,
};
use proptest::prelude::*;

/// C callers hand us C strings; anything at/after a NUL is invisible to
/// both sides, so inputs are truncated there.
fn cstr(b: &[u8]) -> CString {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    CString::new(&b[..end]).expect("truncated at first NUL")
}

fn ptr_bytes(p: *const c_char) -> Option<Vec<u8>> {
    if p.is_null() {
        None
    } else {
        // SAFETY: `p` is a NUL-terminated string owned by the live config.
        Some(unsafe { CStr::from_ptr(p) }.to_bytes().to_vec())
    }
}

fn f64_eq(a: f64, b: f64) -> bool {
    (a == b) || (a.is_nan() && b.is_nan() && a.is_sign_negative() == b.is_sign_negative())
}

#[derive(Debug, Clone)]
enum Op {
    SetStr {
        s: Vec<u8>,
        n: Vec<u8>,
        v: Vec<u8>,
    },
    SetInt {
        s: Vec<u8>,
        n: Vec<u8>,
        v: i64,
    },
    SetUint {
        s: Vec<u8>,
        n: Vec<u8>,
        v: u64,
    },
    SetBool {
        s: Vec<u8>,
        n: Vec<u8>,
        v: bool,
    },
    SetDbl {
        s: Vec<u8>,
        n: Vec<u8>,
        v: f64,
    },
    SetDefStr {
        s: Vec<u8>,
        n: Vec<u8>,
        v: Vec<u8>,
    },
    SetDefInt {
        s: Vec<u8>,
        n: Vec<u8>,
        v: i64,
    },
    SetDefUint {
        s: Vec<u8>,
        n: Vec<u8>,
        v: u64,
    },
    SetDefBool {
        s: Vec<u8>,
        n: Vec<u8>,
        v: bool,
    },
    SetDefDbl {
        s: Vec<u8>,
        n: Vec<u8>,
        v: f64,
    },
    Remove {
        s: Vec<u8>,
        n: Vec<u8>,
    },
    /// Not a mutation: full getter probe on (s, n), hits and misses.
    Probe {
        s: Vec<u8>,
        n: Vec<u8>,
    },
}

fn arb_bytes(max: usize) -> BoxedStrategy<Vec<u8>> {
    prop::collection::vec(proptest::arbitrary::any::<u8>(), 0..max).boxed()
}

/// A short name from a tiny alphabet so sections and keys collide, plus
/// bytes that matter to the lexer (spaces, `=`, `]`, `\\`, non-ASCII).
fn arb_name() -> BoxedStrategy<Vec<u8>> {
    prop::collection::vec(
        prop::sample::select(b"abAB =]\\\t#;\xc3\xff".to_vec()),
        0..6,
    )
    .boxed()
}

/// Arbitrary INI text: either raw bytes, or lines drawn from well-formed
/// and malformed shapes (unterminated/empty headers, trailing text after
/// `]`, keys before any section, lines without `=`, empty keys, comments,
/// escapes, CR/LF/CRLF endings, no final newline).
fn arb_ini(max: usize) -> BoxedStrategy<Vec<u8>> {
    let line = prop_oneof![
        arb_name().prop_map(|n| [b"[".as_slice(), &n, b"]"].concat()),
        arb_name().prop_map(|n| [b"[".as_slice(), &n].concat()),
        Just(b"[]".to_vec()),
        (arb_name(), arb_name()).prop_map(|(n, j)| [b"[".as_slice(), &n, b"]", &j].concat()),
        (arb_name(), arb_bytes(24)).prop_map(|(k, v)| [k.as_slice(), b"=", &v].concat()),
        arb_name(),
        arb_bytes(24).prop_map(|v| [b"=".as_slice(), &v].concat()),
        arb_name().prop_map(|c| [b"#".as_slice(), &c].concat()),
        arb_name().prop_map(|c| [b";".as_slice(), &c].concat()),
        arb_name().prop_map(|k| [k.as_slice(), b"=a\\nb\\\\c\\rd\\xe\\"].concat()),
        Just(Vec::new()),
        arb_bytes(32),
    ];
    let eol = prop::sample::select(vec![b"\n".as_slice(), b"\r\n", b"\r", b""]);
    let lines = prop::collection::vec((line, eol), 0..48).prop_map(move |ls| {
        let mut out = Vec::new();
        for (l, e) in ls {
            out.extend_from_slice(&l);
            out.extend_from_slice(e);
        }
        out.truncate(max);
        out
    });
    prop_oneof![1 => arb_bytes(max), 4 => lines].boxed()
}

fn arb_op() -> BoxedStrategy<Op> {
    let key = (arb_bytes(48), arb_bytes(48));
    prop_oneof![
        (key.clone(), arb_bytes(200)).prop_map(|((s, n), v)| Op::SetStr { s, n, v }),
        (key.clone(), proptest::arbitrary::any::<i64>()).prop_map(|((s, n), v)| Op::SetInt {
            s,
            n,
            v
        }),
        (key.clone(), proptest::arbitrary::any::<u64>()).prop_map(|((s, n), v)| Op::SetUint {
            s,
            n,
            v
        }),
        (key.clone(), proptest::arbitrary::any::<bool>()).prop_map(|((s, n), v)| Op::SetBool {
            s,
            n,
            v
        }),
        (key.clone(), proptest::arbitrary::any::<f64>()).prop_map(|((s, n), v)| Op::SetDbl {
            s,
            n,
            v
        }),
        (key.clone(), arb_bytes(200)).prop_map(|((s, n), v)| Op::SetDefStr { s, n, v }),
        (key.clone(), proptest::arbitrary::any::<i64>()).prop_map(|((s, n), v)| Op::SetDefInt {
            s,
            n,
            v
        }),
        (key.clone(), proptest::arbitrary::any::<u64>()).prop_map(|((s, n), v)| Op::SetDefUint {
            s,
            n,
            v
        }),
        (key.clone(), proptest::arbitrary::any::<bool>()).prop_map(|((s, n), v)| Op::SetDefBool {
            s,
            n,
            v
        }),
        (key.clone(), proptest::arbitrary::any::<f64>()).prop_map(|((s, n), v)| Op::SetDefDbl {
            s,
            n,
            v
        }),
        key.clone().prop_map(|(s, n)| Op::Remove { s, n }),
        key.prop_map(|(s, n)| Op::Probe { s, n }),
    ]
    .boxed()
}

static SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "obs-config-parity-{}-{}-{}",
        tag,
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn read(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// Both implementations of one live config, created through the same
/// kind of call (open_string / open / create).
struct Pair {
    oracle: *mut OracleConfig,
    rust: *mut config_data,
}

impl Pair {
    fn from_strings(text: &[u8]) -> (Pair, c_int, c_int) {
        let t = cstr(text);
        let mut o: *mut OracleConfig = core::ptr::null_mut();
        let mut r: *mut config_data = core::ptr::null_mut();
        // SAFETY: out-pointers are live for the calls; `t` outlives them.
        let oc = unsafe { oracle_config_open_string(&mut o, t.as_ptr()) };
        // SAFETY: same as above.
        let rc = unsafe { config_open_string(&mut r, t.as_ptr()) };
        (Pair { oracle: o, rust: r }, oc, rc)
    }

    fn open_files(c_path: &[u8], r_path: &[u8], open_type: c_int) -> (Pair, c_int, c_int) {
        let cp = cstr(c_path);
        let rp = cstr(r_path);
        let mut o: *mut OracleConfig = core::ptr::null_mut();
        let mut r: *mut config_data = core::ptr::null_mut();
        // SAFETY: out-pointers live; path strings outlive the calls.
        let oc = unsafe { oracle_config_open(&mut o, cp.as_ptr(), open_type) };
        // SAFETY: same as above.
        let rc = unsafe { config_open(&mut r, rp.as_ptr(), open_type) };
        (Pair { oracle: o, rust: r }, oc, rc)
    }

    fn num_sections(&self) -> (usize, usize) {
        // SAFETY: both pointers are live configs from a successful open.
        let o = unsafe { oracle_config_num_sections(self.oracle) };
        // SAFETY: same as above.
        let r = unsafe { config_num_sections(self.rust) };
        (o, r)
    }

    fn section_name(&self, idx: usize) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
        // SAFETY: both pointers are live configs; `idx` in range checked by caller.
        let o = unsafe { oracle_config_get_section(self.oracle, idx) };
        // SAFETY: same as above.
        let r = unsafe { config_get_section(self.rust, idx) };
        (ptr_bytes(o), ptr_bytes(r))
    }

    fn save(&self) -> (c_int, c_int) {
        // SAFETY: both pointers are live configs.
        let o = unsafe { oracle_config_save(self.oracle) };
        // SAFETY: same as above.
        let r = unsafe { config_save(self.rust) };
        (o, r)
    }

    fn save_safe(&self, temp: &[u8], backup: &[u8]) -> (c_int, c_int) {
        let t = cstr(temp);
        let b = cstr(backup);
        // SAFETY: both pointers are live configs; ext strings outlive the calls.
        let o = unsafe { oracle_config_save_safe(self.oracle, t.as_ptr(), b.as_ptr()) };
        // SAFETY: same as above.
        let r = unsafe { config_save_safe(self.rust, t.as_ptr(), b.as_ptr()) };
        (o, r)
    }

    /// Run one mutation/probe on both configs and compare every return.
    fn apply(&self, op: &Op) {
        match *op {
            Op::SetStr {
                ref s,
                ref n,
                ref v,
            } => {
                let (s, n, v) = (cstr(s), cstr(n), cstr(v));
                // SAFETY: live configs; C strings outlive the calls.
                unsafe {
                    oracle_config_set_string(self.oracle, s.as_ptr(), n.as_ptr(), v.as_ptr())
                };
                // SAFETY: same as above.
                unsafe { config_set_string(self.rust, s.as_ptr(), n.as_ptr(), v.as_ptr()) };
            }
            Op::SetInt { ref s, ref n, v } => {
                let (s, n) = (cstr(s), cstr(n));
                // SAFETY: live configs; C strings outlive the calls.
                unsafe { oracle_config_set_int(self.oracle, s.as_ptr(), n.as_ptr(), v) };
                // SAFETY: same as above.
                unsafe { config_set_int(self.rust, s.as_ptr(), n.as_ptr(), v) };
            }
            Op::SetUint { ref s, ref n, v } => {
                let (s, n) = (cstr(s), cstr(n));
                // SAFETY: live configs; C strings outlive the calls.
                unsafe { oracle_config_set_uint(self.oracle, s.as_ptr(), n.as_ptr(), v) };
                // SAFETY: same as above.
                unsafe { config_set_uint(self.rust, s.as_ptr(), n.as_ptr(), v) };
            }
            Op::SetBool { ref s, ref n, v } => {
                let (s, n) = (cstr(s), cstr(n));
                // SAFETY: live configs; C strings outlive the calls.
                unsafe { oracle_config_set_bool(self.oracle, s.as_ptr(), n.as_ptr(), v) };
                // SAFETY: same as above.
                unsafe { config_set_bool(self.rust, s.as_ptr(), n.as_ptr(), v) };
            }
            Op::SetDbl { ref s, ref n, v } => {
                let (s, n) = (cstr(s), cstr(n));
                // SAFETY: live configs; C strings outlive the calls.
                unsafe { oracle_config_set_double(self.oracle, s.as_ptr(), n.as_ptr(), v) };
                // SAFETY: same as above.
                unsafe { config_set_double(self.rust, s.as_ptr(), n.as_ptr(), v) };
            }
            Op::SetDefStr {
                ref s,
                ref n,
                ref v,
            } => {
                let (s, n, v) = (cstr(s), cstr(n), cstr(v));
                // SAFETY: live configs; C strings outlive the calls.
                unsafe {
                    oracle_config_set_default_string(
                        self.oracle,
                        s.as_ptr(),
                        n.as_ptr(),
                        v.as_ptr(),
                    )
                };
                // SAFETY: same as above.
                unsafe { config_set_default_string(self.rust, s.as_ptr(), n.as_ptr(), v.as_ptr()) };
            }
            Op::SetDefInt { ref s, ref n, v } => {
                let (s, n) = (cstr(s), cstr(n));
                // SAFETY: live configs; C strings outlive the calls.
                unsafe { oracle_config_set_default_int(self.oracle, s.as_ptr(), n.as_ptr(), v) };
                // SAFETY: same as above.
                unsafe { config_set_default_int(self.rust, s.as_ptr(), n.as_ptr(), v) };
            }
            Op::SetDefUint { ref s, ref n, v } => {
                let (s, n) = (cstr(s), cstr(n));
                // SAFETY: live configs; C strings outlive the calls.
                unsafe { oracle_config_set_default_uint(self.oracle, s.as_ptr(), n.as_ptr(), v) };
                // SAFETY: same as above.
                unsafe { config_set_default_uint(self.rust, s.as_ptr(), n.as_ptr(), v) };
            }
            Op::SetDefBool { ref s, ref n, v } => {
                let (s, n) = (cstr(s), cstr(n));
                // SAFETY: live configs; C strings outlive the calls.
                unsafe { oracle_config_set_default_bool(self.oracle, s.as_ptr(), n.as_ptr(), v) };
                // SAFETY: same as above.
                unsafe { config_set_default_bool(self.rust, s.as_ptr(), n.as_ptr(), v) };
            }
            Op::SetDefDbl { ref s, ref n, v } => {
                let (s, n) = (cstr(s), cstr(n));
                // SAFETY: live configs; C strings outlive the calls.
                unsafe { oracle_config_set_default_double(self.oracle, s.as_ptr(), n.as_ptr(), v) };
                // SAFETY: same as above.
                unsafe { config_set_default_double(self.rust, s.as_ptr(), n.as_ptr(), v) };
            }
            Op::Remove { ref s, ref n } => {
                let (s, n) = (cstr(s), cstr(n));
                // SAFETY: live configs; C strings outlive the calls.
                let o = unsafe { oracle_config_remove_value(self.oracle, s.as_ptr(), n.as_ptr()) };
                // SAFETY: same as above.
                let r = unsafe { config_remove_value(self.rust, s.as_ptr(), n.as_ptr()) };
                assert_eq!(o, r, "remove_value {s:?} {n:?}");
            }
            Op::Probe { ref s, ref n } => {
                let (s, n) = (cstr(s), cstr(n));
                self.probe(&s, &n);
            }
        }
    }

    /// Compare every getter on (s, n): string, three numeric types, bool,
    /// the five default getters, and both has_* flags.
    fn probe(&self, s: &CString, n: &CString) {
        let (sp, np) = (s.as_ptr(), n.as_ptr());
        // SAFETY: live configs; C strings outlive every call here.
        let (os, rs) = unsafe {
            (
                oracle_config_get_string(self.oracle, sp, np),
                config_get_string(self.rust, sp, np),
            )
        };
        assert_eq!(ptr_bytes(os), ptr_bytes(rs), "get_string {s:?} {n:?}");

        // SAFETY: live configs; C strings outlive the calls.
        let (oi, ri) = unsafe {
            (
                oracle_config_get_int(self.oracle, sp, np),
                config_get_int(self.rust, sp, np),
            )
        };
        assert_eq!(oi, ri, "get_int {s:?} {n:?}");

        // SAFETY: live configs; C strings outlive the calls.
        let (ou, ru) = unsafe {
            (
                oracle_config_get_uint(self.oracle, sp, np),
                config_get_uint(self.rust, sp, np),
            )
        };
        assert_eq!(ou, ru, "get_uint {s:?} {n:?}");

        // SAFETY: live configs; C strings outlive the calls.
        let (ob, rb) = unsafe {
            (
                oracle_config_get_bool(self.oracle, sp, np),
                config_get_bool(self.rust, sp, np),
            )
        };
        assert_eq!(ob, rb, "get_bool {s:?} {n:?}");

        // SAFETY: live configs; C strings outlive the calls.
        let (od, rd) = unsafe {
            (
                oracle_config_get_double(self.oracle, sp, np),
                config_get_double(self.rust, sp, np),
            )
        };
        assert!(f64_eq(od, rd), "get_double {s:?} {n:?}: {od} != {rd}");

        // SAFETY: live configs; C strings outlive the calls.
        let (ods, rds) = unsafe {
            (
                oracle_config_get_default_string(self.oracle, sp, np),
                config_get_default_string(self.rust, sp, np),
            )
        };
        assert_eq!(
            ptr_bytes(ods),
            ptr_bytes(rds),
            "get_default_string {s:?} {n:?}"
        );

        // SAFETY: live configs; C strings outlive the calls.
        let (odi, rdi) = unsafe {
            (
                oracle_config_get_default_int(self.oracle, sp, np),
                config_get_default_int(self.rust, sp, np),
            )
        };
        assert_eq!(odi, rdi, "get_default_int {s:?} {n:?}");

        // SAFETY: live configs; C strings outlive the calls.
        let (odu, rdu) = unsafe {
            (
                oracle_config_get_default_uint(self.oracle, sp, np),
                config_get_default_uint(self.rust, sp, np),
            )
        };
        assert_eq!(odu, rdu, "get_default_uint {s:?} {n:?}");

        // SAFETY: live configs; C strings outlive the calls.
        let (odb, rdb) = unsafe {
            (
                oracle_config_get_default_bool(self.oracle, sp, np),
                config_get_default_bool(self.rust, sp, np),
            )
        };
        assert_eq!(odb, rdb, "get_default_bool {s:?} {n:?}");

        // SAFETY: live configs; C strings outlive the calls.
        let (odd, rdd) = unsafe {
            (
                oracle_config_get_default_double(self.oracle, sp, np),
                config_get_default_double(self.rust, sp, np),
            )
        };
        assert!(
            f64_eq(odd, rdd),
            "get_default_double {s:?} {n:?}: {odd} != {rdd}"
        );

        // SAFETY: live configs; C strings outlive the calls.
        let (ohu, rhu) = unsafe {
            (
                oracle_config_has_user_value(self.oracle, sp, np),
                config_has_user_value(self.rust, sp, np),
            )
        };
        assert_eq!(ohu, rhu, "has_user_value {s:?} {n:?}");

        // SAFETY: live configs; C strings outlive the calls.
        let (ohd, rhd) = unsafe {
            (
                oracle_config_has_default_value(self.oracle, sp, np),
                config_has_default_value(self.rust, sp, np),
            )
        };
        assert_eq!(ohd, rhd, "has_default_value {s:?} {n:?}");
    }

    fn check_sections(&self) {
        let (on, rn) = self.num_sections();
        assert_eq!(on, rn, "num_sections");
        for i in 0..on {
            assert_eq!(self.section_name(i), self.section_name(i), "section {i}");
            let (o, r) = self.section_name(i);
            assert_eq!(o, r, "get_section {i}");
        }
    }
}

impl Drop for Pair {
    fn drop(&mut self) {
        // SAFETY: each config is closed exactly once, here.
        unsafe { oracle_config_close(self.oracle) };
        // SAFETY: same as above.
        unsafe { config_close(self.rust) };
    }
}

fn cfg_path(dir: &Path, name: &str) -> (PathBuf, Vec<u8>) {
    let p = dir.join(name);
    (p.clone(), p.as_os_str().as_encoded_bytes().to_vec())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(192))]

    /// Arbitrary INI text through open_string: same sections, same names,
    /// same values reachable through the getters.
    #[test]
    fn parity_open_string(text in arb_ini(4096)) {
        let (pair, oc, rc) = Pair::from_strings(&text);
        assert_eq!(oc, rc, "open_string return");
        pair.check_sections();

        // Sections may contain duplicate names/values; probe a few
        // structural queries and malformed-input results.
        let (os, rs) = pair.save();
        assert_eq!(os, rs, "save on a string config");
    }

    /// Arbitrary INI file: open both, mutate identically, compare every
    /// observable return and then the saved bytes in separate temp dirs.
    #[test]
    fn parity_open_mutate_save(
        text in arb_ini(4096),
        ops in prop::collection::vec(arb_op(), 0..24),
    ) {
        let c_dir = scratch("c");
        let r_dir = scratch("r");
        let (c_path, c_bytes) = cfg_path(&c_dir, "cfg.ini");
        let (r_path, r_bytes) = cfg_path(&r_dir, "cfg.ini");
        std::fs::write(&c_path, &text).unwrap();
        std::fs::write(&r_path, &text).unwrap();

        let (pair, oc, rc) = Pair::open_files(&c_bytes, &r_bytes, 0);
        assert_eq!(oc, rc, "config_open return");
        prop_assert_eq!(oc, 0, "config_open should succeed on a real file");

        pair.check_sections();
        for op in &ops {
            pair.apply(op);
        }
        pair.check_sections();

        let (os, rs) = pair.save();
        assert_eq!(os, rs, "config_save return");
        prop_assert_eq!(os, 0);
        let c_saved = read(&c_path).expect("C wrote its file");
        let r_saved = read(&r_path).expect("Rust wrote its file");
        assert_eq!(c_saved, r_saved, "saved bytes");

        let (os2, rs2) = pair.save_safe(b"tmp", b"bak");
        assert_eq!(os2, rs2, "config_save_safe return");
        assert_eq!(read(&c_path), read(&r_path), "save_safe result bytes");
        assert_eq!(
            read(&c_dir.join("cfg.ini.tmp")),
            read(&r_dir.join("cfg.ini.tmp")),
            "save_safe temp side effect",
        );
        assert_eq!(
            read(&c_dir.join("cfg.ini.bak")),
            read(&r_dir.join("cfg.ini.bak")),
            "save_safe backup side effect",
        );
    }

    /// Defaults file plus ops: get_default_* and has_default_value must
    /// agree on every probed key, and set_default_* writes land in the
    /// user section identically (the C quirk is part of the contract).
    #[test]
    fn parity_defaults(
        defaults in arb_ini(2048),
        ops in prop::collection::vec(arb_op(), 1..24),
        probes in prop::collection::vec((arb_name(), arb_name()), 0..24),
    ) {
        let c_dir = scratch("cdef");
        let r_dir = scratch("rdef");
        let (c_path, c_bytes) = cfg_path(&c_dir, "cfg.ini");
        let (r_path, r_bytes) = cfg_path(&r_dir, "cfg.ini");
        let (cd_path, cd_bytes) = cfg_path(&c_dir, "def.ini");
        let (rd_path, rd_bytes) = cfg_path(&r_dir, "def.ini");
        std::fs::write(&c_path, b"").unwrap();
        std::fs::write(&r_path, b"").unwrap();
        std::fs::write(&cd_path, &defaults).unwrap();
        std::fs::write(&rd_path, &defaults).unwrap();

        let (pair, oc, rc) = Pair::open_files(&c_bytes, &r_bytes, 1);
        assert_eq!(oc, rc, "config_open always");
        prop_assert_eq!(oc, 0);

        // SAFETY: live configs; defaults paths are real files.
        let od = unsafe { oracle_config_open_defaults(pair.oracle, cstr(&cd_bytes).as_ptr()) };
        // SAFETY: same as above.
        let rd = unsafe { config_open_defaults(pair.rust, cstr(&rd_bytes).as_ptr()) };
        assert_eq!(od, rd, "open_defaults return");

        for op in &ops {
            pair.apply(op);
        }
        for (s, n) in &probes {
            pair.probe(&cstr(s), &cstr(n));
        }

        let (os, rs) = pair.save();
        assert_eq!(os, rs, "config_save return");
        assert_eq!(read(&c_path), read(&r_path), "saved bytes");
    }
}

/// A null `str` to `config_open_string`: C treats it as an empty parse
/// and succeeds (`lexer_start(NULL)` walks nothing).
#[test]
fn open_string_null_succeeds_both() {
    let mut o: *mut OracleConfig = core::ptr::null_mut();
    let mut r: *mut config_data = core::ptr::null_mut();
    // SAFETY: out-pointers are live; null str is a documented C case.
    let oc = unsafe { oracle_config_open_string(&mut o, core::ptr::null()) };
    // SAFETY: same as above.
    let rc = unsafe { config_open_string(&mut r, core::ptr::null()) };
    assert_eq!(oc, rc);
    assert_eq!(oc, 0);
    // SAFETY: each config is closed exactly once.
    unsafe {
        oracle_config_close(o);
        config_close(r);
    }
}

/// `config_open` on a missing file: FILENOTFOUND for both.
#[test]
fn open_missing_file_matches() {
    let dir = scratch("missing");
    let (c_path, c_bytes) = cfg_path(&dir, "no-such-c.ini");
    let (r_path, r_bytes) = cfg_path(&dir, "no-such-r.ini");
    let (pair, oc, rc) = Pair::open_files(&c_bytes, &r_bytes, 0);
    assert_eq!(oc, rc);
    assert_eq!(oc, -1, "CONFIG_FILENOTFOUND");
    let _ = (pair, c_path, r_path);
}

/// `config_save_safe` with a null/empty temp_ext: CONFIG_ERROR for both.
#[test]
fn save_safe_bad_ext_matches() {
    let dir = scratch("badext");
    let (c_path, c_bytes) = cfg_path(&dir, "c.ini");
    let (r_path, r_bytes) = cfg_path(&dir, "r.ini");
    std::fs::write(&c_path, b"[a]\nx=1\n").unwrap();
    std::fs::write(&r_path, b"[a]\nx=1\n").unwrap();
    let (pair, oc, rc) = Pair::open_files(&c_bytes, &r_bytes, 0);
    assert_eq!(oc, rc);
    assert_eq!(oc, 0);

    // SAFETY: live configs; null temp_ext is the documented error case.
    let o = unsafe { oracle_config_save_safe(pair.oracle, core::ptr::null(), core::ptr::null()) };
    // SAFETY: same as above.
    let r = unsafe { config_save_safe(pair.rust, core::ptr::null(), core::ptr::null()) };
    assert_eq!(o, r);
    assert_eq!(o, -2, "CONFIG_ERROR");
}

/// os_dtostr's exponent handling across magnitudes: `+` exponents keep
/// the trailing duplicate digit, `-` exponents lose leading zeros.
#[test]
fn dtostr_values() {
    let vals: [f64; 12] = [
        3.918_947_176_923_697e151,
        1e18,
        1e100,
        1.5e-5,
        2.5e-300,
        f64::from_bits(1),
        1.0,
        0.1,
        123_456_789_012_345.6,
        1e16,
        1e-4,
        9.999e-5,
    ];
    let (pair, oc, rc) = Pair::from_strings(b"");
    assert_eq!(oc, rc);
    let s = cstr(b"S");
    for (i, v) in vals.iter().enumerate() {
        let n = cstr(format!("k{i}").as_bytes());
        // SAFETY: live configs; C strings outlive the calls.
        unsafe {
            oracle_config_set_double(pair.oracle, s.as_ptr(), n.as_ptr(), *v);
            config_set_double(pair.rust, s.as_ptr(), n.as_ptr(), *v);
            let o = oracle_config_get_string(pair.oracle, s.as_ptr(), n.as_ptr());
            let r = config_get_string(pair.rust, s.as_ptr(), n.as_ptr());
            assert_eq!(ptr_bytes(o), ptr_bytes(r), "dtostr({v:e})");
        }
    }
}
