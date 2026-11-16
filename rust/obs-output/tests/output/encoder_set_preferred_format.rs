//! Mirrors video/audio cases in test_encoder_output_getters_characterization.c.
use obs_output::encoder_set_preferred_format::set;

#[test]
fn mirrors_video_nv12_storage() {
    let mut stored = 0;
    set(Some((1, &mut stored)), 2);
    assert_eq!(stored, 2);
}

#[test]
fn mirrors_audio_noop() {
    let mut stored = 0;
    set(Some((0, &mut stored)), 2);
    assert_eq!(stored, 0);
}

#[test]
fn null_is_noop() {
    set(None, u32::MAX);
}

#[test]
fn video_preserves_all_format_bits_and_other_types_do_not_write() {
    for kind in [0, 1, 2, -1, i32::MIN, i32::MAX] {
        for format in [0, 1, 2, 24, 0x8000_0000, u32::MAX] {
            let mut stored = 17;
            set(Some((kind, &mut stored)), format);
            assert_eq!(stored, if kind == 1 { format } else { 17 });
        }
    }
}
