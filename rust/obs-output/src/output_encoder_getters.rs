//! Safe core of `libobs/obs-output-encoder-getters.c.h`: the borrowed
//! first video encoder and indexed audio encoder of an output.
use core::ffi::CStr;

/// `obs_output_get_video_encoder()`.
pub fn video_encoder<T: Default>(
    valid: bool,
    slots: usize,
    occupied: impl FnMut(usize) -> bool,
    get: impl FnOnce(usize) -> T,
    log: impl FnOnce(&'static CStr),
) -> T {
    let _ = (valid, slots, occupied, get, log);
    todo!("obs_output_get_video_encoder")
}

/// `obs_output_get_audio_encoder()`.
pub fn audio_encoder<T: Default>(
    valid: bool,
    idx: usize,
    read: impl FnOnce(usize) -> T,
    log: impl FnOnce(&'static CStr),
) -> T {
    let _ = (valid, idx, read, log);
    todo!("obs_output_get_audio_encoder")
}
