//! Safe core of `libobs/audio-monitoring/monitor-speaker-layout.c.h`: the
//! speaker layout the WASAPI monitor resamples to for the device's mix
//! format (`audio_monitor_init_wasapi()`, `win32/wasapi-output.c`).
//!
//! The device describes its format by a channel mask (`dwChannelMask`, a
//! `KSAUDIO_SPEAKER_*` value) and a channel count (`nChannels`). The five
//! masks the monitor lists map to their layouts; any other mask takes the
//! layout numbered by the channel count. The monitor copies `nChannels`
//! floats per frame, so a layout with another channel count is never
//! returned: [`UNKNOWN`] instead.
//!
//! The layout is the raw `enum speaker_layout` value, not
//! [`SpeakerLayout`]: the default turns 7 channels into the value 7, which
//! no layout has, before the channel count check refuses it. Its channel
//! count is `obs-media-io`'s port of `get_audio_channels()`, 0 for a value
//! outside the enum, as the C's `default:`. `wasapi-output.c` keeps the
//! device query, the `#if` check of the masks against `ksmedia.h` and the
//! resampler. Nothing here keeps state, panics or overflows.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use std::ffi::c_int;

use obs_media_io::audio_io::SpeakerLayout;

/// `AUDIO_MONITOR_SPEAKER_MASK_2POINT1` (`KSAUDIO_SPEAKER_2POINT1`): FL FR
/// LFE.
pub const MASK_2POINT1: u32 = 0x0000_000B;
/// `AUDIO_MONITOR_SPEAKER_MASK_SURROUND` (`KSAUDIO_SPEAKER_SURROUND`): FL FR
/// FC BC.
pub const MASK_SURROUND: u32 = 0x0000_0107;
/// `AUDIO_MONITOR_SPEAKER_MASK_4POINT1` (`KSAUDIO_SPEAKER_4POINT1`): FL FR
/// FC LFE BC.
pub const MASK_4POINT1: u32 = 0x0000_010F;
/// `AUDIO_MONITOR_SPEAKER_MASK_5POINT1` (`KSAUDIO_SPEAKER_5POINT1`): FL FR
/// FC LFE BL BR.
pub const MASK_5POINT1: u32 = 0x0000_003F;
/// `AUDIO_MONITOR_SPEAKER_MASK_7POINT1` (`KSAUDIO_SPEAKER_7POINT1`): FL FR
/// FC LFE BL BR FLC FRC.
pub const MASK_7POINT1: u32 = 0x0000_00FF;

/// `SPEAKERS_UNKNOWN`: no layout.
pub const UNKNOWN: c_int = 0;

/// The `enum speaker_layout` value to resample to for a device with this
/// channel mask and channel count, or [`UNKNOWN`].
#[must_use]
#[expect(clippy::todo, reason = "RED: the port is not written yet")]
pub fn speaker_layout(mask: u32, channels: u32) -> c_int {
    let _ = (mask, channels, SpeakerLayout::Unknown);
    todo!()
}
