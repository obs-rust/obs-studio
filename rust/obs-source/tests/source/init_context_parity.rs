//! Tier 3: real private C layout, original C unit and production adapter. No exclusions.
use core::ffi::{c_char, c_void};
use obs_source_seam::ffi::source_init_context::obs_source_init_context;
use proptest::prelude::*;
use std::sync::Mutex;
static LOCK: Mutex<()> = Mutex::new(());
type Init = unsafe extern "C" fn(
    *mut c_void,
    *mut c_void,
    *const c_char,
    *const c_char,
    *mut c_void,
    bool,
) -> bool;
unsafe extern "C" {
    fn oracle_source_init_context_check(
        shim: Init,
        init: bool,
        registration: bool,
        populated: bool,
        private: bool,
    ) -> bool;
}
fn check(init: bool, registration: bool, populated: bool, private: bool) {
    let _guard = LOCK.lock().unwrap();
    // SAFETY: C owns the real source fixture and borrowed tokens for each call;
    // the lock serializes the controlled dependency state.
    assert!(unsafe {
        oracle_source_init_context_check(
            obs_source_init_context,
            init,
            registration,
            populated,
            private,
        )
    });
}
#[test]
fn characterized_edges() {
    for init in [false, true] {
        for registration in [false, true] {
            for populated in [false, true] {
                for private in [false, true] {
                    check(init, registration, populated, private);
                }
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]
    #[test]
    fn parity(init: bool, registration: bool, populated: bool, private: bool) {
        check(init, registration, populated, private);
    }
}
