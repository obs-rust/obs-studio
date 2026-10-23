//! C ABI for `libobs/obs-source-audio-input.c.h`, the audio input steps of
//! `process_audio()` and `reset_resampler()`, declared in
//! `libobs/obs-source-audio-input.h` without `EXPORT`.
//!
//! `process_audio()` and `reset_resampler()` in `obs-source.c.h` (still C)
//! read the source's fields, pass them in and store what comes back. So
//! `struct obs_source` is never mirrored: the decisions take plain values
//! and `struct obs_source_audio_input_format` by value (mirrored), and the
//! two loops take the source's `float **` planes and counts. All seven stay
//! hidden (`rust-hidden.txt`).
//!
//! The loops read exactly what the C reads: nothing when there are no
//! frames, nothing for an unknown law (not even the plane pointers), planes
//! 0 and 1 for a law, and planes 0 to `channels - 1` for the downmix (plane
//! 0 alone for 0 channels). The downmix holds at most two planes at a time,
//! so it allocates nothing for any channel count. The planes must not
//! overlap; libobs's are separate allocations (`copy_audio_data()`).
//!
//! Intentional difference: a null `data`, or a null plane the C would
//! read, does nothing (the C dereferences it). libobs passes
//! `source->audio_data.data`, whose planes `copy_audio_data()` allocated.

use core::ffi::c_int;

use crate::audio_input::{self as input, Format, Law};

/// `struct obs_source_audio_input_format`, as the C lays it out.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_source_audio_input_format {
    /// `enum audio_format`.
    pub format: c_int,
    pub samples_per_sec: u32,
    /// `enum speaker_layout`.
    pub speakers: c_int,
}

impl From<obs_source_audio_input_format> for Format {
    fn from(format: obs_source_audio_input_format) -> Self {
        Self {
            format: format.format,
            samples_per_sec: format.samples_per_sec,
            speakers: format.speakers,
        }
    }
}

/// `bool obs_source_audio_input_discontinuous(bool has_resampler, uint64_t
/// next_resample_in_ts, uint64_t timestamp)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_input_discontinuous(
    has_resampler: bool,
    next_resample_in_ts: u64,
    timestamp: u64,
) -> bool {
    input::discontinuous(has_resampler, next_resample_in_ts, timestamp)
}

/// `bool obs_source_audio_input_reset_needed(struct
/// obs_source_audio_input_format current, struct
/// obs_source_audio_input_format packet, bool last_compensation, bool
/// compensation, bool discontinuous)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_input_reset_needed(
    current: obs_source_audio_input_format,
    packet: obs_source_audio_input_format,
    last_compensation: bool,
    compensation: bool,
    discontinuous: bool,
) -> bool {
    input::reset_needed(
        current.into(),
        packet.into(),
        last_compensation,
        compensation,
        discontinuous,
    )
}

/// `bool obs_source_audio_input_resampler_needed(struct
/// obs_source_audio_input_format source, struct
/// obs_source_audio_input_format output, bool compensation)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_input_resampler_needed(
    source: obs_source_audio_input_format,
    output: obs_source_audio_input_format,
    compensation: bool,
) -> bool {
    input::resampler_needed(source.into(), output.into(), compensation)
}

/// `bool obs_source_audio_input_balance_needed(bool mono_output, int
/// speakers, float balance)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_input_balance_needed(
    mono_output: bool,
    speakers: c_int,
    balance: f32,
) -> bool {
    input::balance_needed(mono_output, speakers, balance)
}

/// `bool obs_source_audio_input_mono_needed(bool mono_output, uint32_t
/// flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_audio_input_mono_needed(mono_output: bool, flags: u32) -> bool {
    input::mono_needed(mono_output, flags)
}

/// Plane `index` of `data`, or null.
///
/// # Safety
///
/// `data` holds at least `index + 1` plane pointers.
unsafe fn plane(data: *mut *mut f32, index: usize) -> *mut f32 {
    // SAFETY: the caller's contract.
    unsafe { *data.add(index) }
}

/// `void obs_source_audio_input_balance(float **data, uint32_t frames,
/// float balance, int law)`.
///
/// # Safety
///
/// For a known law and `frames > 0`: `data` holds two plane pointers, each
/// to `frames` writable floats, not overlapping.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_audio_input_balance(
    data: *mut *mut f32,
    frames: u32,
    balance: f32,
    law: c_int,
) {
    let Some(law) = Law::from_c(law) else {
        return;
    };
    if frames == 0 || data.is_null() {
        return;
    }
    // SAFETY: a known law reads planes 0 and 1.
    let (left, right) = unsafe { (plane(data, 0), plane(data, 1)) };
    if left.is_null() || right.is_null() {
        return;
    }
    let frames = usize::try_from(frames).unwrap_or(usize::MAX);
    // SAFETY: each plane holds `frames` floats and they do not overlap.
    let (left, right) = unsafe {
        (
            core::slice::from_raw_parts_mut(left, frames),
            core::slice::from_raw_parts_mut(right, frames),
        )
    };
    input::balance(left, right, balance, law);
}

/// `void obs_source_audio_input_downmix(float **data, size_t channels,
/// uint32_t frames)`.
///
/// # Safety
///
/// For `frames > 0`: `data` holds `channels` plane pointers (one for 0
/// channels), each to `frames` writable floats, not overlapping.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_audio_input_downmix(
    data: *mut *mut f32,
    channels: usize,
    frames: u32,
) {
    if frames == 0 || data.is_null() {
        return;
    }
    let frames = usize::try_from(frames).unwrap_or(usize::MAX);
    let planes = channels.max(1);
    // Every plane the C reads, checked before anything is written.
    for index in 0..planes {
        // SAFETY: `data` holds `planes` pointers.
        if unsafe { plane(data, index) }.is_null() {
            return;
        }
    }
    // SAFETY: plane 0 holds `frames` floats.
    let first = unsafe { core::slice::from_raw_parts_mut(plane(data, 0), frames) };
    // The C's three loops in its order: each other plane added into the
    // first, the first scaled, then copied back to each other plane.
    for index in 1..channels {
        // SAFETY: plane `index` holds `frames` floats and does not overlap
        // the first.
        let other = unsafe { core::slice::from_raw_parts(plane(data, index), frames) };
        input::add_plane(first, other);
    }
    input::scale_plane(first, channels);
    for index in 1..channels {
        // SAFETY: as above.
        let other = unsafe { core::slice::from_raw_parts_mut(plane(data, index), frames) };
        input::copy_plane(first, other);
    }
}
