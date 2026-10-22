//! C ABI for `libobs/audio-monitoring/pulse/pulseaudio-monitor-format.c.h`.
//!
//! `pulseaudio-output.c` and `pulseaudio-wrapper.c` call these inside
//! libobs. The C file has no `EXPORT`, so the symbols stay hidden
//! (`rust-hidden.txt`). Only Linux and FreeBSD with `ENABLE_PULSEAUDIO` call
//! them; elsewhere the shims are built but unused, and hidden all the same.
//!
//! The enums cross as their C type: `pa_sample_format_t` and
//! `pa_channel_position_t` have negative values (`*_INVALID`), so they are
//! `int`; `enum speaker_layout` and `enum audio_format` have none, so GCC
//! and Clang give them `unsigned int`. `pa_channel_map` (libpulse ABI) is
//! mirrored here; the layout test compares it with libpulse's header.

use std::ffi::{CStr, c_char, c_int, c_uint};

use crate::pulse_format::{
    CHANNELS_MAX, DropSplit, POSITION_INVALID, channel_map, channels_to_speakers, devices_match,
    drop_split, process_volume, queued_bytes, sample_size, sink_channels, sink_format,
    to_obs_audio_format,
};

// `uint_fast32_t` and `uint_fast8_t` are as wide as the host's <stdint.h>
// makes them; the layout test compares both with the C. glibc on 64-bit
// (and Bionic) widens uint_fast32_t to 64 bits, FreeBSD and OpenBSD widen
// uint_fast8_t to 32.
#[cfg(any(
    all(target_os = "linux", target_env = "gnu", target_pointer_width = "64"),
    all(target_os = "android", target_pointer_width = "64")
))]
#[allow(non_camel_case_types)]
/// `uint_fast32_t` on this host.
pub type uint_fast32_t = u64;
#[cfg(not(any(
    all(target_os = "linux", target_env = "gnu", target_pointer_width = "64"),
    all(target_os = "android", target_pointer_width = "64")
)))]
#[allow(non_camel_case_types)]
/// `uint_fast32_t` on this host.
pub type uint_fast32_t = u32;

#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
#[allow(non_camel_case_types)]
/// `uint_fast8_t` on this host.
pub type uint_fast8_t = u32;
#[cfg(not(any(target_os = "freebsd", target_os = "openbsd")))]
#[allow(non_camel_case_types)]
/// `uint_fast8_t` on this host.
pub type uint_fast8_t = u8;

/// `pa_channel_map` in libpulse's `<pulse/channelmap.h>`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct pa_channel_map {
    pub channels: u8,
    pub map: [c_int; CHANNELS_MAX],
}

/// `struct pulseaudio_monitor_drop` in `pulseaudio-monitor-format.h`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct pulseaudio_monitor_drop {
    pub unwritten: usize,
    pub rewind: usize,
}

/// `id` as the safe core takes it: `None` for null.
///
/// # Safety
///
/// `id` is null or a NUL-terminated string that outlives the result.
unsafe fn c_id<'a>(id: *const c_char) -> Option<&'a [u8]> {
    if id.is_null() {
        None
    } else {
        // SAFETY: the caller passes a NUL-terminated string.
        Some(unsafe { CStr::from_ptr(id) }.to_bytes())
    }
}

/// Same contract as C `pulseaudio_channels_to_obs_speakers`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_channels_to_obs_speakers(channels: uint_fast32_t) -> c_uint {
    // A no-op where uint_fast32_t is already 64 bits.
    #[allow(clippy::useless_conversion)]
    let channels = u64::from(channels);
    channels_to_speakers(channels)
}

/// Same contract as C `pulseaudio_to_obs_audio_format`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_to_obs_audio_format(format: c_int) -> c_uint {
    to_obs_audio_format(format)
}

