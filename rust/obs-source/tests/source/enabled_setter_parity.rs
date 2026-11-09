//! #4177: real private layout, original C setter, callbacks and reentrancy.
//! No intentional differences or excluded inputs. Sequences allocate at most 32 entries.
use core::ffi::c_void;
use obs_source::ffi::source_enabled_setter::obs_source_set_enabled;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Setter = unsafe extern "C" fn(*mut c_void, bool);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_obs_source_set_enabled(source: *mut c_void, enabled: bool);
    fn oracle_source_enabled_run(
        setter: Setter,
        initial: bool,
        mutate: bool,
        reenter: bool,
        values: *const bool,
        count: usize,
        trace: *mut u32,
    ) -> bool;
    fn oracle_source_enabled_layout(layout: *mut usize);
    fn oracle_source_enabled_null_logs(setter: Setter, get: GetLog, set: SetLog) -> bool;
}

#[test]
fn real_header_bool_layout() {
    let mut layout = [0; 2];
    // SAFETY: the oracle writes exactly two entries into this output array.
    unsafe { oracle_source_enabled_layout(layout.as_mut_ptr()) };
    assert_eq!(layout, [size_of::<bool>(), align_of::<bool>()]);
}

fn compare(initial: bool, mutate: bool, reenter: bool, values: &[bool]) {
    assert!(values.len() <= 32);
    let mut rust = vec![0; values.len()];
    let mut original = vec![0; values.len()];
    // SAFETY: the C fixture owns a real source and live signal handler; both
    // inputs/outputs have count entries and each setter has the original ABI.
    unsafe {
        assert!(oracle_source_enabled_run(
            obs_source_set_enabled,
            initial,
            mutate,
            reenter,
            values.as_ptr(),
            values.len(),
            rust.as_mut_ptr()
        ));
        assert!(oracle_source_enabled_run(
            oracle_obs_source_set_enabled,
            initial,
            mutate,
            reenter,
            values.as_ptr(),
            values.len(),
            original.as_mut_ptr()
        ));
    }
    assert_eq!(rust, original);
}

#[test]
fn signal_payload_repeats_mutation_and_reentrancy() {
    for initial in [false, true] {
        for mutate in [false, true] {
            for reenter in [false, true] {
                compare(initial, mutate, reenter, &[]);
                compare(
                    initial,
                    mutate,
                    reenter,
                    &[false, false, true, true, false, true],
                );
            }
        }
    }
}

#[test]
fn null_diagnostics_match_original() {
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_original")) {
        return;
    }
    // SAFETY: isolated child owns the process log callback and the C probe
    // restores it. Callback pointers use the base shim's opaque pointer ABI.
    unsafe {
        assert!(oracle_source_enabled_null_logs(
            obs_source_set_enabled,
            base_get_log_handler,
            base_set_log_handler
        ));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_enabled_sequences_match_c(initial in any::<bool>(), mutate in any::<bool>(), reenter in any::<bool>(), values in prop::collection::vec(any::<bool>(), 0..=32)) {
        compare(initial, mutate, reenter, &values);
    }
}
