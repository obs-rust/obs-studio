//! Borrowed codec metadata queries; no string decoding or ownership transfer.
use core::ffi::CStr;

#[derive(Clone, Copy)]
pub struct Codecs<T> {
    pub video: T,
    pub audio: T,
}

pub fn video<T: Copy + Default>(output: Option<Codecs<T>>, log: impl FnOnce(&'static CStr)) -> T {
    match output {
        Some(fields) => fields.video,
        None => {
            log(c"obs_output_get_supported_video_codecs");
            T::default()
        }
    }
}

pub fn audio<T: Copy + Default>(output: Option<Codecs<T>>, log: impl FnOnce(&'static CStr)) -> T {
    match output {
        Some(fields) => fields.audio,
        None => {
            log(c"obs_output_get_supported_audio_codecs");
            T::default()
        }
    }
}
