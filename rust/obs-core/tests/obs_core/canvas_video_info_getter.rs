//! Mirrors test_canvas_video_info_getter.c without FFI or unsafe code.
use obs_core::canvas_video_info_getter::video_info;

#[test]
fn unavailable_graphics_preserves_output_and_short_circuits() {
    let output = [0xa5; 64];
    assert_eq!(video_info::<[u8; 64]>(false, || panic!("canvas must not be read")), None);
    assert_eq!(output, [0xa5; 64]);
}

#[test]
fn absent_mix_and_all_characterized_output_bytes() {
    assert_eq!(video_info::<[u8; 64]>(true, || None), None);
    for value in 0..=u8::MAX {
        let info = [value; 64];
        assert_eq!(video_info(true, || Some(&info)), Some(&info));
    }
}
