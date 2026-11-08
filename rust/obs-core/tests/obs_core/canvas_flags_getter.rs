//! Tier 1 mirror of test_canvas_flags_getter.c.
use obs_core::canvas_flags_getter::stored_flags;

#[test]
fn removed_matches_canvas_flags_and_rules_callers_characterization() {
    use obs_core::canvas_flags_getter::stored_removed;
    use obs_core::canvas_rules::REMOVED;

    for flags in [0, 1, 2, 4, 8, 16, 0x7fff_ffff] {
        assert!(!stored_removed(&flags));
        let removed = flags | REMOVED;
        assert!(stored_removed(&removed));
        assert_eq!(removed, flags | REMOVED);
    }
}

#[test]
fn stored_flags_match_c_characterization() {
    for value in [
        0,
        1,
        2,
        4,
        8,
        16,
        32,
        64,
        128,
        0x8000_0000,
        0xa5a5_a5a5,
        u32::MAX,
    ] {
        let flags = value;
        assert_eq!(stored_flags(&flags), value);
        assert_eq!(flags, value);
    }
}

#[test]
fn every_single_bit_is_retained() {
    for bit in 0..32 {
        let flags = 1u32 << bit;
        assert_eq!(stored_flags(&flags), flags);
    }
}
