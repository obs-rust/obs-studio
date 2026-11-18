//! Tier 3 original-C parity; no intentional behavior differences.
use core::ffi::{c_char, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_defaults::{DefaultsFields, obs_encoder_defaults, obs_encoder_get_defaults};
use proptest::prelude::*;

type ById = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type ByInstance = unsafe extern "C" fn(*const c_void) -> *mut c_void;
unsafe extern "C" {
    fn oracle_obs_encoder_defaults(id: *const c_char) -> *mut c_void;
    fn oracle_obs_encoder_get_defaults(encoder: *const c_void) -> *mut c_void;
    fn oracle_encoder_defaults_run(by_id: ById, by_instance: ByInstance, flags: u32, seed: u32, report: *mut u64);
    fn oracle_encoder_defaults_layout(layout: *mut usize);
}
#[test]
fn real_header_layout() {
    let mut actual = [0usize; 5];
    // SAFETY: oracle writes five size_t entries, measured from the real private header.
    unsafe { oracle_encoder_defaults_layout(actual.as_mut_ptr()) };
    assert_eq!(actual, [size_of::<DefaultsFields>(), align_of::<DefaultsFields>(), offset_of!(DefaultsFields, legacy), offset_of!(DefaultsFields, modern), offset_of!(DefaultsFields, type_data)]);
}
fn check(flags: u32, seed: u32) {
    let mut expected = [0u64; 20];
    let mut actual = [0u64; 20];
    // SAFETY: the runner supplies real live C fixtures and writes twenty u64 entries.
    // Its registry/log mutations are isolated by re-executing the containing test.
    unsafe {
        oracle_encoder_defaults_run(oracle_obs_encoder_defaults, oracle_obs_encoder_get_defaults, flags, seed, expected.as_mut_ptr());
        oracle_encoder_defaults_run(obs_encoder_defaults, obs_encoder_get_defaults, flags, seed, actual.as_mut_ptr());
    }
    assert_eq!(actual, expected, "flags={flags}, seed={seed}");
    let null_instance = flags & 32 != 0 && flags & 16 != 0;
    let exists = if flags & 32 != 0 { !null_instance } else { flags & 8 == 0 };
    let mask = flags & 3;
    let trace = if exists { match mask { 1 => 111, 2 => 222, 3 => 121212, _ => 0 } } else { 0 };
    assert_eq!(actual, [
        u64::from(exists), u64::from(exists && mask != 0),
        if !exists { 0 } else if mask & 2 != 0 { u64::from(seed ^ 0x1234) } else if mask & 1 != 0 { u64::from(seed) } else { 0 },
        u64::from(exists && mask != 0), 0, u64::from(exists && mask & 1 != 0),
        u64::from(exists), u64::from(exists), u64::from(exists), u64::from(exists), u64::from(exists),
        trace, 1, 0, 1, 1, u64::from(null_instance), if null_instance {400} else {0}, u64::from(null_instance), 1,
    ]);
}
#[test]
fn callbacks_fresh_ownership_and_diagnostics() {
    if !super::isolated(concat!(module_path!(), "::callbacks_fresh_ownership_and_diagnostics")) { return; }
    for flags in 0..64 {
        for seed in [0, 1, 11, 22, u32::MAX] { check(flags, seed); }
    }
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..Default::default()
    });
    runner.run(&(0u32..64, any::<u32>()), |(flags, seed)| { check(flags, seed); Ok(()) }).unwrap();
}
