//! Original C requires a live service; NULL field values are valid.
//! No representation of obs_service crosses into Rust. Address checks measure
//! the production adapter against fields of the real private-header object.
#![allow(unsafe_code)]
use obs_c_oracle::service_getters::{
    oracle_service_free, oracle_service_new, oracle_service_set_context,
};
use obs_output::ffi::{service::ObsServiceInfo, service_context_getters as rust};
use proptest::prelude::*;
use std::ffi::{c_char, c_void};

unsafe extern "C" {
    fn oracle_context_name(service: *const c_void) -> *const c_char;
    fn oracle_context_settings(service: *const c_void) -> *mut c_void;
    fn oracle_context_field_addresses(service: *const c_void) -> bool;
    fn oracle_context_snapshot(service: *const c_void) -> *mut c_void;
    fn oracle_context_unchanged(service: *const c_void, snapshot: *mut c_void) -> bool;
}
struct Service(*mut c_void);
impl Drop for Service {
    fn drop(&mut self) {
        // SAFETY: owns the allocation; borrowed context fields are caller-owned.
        unsafe { oracle_service_free(self.0) }
    }
}
fn compare(name_null: bool, settings_null: bool, replacement: bool) {
    let mut name_a = *b"same\0";
    let mut name_b = *b"same\0";
    let mut settings_a = 17u64;
    let mut settings_b = 29u64;
    // SAFETY: zero is a valid empty callback table. The C fixture creates and
    // frees a real service, without invoking callbacks or freeing borrowed fields.
    unsafe {
        let info: ObsServiceInfo = std::mem::zeroed();
        let service = Service(oracle_service_new(
            (&info as *const ObsServiceInfo).cast(),
            std::ptr::null_mut(),
        ));
        assert!(!service.0.is_null());
        for second in [false, replacement] {
            let name = if name_null {
                std::ptr::null_mut()
            } else if second {
                name_b.as_mut_ptr().cast()
            } else {
                name_a.as_mut_ptr().cast()
            };
            let settings = if settings_null {
                std::ptr::null_mut()
            } else if second {
                (&mut settings_b as *mut u64).cast()
            } else {
                (&mut settings_a as *mut u64).cast()
            };
            oracle_service_set_context(service.0, name, settings);
            assert!(oracle_context_field_addresses(service.0));
            let snapshot = oracle_context_snapshot(service.0);
            for _ in 0..3 {
                assert_eq!(
                    rust::obs_service_get_context_name(service.0),
                    oracle_context_name(service.0)
                );
                assert_eq!(
                    rust::obs_service_get_context_name(service.0),
                    name.cast_const()
                );
                assert_eq!(
                    rust::obs_service_get_context_settings(service.0),
                    oracle_context_settings(service.0)
                );
                assert_eq!(rust::obs_service_get_context_settings(service.0), settings);
            }
            assert!(oracle_context_unchanged(service.0, snapshot));
        }
    }
    assert_eq!(name_a, *b"same\0");
    assert_eq!(name_b, *b"same\0");
    assert_eq!((settings_a, settings_b), (17, 29));
}
#[test]
fn real_private_layout_and_nullable_fields() {
    for name_null in [false, true] {
        for settings_null in [false, true] {
            compare(name_null, settings_null, true);
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(64)))]
    #[test]
    fn borrowed_pointer_and_state_parity(name_null in any::<bool>(), settings_null in any::<bool>(), replacement in any::<bool>()) {
        compare(name_null, settings_null, replacement);
    }
}
