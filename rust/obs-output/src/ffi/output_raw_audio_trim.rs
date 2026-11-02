//! C ABI for `libobs/obs-output-raw-audio-trim.c.h`, the two sample-rate
//! conversions of `prepare_audio()` in `obs-output.c`, declared in
//! `libobs/obs-output-raw-audio-trim.h` without `EXPORT`.
//!
//! `obs-output.c.h` (still C) keeps the passthrough, the video start taken
//! from the pause data under the pause mutex, the copy of the packet, the
//! before-video test, the drop test, the cut in nanoseconds and the new
//! timestamp, each plane's pointer advance and the new frame count, and
//! passes plain values here. So `struct obs_output` is never mirrored, and these shims take no
//! lock, call nothing in libobs and dereference no pointer. They stay hidden
//! (`rust-hidden.d`).
//!
//! Intentional difference: [`obs_output_raw_audio_trim_duration`] at a
//! sample rate of 0 is 0, where the C divides by zero; the caller then drops
//! the packet.

use crate::raw_audio_trim as core;

/// `uint64_t obs_output_raw_audio_trim_duration(uint32_t frames, size_t
/// sample_rate)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_trim_duration(frames: u32, sample_rate: usize) -> u64 {
    core::duration(frames, sample_rate)
}

/// `uint64_t obs_output_raw_audio_trim_frames(uint64_t cut_ns, size_t
/// sample_rate)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_output_raw_audio_trim_frames(cut_ns: u64, sample_rate: usize) -> u64 {
    core::frames(cut_ns, sample_rate)
}
