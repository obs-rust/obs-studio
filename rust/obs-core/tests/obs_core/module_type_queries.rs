//! Tier 1 mirrors test_module_load_steps::test_is_core_legacy_null and
//! the loaded-module assertions in test_core_module_load, plus all type words.
use obs_core::module_type_queries::{is_core, is_legacy};

#[test]
fn test_is_core_legacy_null() {
    assert!(!is_core(None));
    assert!(!is_legacy(None));
}

#[test]
fn exact_type_classification() {
    for word in [0, 1, 2, 3, 4, u32::MAX, i32::MAX as u32, 1 << 31] {
        assert_eq!(is_core(Some(word)), word == 1);
        assert_eq!(is_legacy(Some(word)), word == 3);
    }
}
