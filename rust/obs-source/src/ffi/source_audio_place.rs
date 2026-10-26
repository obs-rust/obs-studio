//! C ABI for `libobs/obs-source-audio-place.c.h`, the source audio
//! placement and action timeline of `source_output_audio_place()`,
//! `source_output_audio_push_back()`, `source_output_audio_data()` and
//! `apply_audio_actions()`, declared in `libobs/obs-source-audio-place.h`
//! without `EXPORT`.
//!
//! Those functions in `obs-source.c.h` (still C) read the source's fields
//! themselves, each at its point, pass the values to these shims and act on
//! what comes back: both mutexes, the deques, `reset_audio_data()`,
//! `apply_audio_action()`, every `get_source_volume()` call, the volume
//! array and `multiply_vol_data()` stay there. So `struct obs_source` is
//! never mirrored, no pointer crosses, and the shims take no lock, call
//! nothing in libobs and touch no memory. All eleven stay hidden
//! (`rust-hidden.txt`).
//!
//! The monitoring type crosses as `int`, as the C passes `enum
//! obs_monitoring_type` (int-sized, checked in `obs-source.c.h`).
//!
//! Intentional differences from C, both on inputs libobs never passes:
//! `obs_source_audio_place_mix_on()` with a mix at or past 32 (the C's
//! `int` shift is undefined from 31 on) says no mix, and on MSVC x64 a
//! frame count past 64 bits (a rate above 1e9 Hz), where the C's
//! `_udiv128()` faults, saturates.

use std::os::raw::c_int;

use crate::audio_place as core;

/// `bool obs_source_audio_place_started(uint64_t audio_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_place_started(audio_ts: u64) -> bool {
    core::started(audio_ts)
}

/// `bool obs_source_audio_place_older(uint64_t timestamp, uint64_t
/// audio_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_place_older(timestamp: u64, audio_ts: u64) -> bool {
    core::older(timestamp, audio_ts)
}

/// `size_t obs_source_audio_place_size(uint32_t frames)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_place_size(frames: u32) -> usize {
    core::size(frames)
}

/// `size_t obs_source_audio_place_offset(uint64_t timestamp, uint64_t
/// audio_ts, uint32_t sample_rate)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_place_offset(
    timestamp: u64,
    audio_ts: u64,
    sample_rate: u32,
) -> usize {
    core::offset(timestamp, audio_ts, sample_rate)
}

/// `bool obs_source_audio_place_fits(size_t start, size_t size)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_place_fits(start: usize, size: usize) -> bool {
    core::fits(start, size)
}

/// `size_t obs_source_audio_place_pop_back(size_t buf_size, size_t
/// placement, size_t size)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_place_pop_back(
    buf_size: usize,
    placement: usize,
    size: usize,
) -> usize {
    core::pop_back(buf_size, placement, size)
}

/// `bool obs_source_audio_place_outputs(int monitoring_type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_place_outputs(monitoring_type: c_int) -> bool {
    core::outputs(monitoring_type)
}

/// `size_t obs_source_audio_place_action_frame(uint64_t timestamp, uint64_t
/// audio_ts, size_t sample_rate)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_place_action_frame(
    timestamp: u64,
    audio_ts: u64,
    sample_rate: usize,
) -> usize {
    core::action_frame(timestamp, audio_ts, sample_rate)
}

/// `bool obs_source_audio_place_action_stops(size_t frame)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_place_action_stops(frame: usize) -> bool {
    core::action_stops(frame)
}

/// `size_t obs_source_audio_place_fill_end(size_t frame_num, size_t
/// new_frame_num)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_place_fill_end(frame_num: usize, new_frame_num: usize) -> usize {
    core::fill_end(frame_num, new_frame_num)
}

/// `bool obs_source_audio_place_mix_on(uint32_t audio_mixers, size_t mix)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_place_mix_on(audio_mixers: u32, mix: usize) -> bool {
    core::mix_on(audio_mixers, mix)
}
