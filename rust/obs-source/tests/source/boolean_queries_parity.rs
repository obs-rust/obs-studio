//! Exact result and full-state parity. NULL is excluded because C dereferences it.
use core::ffi::c_void;
use core::mem::{align_of, offset_of, size_of};
use obs_source::ffi::source_boolean_queries::{self as rust, BooleanQueryFields};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_source_boolean_layout(layout: *mut usize);
    fn oracle_source_boolean_new(
        centered: bool,
        hidden: bool,
        enabled: bool,
        flags: u32,
    ) -> *mut c_void;
    fn oracle_source_boolean_free(source: *mut c_void);
    fn oracle_source_boolean_unchanged(source: *const c_void) -> bool;
    fn oracle_obs_source_get_texcoords_centered(source: *mut c_void) -> bool;
    fn oracle_obs_source_is_hidden(source: *mut c_void) -> bool;
}

#[test]
fn real_header_view_layout() {
    let mut c = [0; 6];
    // SAFETY: the oracle writes six size_t values into the array.
    unsafe { oracle_source_boolean_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<BooleanQueryFields>(),
            align_of::<BooleanQueryFields>(),
            offset_of!(BooleanQueryFields, texcoords_centered),
            offset_of!(BooleanQueryFields, hidden),
            size_of::<bool>(),
            align_of::<bool>(),
        ]
    );
}

struct Source(*mut c_void);
impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: the fixture is allocated once and freed exactly once.
        unsafe { oracle_source_boolean_free(self.0) };
    }
}

fn check(centered: bool, hidden: bool, enabled: bool, flags: u32) {
    // SAFETY: the oracle allocates the complete real C source layout.
    let source = Source(unsafe { oracle_source_boolean_new(centered, hidden, enabled, flags) });
    assert!(!source.0.is_null());
    // SAFETY: both implementations borrow the exclusively owned, live fixture.
    unsafe {
        for _ in 0..2 {
            assert_eq!(
                rust::obs_source_get_texcoords_centered(source.0),
                oracle_obs_source_get_texcoords_centered(source.0)
            );
            assert!(oracle_source_boolean_unchanged(source.0));
            assert_eq!(
                rust::obs_source_is_hidden(source.0),
                oracle_obs_source_is_hidden(source.0)
            );
            assert!(oracle_source_boolean_unchanged(source.0));
        }
    }
}

#[test]
fn characterized_edges() {
    for centered in [false, true] {
        for hidden in [false, true] {
            check(centered, hidden, true, 0xa5a5_5a5a);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(64),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_fields_match_c(centered in any::<bool>(), hidden in any::<bool>(), enabled in any::<bool>(), flags in any::<u32>()) {
        check(centered, hidden, enabled, flags);
    }
}
