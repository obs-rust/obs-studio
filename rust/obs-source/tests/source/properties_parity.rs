//! Tier 3: real C fields, unchanged constructor bodies, and actual settings/properties.
use core::ffi::{c_char, c_void};
use obs_source::ffi::source_properties::{
    Fields, obs_get_source_properties, obs_source_properties,
};
use proptest::prelude::*;
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());
unsafe extern "C" {
    fn oracle_source_properties_check(
        type_fn: unsafe extern "C" fn(*const c_char) -> *mut c_void,
        instance_fn: unsafe extern "C" fn(*const c_void) -> *mut c_void,
        flags: u32,
        value: i64,
    ) -> bool;
    fn oracle_source_properties_layout(layout: *mut usize);
}

fn check(flags: u32, value: i64) {
    let _guard = LOCK.lock().unwrap();
    // SAFETY: oracle owns live C objects and invokes only these ABI-compatible shims.
    assert!(unsafe {
        oracle_source_properties_check(
            obs_get_source_properties,
            obs_source_properties,
            flags,
            value,
        )
    });
}

#[test]
fn real_header_layout() {
    let mut c = [0; 9];
    // SAFETY: the oracle writes exactly nine size_t entries to this array.
    unsafe {
        oracle_source_properties_layout(c.as_mut_ptr());
    }
    assert_eq!(
        c,
        [
            size_of::<Fields>(),
            align_of::<Fields>(),
            core::mem::offset_of!(Fields, legacy),
            core::mem::offset_of!(Fields, modern),
            core::mem::offset_of!(Fields, defaults),
            core::mem::offset_of!(Fields, defaults2),
            core::mem::offset_of!(Fields, type_data),
            core::mem::offset_of!(Fields, data),
            core::mem::offset_of!(Fields, settings),
        ]
    );
}

#[test]
fn characterized_edges() {
    for flags in 0..64 {
        for value in [i64::MIN, -1, 0, 17, 29, 83, i64::MAX] {
            check(flags, value);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn callback_and_settings_parity(flags in 0_u32..64, value in any::<i64>()) {
        check(flags, value);
    }
}
