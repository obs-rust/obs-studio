//! Mirrors test_encoder_output_getters_characterization.c.
use obs_output::encoder_preferred_format::get;

#[test]
fn mirrors_video_default_and_nv12_after_set() {
    assert_eq!(get(Some((1, 0))), 0); // VIDEO_FORMAT_NONE
    assert_eq!(get(Some((1, 2))), 2); // VIDEO_FORMAT_NV12
}

#[test]
fn mirrors_audio_and_null_defaults() {
    assert_eq!(get(None), 0);
    assert_eq!(get(Some((0, 0))), 0);
    assert_eq!(get(Some((0, 2))), 0);
}

#[test]
fn preserves_all_format_bits() {
    for format in [1, 3, 24, i32::MAX as u32, u32::MAX] {
        assert_eq!(get(Some((1, format))), format);
    }
}

#[test]
fn rejects_non_video_kinds_even_with_stored_format() {
    for kind in [0, 2, -1, i32::MIN, i32::MAX] {
        assert_eq!(get(Some((kind, u32::MAX))), 0);
    }
}
