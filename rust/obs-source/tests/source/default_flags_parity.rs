//! Tier 3: exact real-layout storage and NULL diagnostics against original C.
//! No intentional differences or exclusions. Sequences allocate at most 64 entries.
use core::ffi::c_void;
use obs_source::ffi::source_default_flags::obs_source_set_default_flags;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Setter = unsafe extern "C" fn(*mut c_void, u32);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn oracle_obs_source_set_default_flags(source: *mut c_void, flags: u32);
    fn oracle_source_default_flags_run(
        setter: Setter,
        initial: u32,
        current: u32,
        values: *const u32,
        count: usize,
        trace: *mut u32,
    ) -> bool;
    fn oracle_source_default_flags_layout(layout: *mut usize);
    fn oracle_source_default_flags_null_logs(setter: Setter, get: GetLog, set: SetLog) -> bool;
}

fn compare(initial: u32, current: u32, values: &[u32]) {
    assert!(values.len() <= 64);
    let mut rust = vec![0; values.len()];
    let mut original = vec![0; values.len()];
    // SAFETY: C owns two separate zeroed real sources and frees each after use.
    // Input/output arrays have count entries; each setter has the original ABI.
    unsafe {
        assert!(oracle_source_default_flags_run(
            obs_source_set_default_flags,
            initial,
            current,
            values.as_ptr(),
            values.len(),
            rust.as_mut_ptr()
        ));
        assert!(oracle_source_default_flags_run(
            oracle_obs_source_set_default_flags,
            initial,
            current,
            values.as_ptr(),
            values.len(),
            original.as_mut_ptr()
        ));
    }
    assert_eq!(rust, original);
    assert_eq!(rust, values);
}

#[test]
fn real_header_uint32_layout() {
    let mut layout = [0; 2];
    // SAFETY: oracle writes exactly two size/alignment entries.
    unsafe { oracle_source_default_flags_layout(layout.as_mut_ptr()) };
    assert_eq!(layout, [size_of::<u32>(), align_of::<u32>()]);
}

#[test]
fn exact_bits_replacement_and_unrelated_bytes_match_c() {
    let values: Vec<_> = [0, u32::MAX, 0xaaaa_aaaa, 0x5555_5555, 0]
        .into_iter()
        .chain((0..32).map(|bit| 1 << bit))
        .collect();
    for initial in [0, u32::MAX, 0x2468_ace0] {
        for current in [0, u32::MAX, 0xa55a_a55a] {
            compare(initial, current, &[]);
            compare(initial, current, &values);
        }
    }
}

#[test]
fn null_diagnostics_match_original() {
    if !crate::output::isolated(concat!(module_path!(), "::null_diagnostics_match_original")) {
        return;
    }
    // SAFETY: isolated child owns the process log handler; C restores it after
    // probing both setters. Callback pointers have the base shim's opaque ABI.
    unsafe {
        assert!(oracle_source_default_flags_null_logs(
            obs_source_set_default_flags,
            base_get_log_handler,
            base_set_log_handler
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
    fn arbitrary_flags_and_sequences_match_c(initial in any::<u32>(), current in any::<u32>(),
        values in proptest::collection::vec(any::<u32>(), 0..=64)) {
        compare(initial, current, &values);
    }
}
