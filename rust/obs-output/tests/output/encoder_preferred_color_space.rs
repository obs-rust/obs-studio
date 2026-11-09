//! Mirrors test_encoder_preferred_color_space.c (#4114).
use obs_output::encoder_preferred_color_space::get;

#[test]
fn mirrors_null_default() {
    assert_eq!(get(None), 0);
}

#[test]
fn mirrors_stored_video_and_audio_spaces() {
    for space in 0..=5 {
        assert_eq!(get(Some((1, space))), space);
        assert_eq!(get(Some((0, space))), 0);
    }
}

#[test]
fn preserves_all_space_bits_and_rejects_other_kinds() {
    for space in [6, i32::MAX as u32, 0x8000_0000, u32::MAX] {
        assert_eq!(get(Some((1, space))), space);
        for kind in [-1, 0, 2, i32::MIN, i32::MAX] {
            assert_eq!(get(Some((kind, space))), 0);
        }
    }
}
