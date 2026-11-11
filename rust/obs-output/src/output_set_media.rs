//! Media assignment rules, preserving the C output's flag gates and diagnostics.
use crate::video_checks::{OBS_OUTPUT_AUDIO, OBS_OUTPUT_ENCODED, OBS_OUTPUT_VIDEO};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    NullOutput,
    EncodedOutput,
}

pub struct Media<'a, T> {
    pub flags: u32,
    pub video: &'a mut T,
    pub audio: &'a mut T,
}

pub fn set_media<T>(
    output: Option<Media<'_, T>>,
    video: T,
    audio: T,
    log: impl FnOnce(Diagnostic),
) {
    let Some(output) = output else {
        log(Diagnostic::NullOutput);
        return;
    };
    if output.flags & OBS_OUTPUT_ENCODED != 0 {
        log(Diagnostic::EncodedOutput);
        return;
    }
    if output.flags & OBS_OUTPUT_VIDEO != 0 {
        *output.video = video;
    }
    if output.flags & OBS_OUTPUT_AUDIO != 0 {
        *output.audio = audio;
    }
}
