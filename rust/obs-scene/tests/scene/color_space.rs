//! Tier 1: the four states in test_scene_color_space.c, plus enum edges.
use obs_scene::color_space::{
    GS_CS_709_EXTENDED, GS_CS_SRGB, VIDEO_CS_2100_HLG, VIDEO_CS_2100_PQ, VIDEO_CS_SRGB,
    video_color_space,
};

#[test]
fn test_scene_color_space_no_video_defaults_to_srgb() {
    assert_eq!(video_color_space(None), GS_CS_SRGB);
}

#[test]
fn test_scene_color_space_srgb_ignores_hdr_preference() {
    assert_eq!(video_color_space(Some(VIDEO_CS_SRGB)), GS_CS_SRGB);
}

#[test]
fn test_scene_color_space_pq_ignores_srgb_preference() {
    assert_eq!(
        video_color_space(Some(VIDEO_CS_2100_PQ)),
        GS_CS_709_EXTENDED
    );
}

#[test]
fn test_scene_color_space_hlg_ignores_srgb_preference() {
    assert_eq!(
        video_color_space(Some(VIDEO_CS_2100_HLG)),
        GS_CS_709_EXTENDED
    );
}

#[test]
fn other_video_spaces_default_to_srgb() {
    for video in [i32::MIN, -1, 0, 1, 2, 6, i32::MAX] {
        assert_eq!(video_color_space(Some(video)), GS_CS_SRGB);
    }
}
