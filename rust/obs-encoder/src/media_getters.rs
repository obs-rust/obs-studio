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
    _encoder: Option<Media<T>>,
    _getter: Getter,
    _log: impl FnMut(i32, &'static CStr),
) -> Option<T> {
    todo!("RED: encoder media getters")
}
