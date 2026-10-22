//! `oracle_*` build of
//! `libobs/audio-monitoring/pulse/pulseaudio-monitor-format.c.h`
//! (oracle/pulse_format.c).

use core::ffi::{c_char, c_int, c_uint};

/// `pa_channel_map`, as the oracle sees it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleChannelMap {
    pub channels: u8,
    pub map: [c_int; 32],
}

/// `struct pulseaudio_monitor_drop`, as the oracle sees it.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OracleDrop {
    pub unwritten: usize,
    pub rewind: usize,
}

/// Index of each value in `oracle_pulse_format_value`, in the C's order.
pub const VALUE_NAMES: [&str; 41] = [
    "PA_SAMPLE_U8",
    "PA_SAMPLE_ALAW",
    "PA_SAMPLE_ULAW",
    "PA_SAMPLE_S16LE",
    "PA_SAMPLE_S16BE",
    "PA_SAMPLE_FLOAT32LE",
    "PA_SAMPLE_FLOAT32BE",
    "PA_SAMPLE_S32LE",
    "PA_SAMPLE_S32BE",
    "PA_SAMPLE_S24LE",
    "PA_SAMPLE_S24BE",
    "PA_SAMPLE_S24_32LE",
    "PA_SAMPLE_S24_32BE",
    "PA_SAMPLE_MAX",
    "PA_SAMPLE_INVALID",
    "PA_CHANNELS_MAX",
    "PA_CHANNEL_POSITION_INVALID",
    "PA_CHANNEL_POSITION_MONO",
    "PA_CHANNEL_POSITION_FRONT_LEFT",
    "PA_CHANNEL_POSITION_FRONT_RIGHT",
    "PA_CHANNEL_POSITION_FRONT_CENTER",
    "PA_CHANNEL_POSITION_REAR_CENTER",
    "PA_CHANNEL_POSITION_REAR_LEFT",
    "PA_CHANNEL_POSITION_REAR_RIGHT",
    "PA_CHANNEL_POSITION_LFE",
    "PA_CHANNEL_POSITION_SIDE_LEFT",
    "PA_CHANNEL_POSITION_SIDE_RIGHT",
    "PA_CHANNEL_POSITION_MAX",
    "AUDIO_FORMAT_UNKNOWN",
    "AUDIO_FORMAT_U8BIT",
    "AUDIO_FORMAT_16BIT",
    "AUDIO_FORMAT_32BIT",
    "AUDIO_FORMAT_FLOAT",
    "SPEAKERS_UNKNOWN",
    "SPEAKERS_MONO",
    "SPEAKERS_STEREO",
    "SPEAKERS_2POINT1",
    "SPEAKERS_4POINT0",
    "SPEAKERS_4POINT1",
    "SPEAKERS_5POINT1",
    "SPEAKERS_7POINT1",
];

unsafe extern "C" {
    /// `pulseaudio_channels_to_obs_speakers`, the count converted to
    /// `uint_fast32_t` as a C caller's would be.
    pub fn oracle_pulse_channels_to_speakers(channels: u64) -> c_uint;
    pub fn oracle_pulseaudio_to_obs_audio_format(format: c_int) -> c_uint;
    pub fn oracle_pulseaudio_channel_map(layout: c_uint) -> OracleChannelMap;
    pub fn oracle_pulseaudio_monitor_sink_format(format: c_int) -> c_int;
    pub fn oracle_pulseaudio_monitor_sink_channels(channels: u8) -> u8;
    /// `pulseaudio_monitor_process_volume`, the channel count converted to
    /// `uint_fast8_t` as a C caller's would be.
    pub fn oracle_pulse_process_volume(
        format: c_int,
        channels: u64,
        vol: f32,
        resample_data: *const *mut u8,
        resample_frames: u32,
    );
    pub fn oracle_pulseaudio_monitor_queued_bytes(
        write_index: i64,
        read_index: i64,
        played_usec: u64,
        bytes_per_sec: u64,
        unwritten: usize,
    ) -> u64;
    pub fn oracle_pulseaudio_monitor_drop_split(
        excess_usec: u64,
        bytes_per_sec: u64,
        bytes_per_frame: usize,
        unwritten: usize,
    ) -> OracleDrop;
    pub fn oracle_pulseaudio_monitor_devices_match(
        id1: *const c_char,
        id2: *const c_char,
        default_sink: *const c_char,
    ) -> bool;
    /// The value named `VALUE_NAMES[which]`.
    pub fn oracle_pulse_format_value(which: usize) -> i64;
    /// 0 size, 1 align, 2 offset of `channels`, 3 offset of `map` of
    /// `pa_channel_map`; 4-7 the same for `struct pulseaudio_monitor_drop`
    /// (`unwritten`, `rewind`); 8 `sizeof(uint_fast32_t)`, 9
    /// `sizeof(uint_fast8_t)`.
    pub fn oracle_pulse_format_layout(which: usize) -> usize;
    /// 1 when the oracle was built against libpulse's own headers.
    pub fn oracle_pulse_format_headers_real() -> c_int;
}
