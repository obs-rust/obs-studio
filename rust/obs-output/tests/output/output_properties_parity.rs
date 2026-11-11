use core::ffi::{c_char, c_void};
use obs_output::ffi::output_properties::{
    Fields, obs_get_output_properties, obs_output_defaults, obs_output_properties,
};
use proptest::prelude::*;
unsafe extern "C" {
    fn oracle_output_properties_compare(
        mask: u32,
        op: u32,
        id: u32,
        value: i32,
        defaults: unsafe extern "C" fn(*const c_char) -> *mut c_void,
        props: unsafe extern "C" fn(*const c_char) -> *mut c_void,
        instance: unsafe extern "C" fn(*const c_void) -> *mut c_void,
    ) -> bool;
    fn oracle_output_properties_layout(values: *mut usize);
}
#[test]
fn callback_ownership_settings_traces() {
    if !super::isolated(concat!(
        module_path!(),
        "::callback_ownership_settings_traces"
    )) {
        return;
    }
    let compare = |mask, op, id, value| {
        // SAFETY: C owns and releases the fixture; all callbacks have the real ABI.
        unsafe {
            oracle_output_properties_compare(
                mask,
                op,
                id,
                value,
                obs_output_defaults,
                obs_get_output_properties,
                obs_output_properties,
            )
        }
    };
    for mask in 0..16 {
        for op in 0..4 {
            for id in 0..3 {
                assert!(compare(mask, op, id, 43), "mask={mask} op={op} id={id}");
            }
        }
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128),
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(0u32..16, 0u32..4, 0u32..3, any::<i32>()),
            |(mask, op, id, value)| {
                prop_assert!(compare(mask, op, id, value));
                Ok(())
            },
        )
        .unwrap();
}
#[test]
fn private_snapshot_layout() {
    let mut c = [0; 6];
    // SAFETY: oracle writes six size_t entries to this array.
    unsafe { oracle_output_properties_layout(c.as_mut_ptr()) };
    assert_eq!(
        c,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            std::mem::offset_of!(Fields, defaults),
            std::mem::offset_of!(Fields, properties),
            std::mem::offset_of!(Fields, data),
            std::mem::offset_of!(Fields, settings)
        ]
    );
}
