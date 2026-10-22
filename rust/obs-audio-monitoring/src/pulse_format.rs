//! Safe core of
//! `libobs/audio-monitoring/pulse/pulseaudio-monitor-format.c.h`: the
//! PulseAudio monitoring stream's sample format, channel map and volume,
//! the arithmetic of catching up after a stall, and the feedback-loop
//! device match.
//!
//! The monitor plays every monitored source on the chosen sink. It plays in
//! the sink's spec where OBS can produce it (U8, S16LE, S32LE, FLOAT32LE; 1-6
//! or 8 channels), and as FLOAT32LE or stereo where it cannot. The resampler
//! hands it interleaved samples, and it applies the source's volume to them
//! itself, in the sink format.
//!
//! No libpulse here: the `pa_sample_format_t` and `pa_channel_position_t`
//! values below are libpulse ABI, and the `enum audio_format` and
//! `enum speaker_layout` values libobs ABI. The parity test compares them
//! against the C headers through the oracle.
//!
//! Sample buffers are byte slices, so the volume code needs no alignment;
//! samples are read and written in native byte order, as the C's pointer
//! casts do. Nothing here can panic, index out of bounds or overflow.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `PA_SAMPLE_U8`.
pub const SAMPLE_U8: i32 = 0;
/// `PA_SAMPLE_ALAW`.
pub const SAMPLE_ALAW: i32 = 1;
/// `PA_SAMPLE_ULAW`.
pub const SAMPLE_ULAW: i32 = 2;
/// `PA_SAMPLE_S16LE`.
pub const SAMPLE_S16LE: i32 = 3;
/// `PA_SAMPLE_S16BE`.
pub const SAMPLE_S16BE: i32 = 4;
/// `PA_SAMPLE_FLOAT32LE`.
pub const SAMPLE_FLOAT32LE: i32 = 5;
/// `PA_SAMPLE_FLOAT32BE`.
pub const SAMPLE_FLOAT32BE: i32 = 6;
/// `PA_SAMPLE_S32LE`.
pub const SAMPLE_S32LE: i32 = 7;
/// `PA_SAMPLE_S32BE`.
pub const SAMPLE_S32BE: i32 = 8;
/// `PA_SAMPLE_S24LE`.
pub const SAMPLE_S24LE: i32 = 9;
/// `PA_SAMPLE_S24BE`.
pub const SAMPLE_S24BE: i32 = 10;
/// `PA_SAMPLE_S24_32LE`.
pub const SAMPLE_S24_32LE: i32 = 11;
/// `PA_SAMPLE_S24_32BE`.
pub const SAMPLE_S24_32BE: i32 = 12;
/// `PA_SAMPLE_MAX`: one past the last format.
pub const SAMPLE_MAX: i32 = 13;
/// `PA_SAMPLE_INVALID`.
pub const SAMPLE_INVALID: i32 = -1;

/// `PA_CHANNELS_MAX`: the length of `pa_channel_map.map`.
pub const CHANNELS_MAX: usize = 32;

/// `PA_CHANNEL_POSITION_INVALID`.
pub const POSITION_INVALID: i32 = -1;
/// `PA_CHANNEL_POSITION_MONO`.
pub const POSITION_MONO: i32 = 0;
/// `PA_CHANNEL_POSITION_FRONT_LEFT`.
pub const POSITION_FRONT_LEFT: i32 = 1;
/// `PA_CHANNEL_POSITION_FRONT_RIGHT`.
pub const POSITION_FRONT_RIGHT: i32 = 2;
/// `PA_CHANNEL_POSITION_FRONT_CENTER`.
pub const POSITION_FRONT_CENTER: i32 = 3;
/// `PA_CHANNEL_POSITION_REAR_CENTER`.
pub const POSITION_REAR_CENTER: i32 = 4;
/// `PA_CHANNEL_POSITION_REAR_LEFT`.
pub const POSITION_REAR_LEFT: i32 = 5;
/// `PA_CHANNEL_POSITION_REAR_RIGHT`.
pub const POSITION_REAR_RIGHT: i32 = 6;
/// `PA_CHANNEL_POSITION_LFE`.
pub const POSITION_LFE: i32 = 7;
/// `PA_CHANNEL_POSITION_SIDE_LEFT`.
pub const POSITION_SIDE_LEFT: i32 = 10;
/// `PA_CHANNEL_POSITION_SIDE_RIGHT`.
pub const POSITION_SIDE_RIGHT: i32 = 11;
/// `PA_CHANNEL_POSITION_MAX`: one past the last position.
pub const POSITION_MAX: i32 = 51;

