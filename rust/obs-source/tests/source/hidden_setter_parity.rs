//! Exact full-object parity with the original setter; C NULL is excluded.
use core::{
    ffi::c_void,
    mem::{align_of, size_of},
};
use obs_source::ffi::source_hidden_setter::obs_source_set_hidden;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_source_hidden_new(
        centered: bool,
        hidden: bool,
        enabled: bool,
        flags: u32,
    ) -> *mut c_void;
    fn oracle_source_hidden_free(source: *mut c_void);
    fn oracle_source_hidden_equal(left: *const c_void, right: *const c_void) -> bool;
    fn oracle_source_hidden_value(source: *const c_void) -> bool;
    fn oracle_source_hidden_bool_layout(layout: *mut usize);
    fn oracle_obs_source_set_hidden(source: *mut c_void, hidden: bool);
}

struct Source(*mut c_void);
impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: each fixture is allocated once by the matching oracle helper.
        unsafe { oracle_source_hidden_free(self.0) };
    }
}

fn check(centered: bool, initial: bool, enabled: bool, flags: u32, writes: &[bool]) {
    // SAFETY: the oracle allocates complete, zero-initialized real C source objects.
    let (rust, c) = unsafe {
        (
            Source(oracle_source_hidden_new(centered, initial, enabled, flags)),
            Source(oracle_source_hidden_new(centered, initial, enabled, flags)),
        )
    };
    assert!(!rust.0.is_null() && !c.0.is_null());
    for &hidden in writes {
        // SAFETY: both sources are live, independently owned and exclusively borrowed.
        unsafe {
            obs_source_set_hidden(rust.0, hidden);
            oracle_obs_source_set_hidden(c.0, hidden);
            assert_eq!(oracle_source_hidden_value(rust.0), hidden);
            assert!(oracle_source_hidden_equal(rust.0, c.0));
        }
    }
}

#[test]
fn all_boolean_edges_and_repeated_writes() {
    for centered in [false, true] {
        for initial in [false, true] {
            for enabled in [false, true] {
                for flags in [0, 0xa5a5_5a5a, u32::MAX] {
                    check(
                        centered,
                        initial,
                        enabled,
                        flags,
                        &[false, false, true, true, false],
                    );
                }
            }
        }
    }
}

#[test]
fn c_bool_layout_matches() {
    let mut c = [0; 2];
    // SAFETY: the helper writes exactly two size_t entries.
    unsafe { oracle_source_hidden_bool_layout(c.as_mut_ptr()) };
    assert_eq!(c, [size_of::<bool>(), align_of::<bool>()]);
}

#[test]
fn null_is_checked_before_field_access() {
    // SAFETY: the Rust extension accepts NULL without calling the C field adapter.
    unsafe {
        obs_source_set_hidden(core::ptr::null_mut(), false);
        obs_source_set_hidden(core::ptr::null_mut(), true);
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(64),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn random_assignments_match_c(centered in any::<bool>(), initial in any::<bool>(), enabled in any::<bool>(), flags in any::<u32>(), writes in proptest::collection::vec(any::<bool>(), 0..33)) {
        check(centered, initial, enabled, flags, &writes);
    }
}
