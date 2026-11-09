//! Actual-header field seam and exact differential behavior for #3904.
use obs_c_oracle as _;
use obs_graphics::ffi::linear_srgb::gs_get_linear_srgb;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_void};
use std::mem::{align_of, size_of};

unsafe extern "C" {
    fn oracle_gs_get_linear_srgb() -> bool;
    fn oracle_gs_set_linear_srgb(value: bool) -> bool;
    fn oracle_linear_srgb_reset(present: bool, value: bool);
    fn oracle_linear_srgb_logs() -> u32;
    fn oracle_linear_srgb_level() -> i32;
    fn oracle_linear_srgb_log() -> *const c_char;
    fn oracle_linear_srgb_value() -> bool;
    fn oracle_linear_srgb_storage() -> *const c_void;
    fn oracle_linear_srgb_offset() -> usize;
    fn oracle_linear_srgb_bool_size() -> usize;
    fn oracle_linear_srgb_bool_align() -> usize;
    fn obs_graphics_linear_srgb_field(function: *const c_char) -> *const bool;
}

#[test]
fn real_header_field_layout() {
    // SAFETY: the oracle owns thread-local storage of the real C header type.
    unsafe {
        assert_eq!(oracle_linear_srgb_bool_size(), size_of::<bool>());
        assert_eq!(oracle_linear_srgb_bool_align(), align_of::<bool>());
        oracle_linear_srgb_reset(true, true);
        let base = oracle_linear_srgb_storage().addr();
        let field = obs_graphics_linear_srgb_field(c"gs_get_linear_srgb".as_ptr());
        assert_eq!(field.addr() - base, oracle_linear_srgb_offset());
        assert!(*field);
        assert_eq!(oracle_linear_srgb_logs(), 0);
    }
}

fn compare(present: bool, value: bool) {
    // SAFETY: each reset initializes this thread's C-owned context and log;
    // both getters borrow it only for the duration of their call.
    unsafe {
        oracle_linear_srgb_reset(present, value);
        let expected = oracle_gs_get_linear_srgb();
        let expected_logs = oracle_linear_srgb_logs();
        let expected_level = oracle_linear_srgb_level();
        let expected_log = CStr::from_ptr(oracle_linear_srgb_log()).to_bytes().to_vec();
        assert_eq!(oracle_linear_srgb_value(), value);

        oracle_linear_srgb_reset(present, value);
        assert_eq!(gs_get_linear_srgb(), expected);
        assert_eq!(oracle_linear_srgb_value(), value);
        assert_eq!(oracle_linear_srgb_logs(), expected_logs);
        assert_eq!(oracle_linear_srgb_level(), expected_level);
        assert_eq!(
            CStr::from_ptr(oracle_linear_srgb_log()).to_bytes(),
            expected_log
        );
        assert_eq!(expected, present && value);
        if !present {
            assert_eq!(expected_logs, 1);
            assert_eq!(expected_level, 400); // LOG_DEBUG, from the real base.h.
            assert_eq!(
                expected_log,
                b"gs_get_linear_srgb: called while not in a graphics context"
            );
        } else {
            assert_eq!(expected_logs, 0);
        }
    }
}

#[test]
fn all_states_match_c() {
    for present in [false, true] {
        for value in [false, true] {
            compare(present, value);
            compare(present, value);
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
    fn state_sequences_match_c(states in prop::collection::vec((any::<bool>(), any::<bool>()), 0..64)) {
        for (present, value) in states {
            compare(present, value);
        }
    }
}

#[test]
fn characterize_c_setter() {
    for present in [false, true] {
        for previous in [false, true] {
            for next in [false, true] {
                // SAFETY: all state is this thread's initialized real-header C fixture.
                unsafe {
                    oracle_linear_srgb_reset(present, previous);
                    assert_eq!(oracle_gs_set_linear_srgb(next), present && previous);
                    assert_eq!(
                        oracle_linear_srgb_value(),
                        if present { next } else { previous }
                    );
                    assert_eq!(oracle_gs_set_linear_srgb(next), present && next);
                    assert_eq!(oracle_linear_srgb_logs(), if present { 0 } else { 2 });
                    if !present {
                        assert_eq!(oracle_linear_srgb_level(), 400);
                        assert_eq!(
                            CStr::from_ptr(oracle_linear_srgb_log()).to_bytes(),
                            b"gs_set_linear_srgb: called while not in a graphics context"
                        );
                    }
                }
            }
        }
    }
}