/// `AUDIO_FORMAT_UNKNOWN`.
pub const AUDIO_FORMAT_UNKNOWN: u32 = 0;
/// `AUDIO_FORMAT_U8BIT`.
pub const AUDIO_FORMAT_U8BIT: u32 = 1;
/// `AUDIO_FORMAT_16BIT`.
pub const AUDIO_FORMAT_16BIT: u32 = 2;
/// `AUDIO_FORMAT_32BIT`.
pub const AUDIO_FORMAT_32BIT: u32 = 3;
/// `AUDIO_FORMAT_FLOAT`.
pub const AUDIO_FORMAT_FLOAT: u32 = 4;

/// `SPEAKERS_UNKNOWN`.
pub const SPEAKERS_UNKNOWN: u32 = 0;
/// `SPEAKERS_MONO`.
pub const SPEAKERS_MONO: u32 = 1;
/// `SPEAKERS_STEREO`.
pub const SPEAKERS_STEREO: u32 = 2;
/// `SPEAKERS_2POINT1`.
pub const SPEAKERS_2POINT1: u32 = 3;
/// `SPEAKERS_4POINT0`.
pub const SPEAKERS_4POINT0: u32 = 4;
/// `SPEAKERS_4POINT1`.
pub const SPEAKERS_4POINT1: u32 = 5;
/// `SPEAKERS_5POINT1`.
pub const SPEAKERS_5POINT1: u32 = 6;
/// `SPEAKERS_7POINT1`.
pub const SPEAKERS_7POINT1: u32 = 8;

/// `pulseaudio_channels_to_obs_speakers`: the speaker layout of a sink
/// with this many channels; [`SPEAKERS_UNKNOWN`] for 0, 7 and more than 8.
#[must_use]
pub fn channels_to_speakers(channels: u64) -> u32 {
    match channels {
        1 => SPEAKERS_MONO,
        2 => SPEAKERS_STEREO,
        3 => SPEAKERS_2POINT1,
        4 => SPEAKERS_4POINT0,
        5 => SPEAKERS_4POINT1,
        6 => SPEAKERS_5POINT1,
        8 => SPEAKERS_7POINT1,
        _ => SPEAKERS_UNKNOWN,
    }
}

/// `pulseaudio_to_obs_audio_format`: the OBS sample format of a sink
/// format; [`AUDIO_FORMAT_UNKNOWN`] for anything but U8, S16LE, S32LE and
/// FLOAT32LE.
#[must_use]
pub fn to_obs_audio_format(format: i32) -> u32 {
    match format {
        SAMPLE_U8 => AUDIO_FORMAT_U8BIT,
        SAMPLE_S16LE => AUDIO_FORMAT_16BIT,
        SAMPLE_S32LE => AUDIO_FORMAT_32BIT,
        SAMPLE_FLOAT32LE => AUDIO_FORMAT_FLOAT,
        _ => AUDIO_FORMAT_UNKNOWN,
    }
}

/// The part of `pa_channel_map` the C fills: the channel count and the
/// first eight positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelMap {
    /// `channels`; 0 for a layout the monitor does not play.
    pub channels: u8,
    /// `map[0..8]`, filled past `channels` too (the default order).
    pub map: [i32; 8],
}

/// `pulseaudio_channel_map`: the channel map for a speaker layout; 0
/// channels for [`SPEAKERS_UNKNOWN`] and layouts the monitor does not play.
#[must_use]
pub fn channel_map(layout: u32) -> ChannelMap {
    let mut map = [
        POSITION_FRONT_LEFT,
        POSITION_FRONT_RIGHT,
        POSITION_FRONT_CENTER,
        POSITION_LFE,
        POSITION_REAR_LEFT,
        POSITION_REAR_RIGHT,
        POSITION_SIDE_LEFT,
        POSITION_SIDE_RIGHT,
    ];
    let channels = match layout {
        SPEAKERS_MONO => {
            map[0] = POSITION_MONO;
            1
        }
        SPEAKERS_STEREO => 2,
        SPEAKERS_2POINT1 => {
            map[2] = POSITION_LFE;
            3
        }
        SPEAKERS_4POINT0 => {
            map[3] = POSITION_REAR_CENTER;
            4
        }
        SPEAKERS_4POINT1 => {
            map[4] = POSITION_REAR_CENTER;
            5
        }
        SPEAKERS_5POINT1 => 6,
        SPEAKERS_7POINT1 => 8,
        _ => 0,
    };
    ChannelMap { channels, map }
}

/// `pulseaudio_monitor_sink_format`: a format OBS cannot produce plays as
/// FLOAT32LE; the server converts.
#[must_use]
pub fn sink_format(format: i32) -> i32 {
    if to_obs_audio_format(format) == AUDIO_FORMAT_UNKNOWN {
        SAMPLE_FLOAT32LE
    } else {
        format
    }
}

/// `pulseaudio_monitor_sink_channels`: a channel count with no speaker
/// layout (0, 7, more than 8) plays as stereo.
#[must_use]
pub fn sink_channels(channels: u8) -> u8 {
    if channels_to_speakers(u64::from(channels)) == SPEAKERS_UNKNOWN {
        2
    } else {
        channels
    }
}

