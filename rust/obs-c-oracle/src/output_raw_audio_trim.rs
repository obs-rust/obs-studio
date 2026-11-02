//! `oracle_*` build of `libobs/obs-output-raw-audio-trim.c.h`
//! (oracle/output_raw_audio_trim.c): the two sample-rate conversions of
//! `prepare_audio()`, renamed.
//!
//! `oracle_obs_output_raw_audio_trim_duration` divides by `sample_rate`:
//! callers keep it above 0. Both keep the C's `util_mul_div64()`, so on
//! MSVC x64 callers keep the quotient within 64 bits.

unsafe extern "C" {
    /// Divides by `sample_rate`: never 0.
    pub fn oracle_obs_output_raw_audio_trim_duration(frames: u32, sample_rate: usize) -> u64;

    pub fn oracle_obs_output_raw_audio_trim_frames(cut_ns: u64, sample_rate: usize) -> u64;
}
