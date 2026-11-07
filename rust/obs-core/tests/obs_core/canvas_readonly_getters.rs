//! Tier 1 mirror of test_canvas_readonly_getters.c.
use obs_core::canvas_readonly_getters::{borrowed, has_video};

#[test]
fn borrowed_fields() {
    for mask in 0u8..16 {
        let name = (mask & 1 != 0).then_some("canvas name");
        let uuid = (mask & 2 != 0).then_some("12345678-1234-1234-1234-123456789abc");
        let signals = (mask & 4 != 0).then_some(17usize);
        let mix = (mask & 8 != 0).then_some(());
        let before = (name, uuid, signals, mix);
        assert_eq!(borrowed(&name), name);
        assert_eq!(borrowed(&uuid), uuid);
        assert_eq!(borrowed(&signals), signals);
        assert_eq!(has_video(mix), mask & 8 != 0);
        assert_eq!((name, uuid, signals, mix), before);
    }
}

#[test]
fn distinct_identity_values_are_preserved() {
    for value in [0, 1, usize::MAX / 2, usize::MAX] {
        assert_eq!(borrowed(&value), value);
    }
    assert!(has_video(Some(0usize)));
}