/// The size of one sample of a format [`process_volume`] changes; `None`
/// for the formats it leaves alone.
#[must_use]
pub fn sample_size(format: i32) -> Option<usize> {
    match format {
        SAMPLE_U8 => Some(1),
        SAMPLE_S16LE => Some(2),
        SAMPLE_S32LE | SAMPLE_FLOAT32LE => Some(4),
        _ => None,
    }
}

// The volume conversions. Each computes the C's float expression with the
// same f32 operations (Rust never fuses them, and the oracle is built with
// -ffp-contract=off), then saturates the integer formats to their range
// before converting back. The comparisons are the C's: the clamp bound
// itself already saturates (`>=` the top), and inside the open range the
// result is the product truncated toward zero, which `as` gives exactly.
// `as` would saturate out of range too, but the rule is written out so it
// is the C's, not Rust's. A NaN product fails every comparison: C converts
// it with undefined behaviour, Rust's `as` gives 0.

/// `process_byte` for one sample: `((int)x - 128) * vol + 128`, the offset
/// added in float, saturated to 0..=255, then truncated.
#[must_use]
pub fn scale_u8(x: u8, vol: f32) -> u8 {
    let centered = i16::from(x).wrapping_sub(128);
    let v = f32::from(centered) * vol + 128.0;
    if v >= 255.0 {
        u8::MAX
    } else if v <= 0.0 {
        0
    } else {
        // In (0, 255): truncation, as C's conversion.
        v as u8
    }
}

/// `process_s16` for one sample: `x * vol` in float, saturated to the
/// `i16` range, then truncated toward zero.
#[must_use]
pub fn scale_s16(x: i16, vol: f32) -> i16 {
    let v = f32::from(x) * vol;
    if v >= f32::from(i16::MAX) {
        i16::MAX
    } else if v <= f32::from(i16::MIN) {
        i16::MIN
    } else {
        // In (-32768, 32767): truncation toward zero, as C's conversion.
        v as i16
    }
}

/// 2^31: `INT32_MAX` is not an `f32`, this is the first `f32` past it.
const S32_TOP: f32 = 2_147_483_648.0;

/// `process_s32` for one sample: `x` converted to float first (it keeps 24
/// bits), times `vol`, saturated to the `i32` range, then truncated toward
/// zero.
#[must_use]
pub fn scale_s32(x: i32, vol: f32) -> i32 {
    let v = x as f32 * vol;
    if v >= S32_TOP {
        i32::MAX
    } else if v <= -S32_TOP {
        i32::MIN
    } else {
        // In (-2^31, 2^31): truncation toward zero, as C's conversion.
        v as i32
    }
}

/// `process_float` for one sample: `x * vol`, not clamped (the server
/// clips).
#[must_use]
pub fn scale_f32(x: f32, vol: f32) -> f32 {
    x * vol
}

/// Applies `scale` to each `N`-byte sample of `bytes`, in native byte
/// order. A trailing partial sample is left alone.
fn scale_each<const N: usize>(bytes: &mut [u8], scale: impl Fn([u8; N]) -> [u8; N]) {
    let (samples, _) = bytes.as_chunks_mut::<N>();
    for sample in samples {
        *sample = scale(*sample);
    }
}

/// `pulseaudio_monitor_process_volume`: applies a source's volume to the
/// first `frames * channels` interleaved samples of `format` in `data`.
/// Formats other than U8, S16LE, S32LE and FLOAT32LE are left unchanged.
///
/// Never touches more than `data` holds: if it is shorter than
/// `frames * channels` samples (the C would write past it), only its whole
/// samples are processed.
pub fn process_volume(format: i32, channels: usize, frames: usize, vol: f32, data: &mut [u8]) {
    let Some(size) = sample_size(format) else {
        return;
    };
    let want = frames
        .checked_mul(channels)
        .and_then(|samples| samples.checked_mul(size))
        .unwrap_or(usize::MAX);
    let len = want.min(data.len());
    let Some(bytes) = data.get_mut(..len) else {
        return;
    };
    match format {
        SAMPLE_U8 => scale_each::<1>(bytes, |[x]| [scale_u8(x, vol)]),
        SAMPLE_S16LE => scale_each::<2>(bytes, |x| {
            scale_s16(i16::from_ne_bytes(x), vol).to_ne_bytes()
        }),
        SAMPLE_S32LE => scale_each::<4>(bytes, |x| {
            scale_s32(i32::from_ne_bytes(x), vol).to_ne_bytes()
        }),
        SAMPLE_FLOAT32LE => scale_each::<4>(bytes, |x| {
            scale_f32(f32::from_ne_bytes(x), vol).to_ne_bytes()
        }),
        _ => {}
    }
}

