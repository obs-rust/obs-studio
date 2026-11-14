//! Tier 3: the unchanged extracted C setter against the Rust ABI, on the real
//! private source layout through the production adapters. The canvas rename,
//! the context setters and signal delivery are shared tracing stand-ins; the
//! unchanged actual-library Tier 2 suites cover them. The traces hold every
//! call with its name, every signal with its handler and payload, and the
//! final name and privacy. No intentional differences and no excluded inputs.
use core::ffi::{c_char, c_void};
use std::ffi::{CStr, CString};
use std::sync::Mutex;

use obs_source::ffi::source_set_name::obs_source_set_name;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

/// `OBS_SOURCE_REQUIRES_CANVAS` in obs-source.h.
const REQUIRES_CANVAS: u32 = 1 << 17;

type Setter = unsafe extern "C" fn(*mut c_void, *const c_char);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

static LOCK: Mutex<()> = Mutex::new(());
unsafe extern "C" {
    fn oracle_obs_source_set_name(source: *mut c_void, name: *const c_char);
    fn oracle_source_rename_new(
        name: *const c_char,
        taken: *const c_char,
        flags: u32,
        private: bool,
        flip: bool,
        mutate: bool,
    ) -> *mut c_void;
    fn oracle_source_rename_select(source: *mut c_void);
    fn oracle_source_rename_trace(source: *mut c_void) -> *const c_char;
    fn oracle_source_rename_clear(source: *mut c_void);
    fn oracle_source_rename_free(source: *mut c_void);
    fn oracle_source_rename_null_logs(setter: Setter, get: GetLog, set: SetLog) -> bool;
}

#[derive(Clone, Debug)]
struct Case {
    old: Option<Vec<u8>>,
    taken: Vec<u8>,
    flags: u32,
    private: bool,
    flip: bool,
    mutate: bool,
    names: Vec<Option<Vec<u8>>>,
}

fn c_string(bytes: &[u8]) -> CString {
    CString::new(
        bytes
            .iter()
            .map(|&b| if b == 0 { 1 } else { b })
            .collect::<Vec<_>>(),
    )
    .expect("no interior NUL")
}

fn run(rust: bool, case: &Case) -> Vec<Vec<u8>> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let old = case.old.as_deref().map(c_string);
    let taken = c_string(&case.taken);
    let names: Vec<Option<CString>> = case
        .names
        .iter()
        .map(|name| name.as_deref().map(c_string))
        .collect();
    // SAFETY: the fixture owns a real source layout, copies every name and
    // lives until freed; the stand-ins are selected while the lock is held.
    unsafe {
        let source = oracle_source_rename_new(
            old.as_ref().map_or(core::ptr::null(), |name| name.as_ptr()),
            taken.as_ptr(),
            case.flags,
            case.private,
            case.flip,
            case.mutate,
        );
        assert!(!source.is_null());
        oracle_source_rename_select(source);
        let mut traces = Vec::new();
        for name in &names {
            let name = name
                .as_ref()
                .map_or(core::ptr::null(), |name| name.as_ptr());
            if rust {
                obs_source_set_name(source, name);
            } else {
                oracle_obs_source_set_name(source, name);
            }
            traces.push(
                CStr::from_ptr(oracle_source_rename_trace(source))
                    .to_bytes()
                    .to_vec(),
            );
            oracle_source_rename_clear(source);
        }
        oracle_source_rename_free(source);
        traces
    }
}

fn compare(case: &Case) {
    let original = run(false, case);
    let rust = run(true, case);
    assert_eq!(
        rust.iter()
            .map(|t| String::from_utf8_lossy(t))
            .collect::<Vec<_>>(),
        original
            .iter()
            .map(|t| String::from_utf8_lossy(t))
            .collect::<Vec<_>>(),
        "{case:?}"
    );
    assert_eq!(rust, original, "{case:?}");
}

fn bytes(text: &str) -> Option<Vec<u8>> {
    Some(text.as_bytes().to_vec())
}

#[test]
fn every_path_matches_c() {
    let names = vec![
        bytes("before"),
        bytes("after"),
        bytes("after"),
        None,
        None,
        bytes(""),
        bytes("taken"),
        bytes("taken 2"),
        Some(vec![0xff, 0xfe]),
        bytes("before"),
    ];
    for old in [bytes("before"), None, bytes("")] {
        for flags in [0, REQUIRES_CANVAS, !REQUIRES_CANVAS, u32::MAX] {
            for private in [false, true] {
                for flip in [false, true] {
                    for mutate in [false, true] {
                        compare(&Case {
                            old: old.clone(),
                            taken: b"taken".to_vec(),
                            flags,
                            private,
                            flip,
                            mutate,
                            names: names.clone(),
                        });
                    }
                }
            }
        }
    }
}

#[test]
fn null_source_diagnostics_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::null_source_diagnostics_match_c")) {
        return;
    }
    // SAFETY: the isolated child owns the process log callback and the C probe
    // restores it. Callback pointers use the base shim's opaque pointer ABI.
    unsafe {
        assert!(oracle_source_rename_null_logs(
            obs_source_set_name,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

fn name() -> impl Strategy<Value = Option<Vec<u8>>> {
    prop_oneof![
        Just(None),
        Just(Some(Vec::new())),
        Just(bytes("same")),
        Just(bytes("taken")),
        prop::collection::vec(any::<u8>(), 0..=64).prop_map(Some),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_renames_match_c(
        old in name(),
        taken in prop_oneof![Just(b"taken".to_vec()), prop::collection::vec(any::<u8>(), 0..=8)],
        flags in prop_oneof![Just(0u32), Just(REQUIRES_CANVAS), any::<u32>()],
        private in any::<bool>(),
        flip in any::<bool>(),
        mutate in any::<bool>(),
        names in prop::collection::vec(name(), 0..=8),
    ) {
        compare(&Case { old, taken, flags, private, flip, mutate, names });
    }
}
