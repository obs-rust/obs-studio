//! Exact borrowed-pointer, source-type, NULL diagnostic and state parity. No exclusions.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_source::ffi::source_filter_relationship_getters::{self as rust, FilterRelationshipFields};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_filter_relationship_layout(layout: *mut usize);
    fn oracle_filter_relationship_new(parent: u32, target: u32, source_type: u32) -> *mut c_void;
    fn oracle_filter_relationship_free(filter: *mut c_void);
    fn oracle_filter_relationship_unchanged(filter: *const c_void) -> bool;
    fn oracle_obs_filter_get_parent(filter: *const c_void) -> *mut c_void;
    fn oracle_obs_filter_get_target(filter: *const c_void) -> *mut c_void;
    fn oracle_filter_relationship_null_log(
        parent: unsafe extern "C" fn(*const c_void) -> *mut c_void,
        target: unsafe extern "C" fn(*const c_void) -> *mut c_void,
    ) -> bool;
}

#[test]
fn real_header_view_layout() {
    let mut c = [0; 6];
    // SAFETY: the oracle writes six size_t values into the array.
    unsafe { oracle_filter_relationship_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<FilterRelationshipFields>(),
            align_of::<FilterRelationshipFields>(),
            offset_of!(FilterRelationshipFields, parent),
            offset_of!(FilterRelationshipFields, target),
            size_of::<*mut c_void>(),
            align_of::<*mut c_void>()
        ]
    );
}

struct Filter(*mut c_void);
impl Drop for Filter {
    fn drop(&mut self) {
        // SAFETY: the complete C fixture is allocated once and freed exactly once.
        unsafe { oracle_filter_relationship_free(self.0) };
    }
}

fn check(parent: u32, target: u32, source_type: u32) {
    // SAFETY: indices and source_type are in 0..4; C allocates all three complete sources.
    let filter = Filter(unsafe { oracle_filter_relationship_new(parent, target, source_type) });
    assert!(!filter.0.is_null());
    // SAFETY: fixture and all borrowed relationships stay live with no concurrent writes.
    unsafe {
        assert_eq!(
            rust::obs_filter_get_parent(filter.0),
            oracle_obs_filter_get_parent(filter.0)
        );
        assert!(oracle_filter_relationship_unchanged(filter.0));
        assert_eq!(
            rust::obs_filter_get_target(filter.0),
            oracle_obs_filter_get_target(filter.0)
        );
        assert!(oracle_filter_relationship_unchanged(filter.0));
    }
}

#[test]
fn characterized_edges() {
    for source_type in 0..4 {
        for parent in 0..4 {
            for target in 0..4 {
                check(parent, target, source_type);
            }
        }
    }
}

#[test]
fn null_defaults_and_exact_logs() {
    // The logger is global: isolate in the existing Core state executable.
    if std::env::var_os("OBS_FILTER_RELATIONSHIP_GETTERS_CHILD").is_none() {
        let selector = concat!(module_path!(), "::null_defaults_and_exact_logs")
            .split_once("::")
            .unwrap()
            .1;
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", selector, "--nocapture"])
            .env("OBS_FILTER_RELATIONSHIP_GETTERS_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("1 passed"), "{stdout}");
        return;
    }
    // SAFETY: isolated probes save and restore the process-global logger.
    unsafe {
        assert!(oracle_filter_relationship_null_log(
            oracle_obs_filter_get_parent,
            oracle_obs_filter_get_target
        ));
        assert!(oracle_filter_relationship_null_log(
            rust::obs_filter_get_parent,
            rust::obs_filter_get_target
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_fields_match_c(parent in 0u32..4, target in 0u32..4, source_type in 0u32..4) {
        check(parent, target, source_type);
    }
}
