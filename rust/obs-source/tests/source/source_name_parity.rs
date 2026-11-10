//! Tier 3: unchanged original C, including NULL diagnostics and pointer identity.
//! No intentional differences or input exclusions; byte strings need not be UTF-8.
use core::ffi::{c_char, c_void};
use obs_source::ffi::source_name::obs_source_get_name;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;
use std::ffi::CString;

type Getter = unsafe extern "C" fn(*const c_void) -> *const c_char;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_source_name_new(name: *mut c_char, private_source: bool, flags: u32) -> *mut c_void;
    fn oracle_source_name_set(source: *mut c_void, name: *mut c_char);
    fn oracle_source_name_free(source: *mut c_void);
    fn oracle_source_name_unchanged(source: *const c_void) -> bool;
    fn oracle_obs_source_get_name(source: *const c_void) -> *const c_char;
    fn oracle_source_name_logs(getter: Getter, get: GetLog, set: SetLog) -> bool;
}

struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: this fixture owns only its C allocation, never its borrowed name.
        unsafe { oracle_source_name_free(self.0) }
    }
}

fn check(bytes: Option<&[u8]>, private_source: bool, flags: u32) {
    let text = bytes.map(|bytes| CString::new(bytes).unwrap());
    let expected = text.as_ref().map_or(core::ptr::null(), |s| s.as_ptr());
    // SAFETY: the C fixture borrows text for this scope and owns its source storage.
    let fixture =
        Fixture(unsafe { oracle_source_name_new(expected.cast_mut(), private_source, flags) });
    assert!(!fixture.0.is_null());
    // SAFETY: both getters receive the same live real-layout source; neither owns it.
    unsafe {
        for name in [expected, c"renamed".as_ptr(), core::ptr::null(), expected] {
            oracle_source_name_set(fixture.0, name.cast_mut());
            for _ in 0..2 {
                let got = obs_source_get_name(fixture.0);
                assert_eq!(got, oracle_obs_source_get_name(fixture.0));
                assert_eq!(got, name);
                assert!(oracle_source_name_unchanged(fixture.0));
            }
        }
    }
    // text is dropped only after the source fixture: no copied or owned name.
}

#[test]
fn characterized_edges() {
    let long = vec![b'L'; 64 * 1024];
    for bytes in [
        None,
        Some(&b""[..]),
        Some(&b"A"[..]),
        Some(&b"A 2"[..]),
        Some(&b"\xff\xfe\x80"[..]),
        Some(&b"%s %d %n"[..]),
        Some(long.as_slice()),
    ] {
        for private_source in [false, true] {
            for flags in [0, u32::MAX] {
                check(bytes, private_source, flags);
            }
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    // SAFETY: isolated child owns and restores the process log handler.
    unsafe {
        assert!(oracle_source_name_logs(
            obs_source_get_name,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_names_match_c(bytes in prop::option::of(prop::collection::vec(1u8..=255, 0..256)), private_source in any::<bool>(), flags in any::<u32>()) {
        check(bytes.as_deref(), private_source, flags);
    }
}
