//! Mirrors test_encoder_preferred_range.c (#4105).
use obs_output::encoder_preferred_range::get;

#[test]
fn mirrors_null_default() {
    assert_eq!(get(None), 0);
}

#[test]
fn mirrors_stored_video_and_audio_ranges() {
    for range in [0, 1, 2] {
        assert_eq!(get(Some((1, range))), range);
        assert_eq!(get(Some((0, range))), 0);
    }
}

#[test]
fn preserves_all_video_range_bits_and_rejects_other_kinds() {
    for range in [3, i32::MAX as u32, 0x8000_0000, u32::MAX] {
        assert_eq!(get(Some((1, range))), range);
        for kind in [-1, 0, 2, i32::MIN, i32::MAX] {
            assert_eq!(get(Some((kind, range))), 0);
        }
    }
}
