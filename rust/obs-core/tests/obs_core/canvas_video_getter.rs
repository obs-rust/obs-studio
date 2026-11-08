//! Tier 1 mirror of test_canvas_video_getter.c; no unsafe or FFI types.
use obs_core::canvas_video_getter::borrowed_video;

#[test]
fn borrowed_video_matches_c_characterization() {
    let mut tokens = [0u8; 2];
    let pointers = [
        core::ptr::null_mut(),
        &mut tokens[0] as *mut u8,
        &mut tokens[1] as *mut u8,
    ];
    assert!(borrowed_video::<u8>(None).is_null());
    for video in pointers {
        let before = video;
        assert_eq!(borrowed_video(Some(&video)), video);
        assert_eq!(borrowed_video(Some(&video)), video);
        assert_eq!(video, before);
    }
    assert_eq!(tokens, [0, 0]);
}
