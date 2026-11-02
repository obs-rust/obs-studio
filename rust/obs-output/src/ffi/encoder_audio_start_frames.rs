//! C ABI for `libobs/obs-encoder-audio-start-frames.c.h`, the two
//! sample-rate conversions of the encoder's audio start in `obs-encoder.c`,
//! declared in `libobs/obs-encoder-audio-start-frames.h` without `EXPORT`.
//!
//! `obs-encoder.c.h` (still C) keeps every read of the encoder, the paired
//! encoder lookup, the deques and every rule call, and passes plain values
//! here. So `struct obs_encoder` is never mirrored, and these shims take no
//! lock, call nothing in libobs and dereference no pointer. They stay hidden
//! (`rust-hidden.d`).
//!
//! Intentional difference: [`obs_encoder_audio_start_frames_duration`] at a
//! sample rate of 0 is 0, where the C divides by zero.

use crate::encoder_audio_start_frames as core;

/// `uint64_t obs_encoder_audio_start_frames_offset(uint64_t offset_ns,
/// uint32_t samplerate)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_frames_offset(offset_ns: u64, samplerate: u32) -> u64 {
    core::offset(offset_ns, samplerate)
}

/// `uint64_t obs_encoder_audio_start_frames_duration(uint32_t frames,
/// uint32_t samplerate)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_audio_start_frames_duration(frames: u32, samplerate: u32) -> u64 {
    core::duration(frames, samplerate)
}
