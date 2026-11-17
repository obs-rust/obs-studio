//! Exact selection and callback traces against the unchanged extracted C.
//! Codec bytes are ASCII or NULL: negative signed chars passed to C toupper
//! have undefined behavior and are intentionally excluded.
use core::ffi::{CStr, c_char, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_reroute::{RerouteFields, obs_encoder_create_rerouted};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Run = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_obs_encoder_create_rerouted(encoder: *mut c_void, id: *const c_char) -> *mut c_void;
    fn oracle_encoder_reroute_layout(out: *mut usize);
    fn oracle_encoder_reroute_fields_match() -> bool;
    fn oracle_encoder_reroute_run(
        run: Run,
        flags: u32,
        original_type: i32,
        replacement_type: i32,
        original_codec: *const c_char,
        replacement_codec: *const c_char,
        caps: u32,
    ) -> u64;
    fn oracle_encoder_reroute_logs(run: Run, get: GetLog, set: SetLog) -> bool;
}

// Trace bits from oracle_encoder_reroute_run.
const RETURNED: u64 = 1;
const ONE_CALL: u64 = 2;
const CALLBACK_SAW_TABLE: u64 = 8;
const ACTIVE_REPLACED: u64 = 16;
const ORIGINAL_KEPT: u64 = 32;
const ACTIVE_KEPT: u64 = 64;
const REGISTRY_KEPT: u64 = 128;

#[test]
fn real_header_layout() {
    let mut out = [0; 11];
    // SAFETY: eleven writable size_t slots.
    unsafe { oracle_encoder_reroute_layout(out.as_mut_ptr()) };
    assert_eq!(
        out,
        [
            size_of::<RerouteFields>(),
            align_of::<RerouteFields>(),
            offset_of!(RerouteFields, active),
            offset_of!(RerouteFields, replacement),
            offset_of!(RerouteFields, size),
            offset_of!(RerouteFields, original_type),
            offset_of!(RerouteFields, replacement_type),
            offset_of!(RerouteFields, original_codec),
            offset_of!(RerouteFields, replacement_codec),
            offset_of!(RerouteFields, settings),
            offset_of!(RerouteFields, create)
        ]
    );
    // SAFETY: the probe owns its stack fixtures.
    assert!(unsafe { oracle_encoder_reroute_fields_match() });
}

fn check(
    flags: u32,
    original: i32,
    replacement: i32,
    a: Option<&CStr>,
    b: Option<&CStr>,
    caps: u32,
) -> u64 {
    let a = a.map_or(core::ptr::null(), CStr::as_ptr);
    let b = b.map_or(core::ptr::null(), CStr::as_ptr);
    // SAFETY: C owns both live, thread-local fixtures throughout each callback.
    let (c, rust) = unsafe {
        (
            oracle_encoder_reroute_run(
                oracle_obs_encoder_create_rerouted,
                flags,
                original,
                replacement,
                a,
                b,
                caps,
            ),
            oracle_encoder_reroute_run(
                obs_encoder_create_rerouted,
                flags,
                original,
                replacement,
                a,
                b,
                caps,
            ),
        )
    };
    assert_eq!(rust, c, "flags={flags}, kinds={original}/{replacement}");
    rust
}

#[test]
fn characterized_selection_and_callback_effects() {
    for flags in 0..32 {
        for (a, b) in [
            (Some(c"h264"), Some(c"H264")),
            (Some(c"h264"), Some(c"hevc")),
            (None, Some(c"")),
            (Some(c""), None),
            (None, None),
        ] {
            for (original, replacement) in [(0, 0), (0, 1), (1, 1), (1, 0)] {
                check(flags, original, replacement, a, b, u32::MAX);
            }
        }
    }
    // Admitted, matching: the whole table is installed before create, whose
    // result comes back; the original info and the registry are unchanged.
    let installed = ONE_CALL | CALLBACK_SAW_TABLE | ACTIVE_REPLACED | ORIGINAL_KEPT | REGISTRY_KEPT;
    assert_eq!(
        check(1, 0, 0, Some(c"h264"), Some(c"H264"), 1),
        RETURNED | installed
    );
    // A failed create returns NULL and does not roll the table back.
    assert_eq!(check(3, 0, 0, Some(c"h264"), Some(c"H264"), 1), installed);
    // Not admitted, wrong type, wrong codec, missing or empty ID: untouched.
    let untouched = ORIGINAL_KEPT | ACTIVE_KEPT | REGISTRY_KEPT;
    assert_eq!(check(0, 0, 0, Some(c"h264"), Some(c"H264"), 1), untouched);
    assert_eq!(check(1, 0, 1, Some(c"h264"), Some(c"H264"), 1), untouched);
    assert_eq!(check(1, 0, 0, Some(c"h264"), Some(c"hevc"), 1), untouched);
    assert_eq!(
        check(1 | 8, 0, 0, Some(c"h264"), Some(c"h264"), 1),
        untouched
    );
    assert_eq!(
        check(1 | 16, 0, 0, Some(c"h264"), Some(c"h264"), 1),
        untouched
    );
    // NULL codecs compare equal to each other and to "" (astrcmpi).
    assert_eq!(check(1, 1, 1, None, Some(c""), 1), RETURNED | installed);
}

#[test]
fn random_admission_selection_matches_c() {
    let config = ProptestConfig {
        cases: obs_c_oracle::parity::cases(512),
        failure_persistence: None,
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(config);
    runner
        .run(
            &(
                0u32..32,
                0i32..2,
                0i32..2,
                "[a-zA-Z0-9]{0,24}",
                any::<bool>(),
                any::<u32>(),
            ),
            |(flags, original, replacement, codec, matching, caps)| {
                let a = std::ffi::CString::new(codec.clone()).unwrap();
                let b = std::ffi::CString::new(if matching {
                    codec.to_ascii_uppercase()
                } else {
                    format!("{codec}x")
                })
                .unwrap();
                check(flags, original, replacement, Some(&a), Some(&b), caps);
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn null_diagnostics_match_c() {
    if !super::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: isolated process; the C helper restores the previous log handler.
    unsafe {
        assert!(oracle_encoder_reroute_logs(
            oracle_obs_encoder_create_rerouted,
            base_get_log_handler,
            base_set_log_handler
        ));
        assert!(oracle_encoder_reroute_logs(
            obs_encoder_create_rerouted,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}
