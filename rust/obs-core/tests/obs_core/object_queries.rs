//! Tier 1 mirror of test_object_queries.c plus raw type-bit edge cases.
use obs_core::object_queries::{is_private, object_type};

#[test]
fn queries_match_c_characterization() {
    assert_eq!(object_type(None), 0);
    assert!(!is_private(None));
    for kind in 0..=5 {
        for privacy in [false, true] {
            let before = (kind, privacy);
            assert_eq!(object_type(Some(&kind)), kind);
            assert_eq!(is_private(Some(&privacy)), privacy);
            assert_eq!(object_type(Some(&kind)), kind);
            assert_eq!(is_private(Some(&privacy)), privacy);
            assert_eq!((kind, privacy), before);
        }
    }
}

#[test]
fn stored_type_bits_are_preserved() {
    for kind in [i32::MIN, -1, 6, 127, i32::MAX] {
        assert_eq!(object_type(Some(&kind)), kind);
    }
}
