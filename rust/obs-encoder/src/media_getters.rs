//! Borrowed media selection, independent of the private encoder layout.
use std::ffi::CStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Getter {
    Video,
    ParentVideo,
    Audio,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Media<T> {
    pub encoder_type: i32,
    pub parent: Option<T>,
    pub fps_override: Option<T>,
}

/// Select a borrowed identity and report the original diagnostic if refused.
/// The diagnostic's only substitution is the encoder name (ignored for NULL).
pub fn get<T: Copy>(
    encoder: Option<Media<T>>,
    getter: Getter,
    mut log: impl FnMut(i32, &'static CStr),
) -> Option<T> {
    let Some(encoder) = encoder else {
        log(
            400,
            match getter {
                Getter::Video => c"obs_encoder_video: Null 'encoder' parameter",
                Getter::ParentVideo => c"obs_encoder_parent_video: Null 'encoder' parameter",
                Getter::Audio => c"obs_encoder_audio: Null 'encoder' parameter",
            },
        );
        return None;
    };
    let expected_type = if getter == Getter::Audio { 0 } else { 1 };
    if encoder.encoder_type != expected_type {
        log(
            200,
            match getter {
                Getter::Video => c"obs_encoder_set_video: encoder '%s' is not a video encoder",
                Getter::ParentVideo => {
                    c"obs_encoder_parent_video: encoder '%s' is not a video encoder"
                }
                Getter::Audio => c"obs_encoder_set_audio: encoder '%s' is not an audio encoder",
            },
        );
        return None;
    }
    match getter {
        Getter::Video => encoder.fps_override.or(encoder.parent),
        Getter::ParentVideo | Getter::Audio => encoder.parent,
    }
}
