//! Exact original-C parity over real-header fixtures. No intentional differences.
use obs_core::ffi::module_type_queries::{obs_is_core_module, obs_is_legacy_module};
use proptest::prelude::*;
use std::ffi::{c_uint, c_void};

type Query = unsafe extern "C" fn(*mut c_void) -> bool;
unsafe extern "C" {
    fn oracle_module_type_queries_check(word: u32, core: Query, legacy: Query) -> bool;
    fn oracle_module_type_queries_layout(size: *mut usize, align: *mut usize);
}

fn check(word: u32) {
    // SAFETY: C owns the real-header fixture and calls these callbacks only
    // while its module lives. NULL and field-address checks are done in C.
    assert!(unsafe {
        oracle_module_type_queries_check(word, obs_is_core_module, obs_is_legacy_module)
    });
}

#[test]
fn real_header_enum_layout() {
    let (mut size, mut align) = (0, 0);
    // SAFETY: C writes two measured scalar layout values to valid outputs.
    unsafe { oracle_module_type_queries_layout(&mut size, &mut align) };
    assert_eq!(size, size_of::<c_uint>());
    assert_eq!(align, align_of::<c_uint>());
}

#[test]
fn characterized_edges() {
    for word in [0, 1, 2, 3, 4, u32::MAX, i32::MAX as u32, 1 << 31] {
        check(word);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn types_match_original_c(word in any::<u32>()) {
        check(word);
    }
}
