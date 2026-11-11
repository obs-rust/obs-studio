//! Tier 1 mirrors every assertion in test_encoder_media_getters.c.
use obs_encoder::media_getters::{Getter, Media, get};
use std::ffi::CStr;

#[test]
fn test_encoder_media_getters_null_encoders() {
    for (getter, message) in [
        (Getter::Video, c"obs_encoder_video: Null 'encoder' parameter"),
        (Getter::ParentVideo, c"obs_encoder_parent_video: Null 'encoder' parameter"),
        (Getter::Audio, c"obs_encoder_audio: Null 'encoder' parameter"),
    ] {
        let mut logs = Vec::new();
        assert_eq!(get::<usize>(None, getter, |level, text| logs.push((level, text))), None);
        assert_eq!(logs, [(400, message)]);
    }
}

fn check(getter: Getter, encoder_type: i32, parent: Option<usize>, fps_override: Option<usize>) {
    let encoder = Media { encoder_type, parent, fps_override };
    let before = encoder;
    let mut logs = Vec::new();
    let result = get(Some(encoder), getter, |level, text| logs.push((level, text)));
    let expected_type = if getter == Getter::Audio { 0 } else { 1 };
    let expected = if encoder_type != expected_type {
        None
    } else if getter == Getter::Video {
        fps_override.or(parent)
    } else {
        parent
    };
    assert_eq!(result, expected);
    let warning: &CStr = match getter {
        Getter::Video => c"obs_encoder_set_video: encoder '%s' is not a video encoder",
        Getter::ParentVideo => c"obs_encoder_parent_video: encoder '%s' is not a video encoder",
        Getter::Audio => c"obs_encoder_set_audio: encoder '%s' is not an audio encoder",
    };
    if encoder_type != expected_type {
        assert_eq!(logs, [(200, warning)]);
    } else {
        assert!(logs.is_empty());
    }
    assert_eq!(encoder, before);
}

#[test]
fn test_encoder_media_getters_video_getters() {
    for getter in [Getter::Video, Getter::ParentVideo] {
        for encoder_type in [0, 1, 2, i32::MAX] {
            for parent in [None, Some(11)] {
                for fps_override in [None, Some(22), parent] {
                    check(getter, encoder_type, parent, fps_override);
                }
            }
        }
    }
}

#[test]
fn test_encoder_media_getters_audio_getter() {
    for encoder_type in [0, 1, 2, i32::MAX] {
        for parent in [None, Some(11)] {
            for fps_override in [None, Some(22), parent] {
                check(Getter::Audio, encoder_type, parent, fps_override);
            }
        }
    }
}
