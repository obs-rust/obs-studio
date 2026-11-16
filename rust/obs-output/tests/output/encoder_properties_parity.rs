//! Exact dispatch/settings parity against the extracted original C constructors.
use core::ffi::{c_char, c_void};
use core::mem::{align_of, offset_of, size_of};
use obs_output::ffi::encoder_properties::{
    PropertiesFields, obs_encoder_properties, obs_get_encoder_properties,
};
use proptest::prelude::*;

type TypeGetter = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type InstanceGetter = unsafe extern "C" fn(*const c_void) -> *mut c_void;
unsafe extern "C" {
    fn oracle_obs_get_encoder_properties(id: *const c_char) -> *mut c_void;
    fn oracle_obs_encoder_properties(encoder: *const c_void) -> *mut c_void;
    fn oracle_encoder_properties_run(
        type_getter: TypeGetter,
        instance_getter: InstanceGetter,
        flags: u32,
        seed: u32,
        value: u32,
        instance: bool,
    ) -> u64;
    fn oracle_encoder_properties_layout(layout: *mut usize);
}

#[test]
fn real_header_layout() {
    let mut actual = [0usize; 9];
    // SAFETY: the real-header oracle writes exactly nine size_t entries.
    unsafe { oracle_encoder_properties_layout(actual.as_mut_ptr()) };
    assert_eq!(
        actual,
        [
            size_of::<PropertiesFields>(),
            align_of::<PropertiesFields>(),
            offset_of!(PropertiesFields, properties),
            offset_of!(PropertiesFields, properties2),
            offset_of!(PropertiesFields, defaults),
            offset_of!(PropertiesFields, defaults2),
            offset_of!(PropertiesFields, type_data),
            offset_of!(PropertiesFields, data),
            offset_of!(PropertiesFields, settings)
        ]
    );
}

fn check(flags: u32, seed: u32, value: u32, instance: bool) {
    // SAFETY: each run owns and releases its live private C fixture and settings;
    // the isolated test process prevents access to its registry by other tests.
    unsafe {
        let expected = oracle_encoder_properties_run(
            oracle_obs_get_encoder_properties,
            oracle_obs_encoder_properties,
            flags,
            seed,
            value,
            instance,
        );
        let actual = oracle_encoder_properties_run(
            obs_get_encoder_properties,
            obs_encoder_properties,
            flags,
            seed,
            value,
            instance,
        );
        assert_eq!(
            actual, expected,
            "flags={flags}, seed={seed}, value={value}, instance={instance}"
        );
    }
}

#[test]
fn callback_defaults_settings_parity() {
    if !super::isolated(concat!(
        module_path!(),
        "::callback_defaults_settings_parity"
    )) {
        return;
    }
    for flags in 0..128 {
        for instance in [false, true] {
            check(flags, u32::MAX, 0, instance);
        }
    }
    let config = proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Default::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(config);
    runner
        .run(
            &(0u32..128, any::<u32>(), any::<u32>(), any::<bool>()),
            |(flags, seed, value, instance)| {
                check(flags, seed, value, instance);
                Ok(())
            },
        )
        .unwrap();
}