/// Same contract as C `pulseaudio_channel_map`. `map[8..]`, which C leaves
/// indeterminate, is `PA_CHANNEL_POSITION_INVALID`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_channel_map(layout: c_uint) -> pa_channel_map {
    let core = channel_map(layout);
    let mut map = [POSITION_INVALID; CHANNELS_MAX];
    for (slot, position) in map.iter_mut().zip(core.map) {
        *slot = position;
    }
    pa_channel_map {
        channels: core.channels,
        map,
    }
}

/// Same contract as C `pulseaudio_monitor_sink_format`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_sink_format(format: c_int) -> c_int {
    sink_format(format)
}

/// Same contract as C `pulseaudio_monitor_sink_channels`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_sink_channels(channels: u8) -> u8 {
    sink_channels(channels)
}

/// Same contract as C `pulseaudio_monitor_process_volume`: applies `vol` to
/// `resample_frames * channels` interleaved samples of `format` in
/// `resample_data[0]`. Formats other than U8, S16LE, S32LE and FLOAT32LE
/// are left unchanged and `resample_data` is not read, as in C. A null
/// `resample_data` or `resample_data[0]` with samples to process, which C
/// dereferences, does nothing.
///
/// # Safety
///
/// For the four formats, `resample_data` is null or points to a readable
/// pointer, which is null or points to `resample_frames * channels`
/// readable and writable samples of `format`, not otherwise borrowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pulseaudio_monitor_process_volume(
    format: c_int,
    channels: uint_fast8_t,
    vol: f32,
    resample_data: *const *mut u8,
    resample_frames: u32,
) {
    let Some(size) = sample_size(format) else {
        return;
    };
    // uint_fast8_t is u32 on FreeBSD and OpenBSD, where this can fail.
    #[allow(clippy::unnecessary_fallible_conversions)]
    let channels = usize::try_from(channels);
    let (Ok(channels), Ok(frames)) = (channels, usize::try_from(resample_frames)) else {
        return;
    };
    let Some(len) = frames
        .checked_mul(channels)
        .and_then(|samples| samples.checked_mul(size))
    else {
        return;
    };
    if len == 0 || resample_data.is_null() {
        return;
    }
    // SAFETY: the caller passes a readable pointer to the plane pointers.
    let data = unsafe { *resample_data };
    if data.is_null() {
        return;
    }
    // SAFETY: the caller passes `frames * channels` samples of `size` bytes
    // at `data`, exclusively, for the length of this call. u8 needs no
    // alignment.
    let samples = unsafe { std::slice::from_raw_parts_mut(data, len) };
    process_volume(format, channels, frames, vol, samples);
}

/// Same contract as C `pulseaudio_monitor_queued_bytes`.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_queued_bytes(
    write_index: i64,
    read_index: i64,
    played_usec: u64,
    bytes_per_sec: u64,
    unwritten: usize,
) -> u64 {
    queued_bytes(
        write_index,
        read_index,
        played_usec,
        bytes_per_sec,
        unwritten,
    )
}

/// Same contract as C `pulseaudio_monitor_drop_split`. A zero
/// `bytes_per_frame` (a division by zero in C) drops nothing.
#[unsafe(no_mangle)]
pub extern "C" fn pulseaudio_monitor_drop_split(
    excess_usec: u64,
    bytes_per_sec: u64,
    bytes_per_frame: usize,
    unwritten: usize,
) -> pulseaudio_monitor_drop {
    let DropSplit { unwritten, rewind } =
        drop_split(excess_usec, bytes_per_sec, bytes_per_frame, unwritten);
    pulseaudio_monitor_drop { unwritten, rewind }
}

/// Same contract as C `pulseaudio_monitor_devices_match`. A null
/// `default_sink` when an id is `"default"` (C dereferences it) is no
/// match.
///
/// # Safety
///
/// Each argument is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pulseaudio_monitor_devices_match(
    id1: *const c_char,
    id2: *const c_char,
    default_sink: *const c_char,
) -> bool {
    // SAFETY: the caller passes null or NUL-terminated strings, which live
    // for the call.
    unsafe { devices_match(c_id(id1), c_id(id2), c_id(default_sink)) }
}
