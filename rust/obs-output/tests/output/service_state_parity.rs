//! Tier 3 against verbatim C. NULL services and detach without a live output
//! are undefined in C and excluded. No intentional behavior differences.
#![allow(unsafe_code)]
use obs_output::ffi::service_state::{self as rust, ServiceStateFields as F};
use proptest::prelude::*;
use std::ffi::{c_long, c_void};
use std::mem::{align_of, offset_of, size_of};

unsafe extern "C" {
    fn oracle_state_new(
        active: bool,
        destroy: bool,
        output: bool,
        control: bool,
        reverse: bool,
    ) -> *mut c_void;
    fn oracle_state_free(p: *mut c_void);
    fn oracle_state_bits(p: *mut c_void) -> u32;
    fn oracle_state_control(p: *mut c_void) -> *mut c_void;
    fn oracle_state_refs(p: *mut c_void, weak: bool) -> c_long;
    fn oracle_state_output(p: *mut c_void) -> *mut c_void;
    fn oracle_state_reverse(p: *mut c_void) -> *mut c_void;
    fn oracle_state_layout(index: u32) -> usize;
    fn oracle_state_field_addresses(p: *mut c_void) -> bool;
    fn oracle_state_set_destroy(p: *mut c_void);
    fn oracle_state_is_destroying(p: *const c_void) -> bool;
    fn oracle_state_is_active(p: *const c_void) -> bool;
    fn oracle_state_set_active(p: *mut c_void, active: bool);
    fn oracle_state_has_output(p: *const c_void) -> bool;
    fn oracle_state_clear_output(p: *mut c_void);
    fn oracle_state_detach_from_output(p: *mut c_void);
    fn oracle_state_get_control(p: *const c_void) -> *mut c_void;
}
struct Fixture(*mut c_void);
impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: owns exactly one allocation from oracle_state_new.
        unsafe { oracle_state_free(self.0) }
    }
}
fn compare(bits: u8, operations: &[u8]) {
    // SAFETY: independent fixtures have the real C layouts and remain live;
    // detach is called only while their output link is non-null.
    unsafe {
        let create = || {
            Fixture(oracle_state_new(
                bits & 1 != 0,
                bits & 2 != 0,
                bits & 4 != 0,
                bits & 8 != 0,
                bits & 16 != 0,
            ))
        };
        let c = create();
        let r = create();
        assert!(!c.0.is_null() && !r.0.is_null());
        assert!(oracle_state_field_addresses(r.0));
        let initial_output = oracle_state_output(r.0);
        let initial_reverse = oracle_state_reverse(r.0);
        for &op in operations {
            match op % 5 {
                0 => {
                    oracle_state_set_destroy(c.0);
                    rust::obs_service_set_destroy(r.0);
                }
                1 | 2 => {
                    let active = op % 5 == 1;
                    oracle_state_set_active(c.0, active);
                    rust::obs_service_set_active(r.0, active);
                }
                3 => {
                    oracle_state_clear_output(c.0);
                    rust::obs_service_clear_output(r.0);
                }
                _ if oracle_state_has_output(c.0) => {
                    oracle_state_detach_from_output(c.0);
                    rust::obs_service_detach_from_output(r.0);
                }
                _ => {}
            }
            assert_eq!(oracle_state_bits(r.0), oracle_state_bits(c.0));
            assert_eq!(
                rust::obs_service_is_active(r.0),
                oracle_state_is_active(c.0)
            );
            assert_eq!(
                rust::obs_service_is_destroying(r.0),
                oracle_state_is_destroying(c.0)
            );
            assert_eq!(
                rust::obs_service_has_output(r.0),
                oracle_state_has_output(c.0)
            );
            assert_eq!(
                rust::obs_service_get_control(r.0),
                oracle_state_control(r.0)
            );
            assert_eq!(oracle_state_get_control(c.0), oracle_state_control(c.0));
            if oracle_state_bits(c.0) & 4 != 0 {
                assert_eq!(oracle_state_output(r.0), initial_output);
            }
            if oracle_state_bits(c.0) & 16 != 0 {
                assert_eq!(oracle_state_reverse(r.0), initial_reverse);
            }
            for weak in [false, true] {
                assert_eq!(oracle_state_refs(r.0, weak), 0);
                assert_eq!(oracle_state_refs(c.0, weak), 0);
            }
        }
    }
}
#[test]
fn real_header_layout() {
    let rust = [
        size_of::<F>(),
        align_of::<F>(),
        offset_of!(F, active),
        offset_of!(F, destroy),
        offset_of!(F, output),
        offset_of!(F, control),
    ];
    for (index, value) in rust.into_iter().enumerate() {
        // SAFETY: index is within the real-header measurement table.
        assert_eq!(value, unsafe { oracle_state_layout(index as u32) });
    }
}
#[test]
fn fixed_states_match_c() {
    for bits in 0..32 {
        compare(bits, &[4, 4, 0, 0, 1, 2, 1, 3, 3, 2, 4]);
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn state_sequences_match_c(bits in 0u8..32, operations in prop::collection::vec(any::<u8>(), 1..64)) {
        compare(bits, &operations);
    }
}
