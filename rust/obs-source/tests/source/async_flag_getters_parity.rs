//! Exact flag, NULL diagnostic and complete source-state parity for the
//! getters and the unbuffered setter (#5595). No exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_source::ffi::source_async_flag_getters::{self as rust, AsyncFlagFields};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_source_async_flag_layout(layout: *mut usize);
    fn oracle_source_async_flag_new(unbuffered: bool, decoupled: bool) -> *mut c_void;
    fn oracle_source_async_flag_free(source: *mut c_void);
    fn oracle_source_async_flag_unchanged(source: *const c_void) -> bool;
    fn oracle_source_async_flag_stored(source: *mut c_void, expected: bool) -> bool;
    fn oracle_obs_source_set_async_unbuffered(source: *mut c_void, unbuffered: bool);
    fn oracle_source_async_unbuffered_setter_null_log(
        set_unbuffered: unsafe extern "C" fn(*mut c_void, bool),
        requested: bool,
    ) -> bool;
    fn oracle_obs_source_async_unbuffered(source: *const c_void) -> bool;
    fn oracle_obs_source_async_decoupled(source: *const c_void) -> bool;
    fn oracle_source_async_flag_null_log(
        unbuffered: unsafe extern "C" fn(*const c_void) -> bool,
        decoupled: unsafe extern "C" fn(*const c_void) -> bool,
    ) -> bool;
}

#[test]
fn real_header_view_layout() {
    let mut c = [0; 8];
    // SAFETY: the oracle writes eight size_t values into the array.
    unsafe { oracle_source_async_flag_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<AsyncFlagFields>(),
            align_of::<AsyncFlagFields>(),
            offset_of!(AsyncFlagFields, unbuffered),
            offset_of!(AsyncFlagFields, decoupled),
            size_of::<bool>(),
            align_of::<bool>(),
            size_of::<bool>(),
            align_of::<bool>(),
        ]
    );
}

struct Source(*mut c_void);
impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: the C fixture is allocated once and freed exactly once.
        unsafe { oracle_source_async_flag_free(self.0) };
    }
}

fn check(unbuffered: bool, decoupled: bool) {
    // SAFETY: the oracle allocates the complete real C source layout.
    let source = Source(unsafe { oracle_source_async_flag_new(unbuffered, decoupled) });
    assert!(!source.0.is_null());
    // SAFETY: the fixture remains live and exclusively owned during both reads.
    unsafe {
        assert_eq!(
            rust::obs_source_async_unbuffered(source.0),
            oracle_obs_source_async_unbuffered(source.0)
        );
        assert!(oracle_source_async_flag_unchanged(source.0));
        assert_eq!(
            rust::obs_source_async_decoupled(source.0),
            oracle_obs_source_async_decoupled(source.0)
        );
        assert!(oracle_source_async_flag_unchanged(source.0));
    }
}

/// Both setters store the same sequence into twin fixtures; after each store
/// each fixture differs from its snapshot in the unbuffered byte alone, and
/// both getters then agree on it.
fn check_setter(unbuffered: bool, decoupled: bool, requests: &[bool]) {
    // SAFETY: the oracle allocates complete real-layout C fixtures.
    let rust = Source(unsafe { oracle_source_async_flag_new(unbuffered, decoupled) });
    // SAFETY: an independent twin, released by Drop.
    let c = Source(unsafe { oracle_source_async_flag_new(unbuffered, decoupled) });
    assert!(!rust.0.is_null() && !c.0.is_null());
    for &requested in requests {
        // SAFETY: live, exclusively owned fixtures; nothing else touches them.
        unsafe {
            rust::obs_source_set_async_unbuffered(rust.0, requested);
            oracle_obs_source_set_async_unbuffered(c.0, requested);
            assert!(oracle_source_async_flag_stored(rust.0, requested));
            assert!(oracle_source_async_flag_stored(c.0, requested));
            assert_eq!(
                rust::obs_source_async_unbuffered(rust.0),
                oracle_obs_source_async_unbuffered(c.0)
            );
        }
    }
}

#[test]
fn setter_characterized_edges() {
    for unbuffered in [false, true] {
        for decoupled in [false, true] {
            check_setter(
                unbuffered,
                decoupled,
                &[true, true, false, false, true, false],
            );
        }
    }
}

#[test]
fn characterized_edges() {
    for unbuffered in [false, true] {
        for decoupled in [false, true] {
            check(unbuffered, decoupled);
        }
    }
}

#[test]
fn null_defaults_and_historical_logs() {
    // The logger is global: isolate in the existing Core state executable.
    if std::env::var_os("OBS_ASYNC_FLAG_GETTERS_CHILD").is_none() {
        let selector = concat!(module_path!(), "::null_defaults_and_historical_logs")
            .split_once("::")
            .unwrap()
            .1;
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", selector, "--nocapture"])
            .env("OBS_ASYNC_FLAG_GETTERS_CHILD", "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    // SAFETY: the isolated probes save and restore the process-global logger.
    unsafe {
        assert!(oracle_source_async_flag_null_log(
            oracle_obs_source_async_unbuffered,
            oracle_obs_source_async_decoupled
        ));
        assert!(oracle_source_async_flag_null_log(
            rust::obs_source_async_unbuffered,
            rust::obs_source_async_decoupled
        ));
        for requested in [false, true] {
            assert!(oracle_source_async_unbuffered_setter_null_log(
                oracle_obs_source_set_async_unbuffered,
                requested
            ));
            assert!(oracle_source_async_unbuffered_setter_null_log(
                rust::obs_source_set_async_unbuffered,
                requested
            ));
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_fields_match_c(unbuffered in any::<bool>(), decoupled in any::<bool>()) {
        check(unbuffered, decoupled);
    }

    #[test]
    fn setter_sequences_match_c(
        unbuffered in any::<bool>(), decoupled in any::<bool>(),
        requests in prop::collection::vec(any::<bool>(), 1..32),
    ) {
        check_setter(unbuffered, decoupled, &requests);
    }
}
