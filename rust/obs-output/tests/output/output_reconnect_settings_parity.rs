//! Exact signed stores, unrelated bytes, layout and NULL logging; no exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::output_reconnect_settings::{Fields, obs_output_set_reconnect_settings};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_output_reconnect_settings_layout(layout: *mut usize);
    fn oracle_output_reconnect_settings_new() -> *mut c_void;
    fn oracle_output_reconnect_settings_check(output: *mut c_void, count: i32, sec: i32) -> bool;
    fn oracle_output_reconnect_settings_free(output: *mut c_void);
    fn oracle_obs_output_set_reconnect_settings(output: *mut c_void, count: i32, sec: i32);
    fn oracle_output_reconnect_settings_null_log(rust: bool) -> bool;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 4];
    // SAFETY: oracle writes exactly four size_t values.
    unsafe { oracle_output_reconnect_settings_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            offset_of!(Fields, retry_max),
            offset_of!(Fields, retry_sec)
        ]
    );
}

struct Output(*mut c_void);
impl Output {
    fn new() -> Self {
        // SAFETY: oracle returns a uniquely owned, real-layout fixture.
        let output = Self(unsafe { oracle_output_reconnect_settings_new() });
        assert!(!output.0.is_null());
        output
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        // SAFETY: exclusively owned oracle allocation.
        unsafe { oracle_output_reconnect_settings_free(self.0) };
    }
}

fn check(settings: &[(i32, i32)]) {
    let rust = Output::new();
    let c = Output::new();
    for &(count, sec) in settings {
        // SAFETY: live, exclusively owned outputs; setters borrow only for the call.
        unsafe {
            obs_output_set_reconnect_settings(rust.0, count, sec);
            oracle_obs_output_set_reconnect_settings(c.0, count, sec);
            assert!(oracle_output_reconnect_settings_check(rust.0, count, sec));
            assert!(oracle_output_reconnect_settings_check(c.0, count, sec));
        }
    }
}

#[test]
fn signed_edges_and_replacement_match_c() {
    let edges = [i32::MIN, -60, -1, 0, 1, 5, 20, 60, i32::MAX];
    let settings: Vec<_> = edges
        .into_iter()
        .flat_map(|count| edges.map(|sec| (count, sec)))
        .collect();
    check(&settings);
}

#[test]
fn null_log_matches_c() {
    if !super::isolated(concat!(module_path!(), "::null_log_matches_c")) {
        return;
    }
    // SAFETY: logger changes are saved/restored inside this isolated child.
    unsafe {
        assert!(oracle_output_reconnect_settings_null_log(false));
        assert!(oracle_output_reconnect_settings_null_log(true));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_settings_match_c(settings in prop::collection::vec((any::<i32>(), any::<i32>()), 1..17)) {
        check(&settings);
    }
}
