//! `oracle_*` build of `libobs/obs-encoder-audio-start-frames.c.h`
//! (oracle/encoder_audio_start_frames.c): the two sample-rate conversions of
//! the encoder's audio start, renamed.
//!
//! `oracle_obs_encoder_audio_start_frames_duration` divides by `samplerate`:
//! callers keep it above 0. Both keep the C's `util_mul_div64()`, so on
//! MSVC x64 callers keep the quotient within 64 bits.

unsafe extern "C" {
    pub fn oracle_obs_encoder_audio_start_frames_offset(offset_ns: u64, samplerate: u32) -> u64;

    /// Divides by `samplerate`: never 0.
    pub fn oracle_obs_encoder_audio_start_frames_duration(frames: u32, samplerate: u32) -> u64;
}
