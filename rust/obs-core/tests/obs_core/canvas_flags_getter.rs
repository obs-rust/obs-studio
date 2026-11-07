//! Tier 1 mirror of test_canvas_flags_getter.c.
use obs_core::canvas_flags_getter::stored_flags;

#[test]
fn stored_flags_match_c_characterization() {
    for value in [0, 1, 2, 4, 8, 16, 32, 64, 128, 0x8000_0000, 0xa5a5_a5a5, u32::MAX] {
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
