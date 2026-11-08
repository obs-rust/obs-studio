//! Borrowed codec metadata queries; no string decoding or ownership transfer.
use core::ffi::CStr;

#[derive(Clone, Copy)]
pub struct Codecs<T> {
    pub video: T,
    pub audio: T,
}

pub fn video<T: Copy + Default>(
    _output: Option<Codecs<T>>,
    _log: impl FnOnce(&'static CStr),
) -> T {
    todo!("port obs_output_get_supported_video_codecs")
}

pub fn audio<T: Copy + Default>(
    _output: Option<Codecs<T>>,
    _log: impl FnOnce(&'static CStr),
) -> T {
    todo!("port obs_output_get_supported_audio_codecs")
}