const USEC_PER_SEC: u64 = 1_000_000;

/// `util_mul_div64(num, mul, 1000000)` as the C computes it on this host
/// (see `obs_util::util_uint64::mul_div64`), without its panics: the
/// divisor is a non-zero constant, and where the C uses `_udiv128` (MSVC
/// x64, which has no PulseAudio monitoring) a quotient past 64 bits, which
/// faults in C, saturates.
fn mul_div_usec(num: u64, mul: u64) -> u64 {
    if obs_util::platform::MUL_DIV64_IS_UDIV128 {
        u128::from(num)
            .wrapping_mul(u128::from(mul))
            .checked_div(u128::from(USEC_PER_SEC))
            .and_then(|quotient| u64::try_from(quotient).ok())
            .unwrap_or(u64::MAX)
    } else {
        let rem = num.wrapping_rem(USEC_PER_SEC);
        num.wrapping_div(USEC_PER_SEC)
            .wrapping_mul(mul)
            .wrapping_add(rem.wrapping_mul(mul).wrapping_div(USEC_PER_SEC))
    }
}

/// `pulseaudio_monitor_queued_bytes`: how much audio is queued for the
/// stream, in bytes. What the server holds (`write_index - read_index`,
/// less what it has played in `played_usec` since that timing update; 0 if
/// that is negative) plus `unwritten`, what is not written to it yet.
///
/// The `int64_t` arithmetic wraps (in C it is undefined on overflow, which
/// real indices never reach), and the sum wraps as C's `uint64_t`.
#[must_use]
pub fn queued_bytes(
    write_index: i64,
    read_index: i64,
    played_usec: u64,
    bytes_per_sec: u64,
    unwritten: usize,
) -> u64 {
    let played = mul_div_usec(played_usec, bytes_per_sec) as i64;
    let held = write_index.wrapping_sub(read_index).wrapping_sub(played);
    let held = u64::try_from(held).unwrap_or(0);
    held.wrapping_add(unwritten as u64)
}

/// `struct pulseaudio_monitor_drop`: where queued audio is dropped.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DropSplit {
    /// Bytes dropped from what is not written yet.
    pub unwritten: usize,
    /// Bytes dropped by writing the next data that much earlier in the
    /// server's buffer.
    pub rewind: usize,
}

/// `pulseaudio_monitor_drop_split`: `excess_usec` of queued audio in bytes,
/// rounded down to whole frames, dropped first from what is not written yet
/// (at most `unwritten`), the rest by rewinding.
///
/// The byte count is converted to `size_t` as in C (truncated where it is
/// 32 bits). A `bytes_per_frame` of 0, a division by zero in C (its caller
/// never passes it), drops nothing.
#[must_use]
pub fn drop_split(
    excess_usec: u64,
    bytes_per_sec: u64,
    bytes_per_frame: usize,
    unwritten: usize,
) -> DropSplit {
    let bytes = mul_div_usec(excess_usec, bytes_per_sec) as usize;
    let Some(partial) = bytes.checked_rem(bytes_per_frame) else {
        return DropSplit::default();
    };
    let bytes = bytes.wrapping_sub(partial);
    let from_unwritten = bytes.min(unwritten);
    DropSplit {
        unwritten: from_unwritten,
        rewind: bytes.wrapping_sub(from_unwritten),
    }
}

const DEFAULT_ID: &[u8] = b"default";
const MONITOR_SUFFIX: &[u8] = b".monitor";

/// `pulseaudio_monitor_devices_match`: whether a sound source (`id1`) is
/// the `.monitor` device of the selected monitoring output (`id2`).
/// `"default"` on either side stands for `default_sink`, the default sink's
/// name (`""` if there is none).
///
/// The ids are the C strings without their NUL; `None` is a null pointer,
/// which never matches. C builds both names and compares them; this
/// compares the same bytes without allocating. C dereferences a null
/// `default_sink` when an id is `"default"`; here that is no match.
#[must_use]
pub fn devices_match(id1: Option<&[u8]>, id2: Option<&[u8]>, default_sink: Option<&[u8]>) -> bool {
    let (Some(id1), Some(id2)) = (id1, id2) else {
        return false;
    };

    // The source: the default sink's monitor, or the id as it is.
    let (source, source_suffix): (&[u8], &[u8]) = if id1 == DEFAULT_ID {
        let Some(default_sink) = default_sink else {
            return false;
        };
        (default_sink, MONITOR_SUFFIX)
    } else {
        (id1, b"")
    };

    // The output's monitor: the default sink's, or the id's.
    let output = if id2 == DEFAULT_ID {
        let Some(default_sink) = default_sink else {
            return false;
        };
        default_sink
    } else {
        id2
    };

    source
        .iter()
        .chain(source_suffix)
        .eq(output.iter().chain(MONITOR_SUFFIX))
}
