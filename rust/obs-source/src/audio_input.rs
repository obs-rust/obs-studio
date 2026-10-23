//! Safe core of `libobs/obs-source-audio-input.c.h`: the audio input steps
//! of `process_audio()` and `reset_resampler()`.
//!
//! Every packet a source sends goes through `process_audio()`, which
//! converts it to the output's format and then applies the source's
//! balance and Force Mono. These functions make its decisions and run its
//! two sample loops:
//! - [`discontinuous`]: a packet [`SMOOTHING_THRESHOLD`] or more from where
//!   the previous one ended starts a new stream, so the resampler drops the
//!   samples it holds back (#546);
//! - [`reset_needed`]: the resampler is reset when the packet's [`Format`]
//!   differs from the source's, when the clock drift compensation flag
//!   changed, or on a discontinuity;
//! - [`resampler_needed`]: a resampler is created unless the source has the
//!   output's format and the compensation is off (the clock drift fix: the
//!   compensation needs one even when the formats match);
//! - [`balance_needed`] and [`mono_needed`]: the balance gate and the Force
//!   Mono gate;
//! - [`balance`]: the sine, square and linear [`Law`]s over the left and
//!   right planes;
//! - [`downmix`]: the planes' mean into every plane, from [`add_plane`],
//!   [`scale_plane`] and [`copy_plane`].
//!
//! `obs-source.c.h` keeps everything else: the field reads and stores,
//! `next_resample_in_ts` (it computes it and passes it in), the resampler,
//! `copy_audio_data()` and its allocation, the clock drift filter and
//! `audio_resampler_set_compensation_error()`. It reads the compensation
//! flag (an atomic) and passes it in as a `bool`. Nothing here keeps state,
//! takes a lock, allocates or calls libobs.
//!
//! The float arithmetic is the C's, operation for operation: the sine law's
//! gains go through `f64` as the C's casts do, the sum is plane by plane
//! into the first, and the mean is the sum times the `f32` reciprocal of
//! the channel count.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `TS_SMOOTHING_THRESHOLD` (`obs-source.c.h`): a packet this far or more
/// from where the previous one ended is a new stream. In ns.
pub const SMOOTHING_THRESHOLD: u64 = 70_000_000;

/// `SPEAKERS_STEREO` (`enum speaker_layout`).
pub const SPEAKERS_STEREO: i32 = 2;

/// `OBS_SOURCE_FLAG_FORCE_MONO` (`obs.h`).
pub const FORCE_MONO: u32 = 1 << 1;

/// `OBS_BALANCE_TYPE_SINE_LAW` (`enum obs_balance_type`).
pub const SINE_LAW: i32 = 0;

/// `OBS_BALANCE_TYPE_SQUARE_LAW`.
pub const SQUARE_LAW: i32 = 1;

/// `OBS_BALANCE_TYPE_LINEAR`.
pub const LINEAR: i32 = 2;

/// `struct obs_source_audio_input_format`: `struct resample_info`'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Format {
    /// `enum audio_format`.
    pub format: i32,
    pub samples_per_sec: u32,
    /// `enum speaker_layout`.
    pub speakers: i32,
}

/// A balance law (`enum obs_balance_type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Law {
    /// [`SINE_LAW`]: left times `sin((1 - balance) * pi / 2)`, right times
    /// `sin(balance * pi / 2)`.
    Sine,
    /// [`SQUARE_LAW`]: left times `sqrt(1 - balance)`, right times
    /// `sqrt(balance)`.
    Square,
    /// [`LINEAR`]: left times `1 - balance`, right times `balance`.
    Linear,
}

impl Law {
    /// The law of an `enum obs_balance_type` value; `None` for any other
    /// value, which the C's `switch` skips.
    pub fn from_c(law: i32) -> Option<Self> {
        match law {
            SINE_LAW => Some(Self::Sine),
            SQUARE_LAW => Some(Self::Square),
            LINEAR => Some(Self::Linear),
            _ => None,
        }
    }
}

/// `obs_source_audio_input_discontinuous()`: whether the packet at
/// `timestamp` starts a new stream. Only with a resampler and
/// `next_resample_in_ts` set (not 0), and the packet
/// [`SMOOTHING_THRESHOLD`] or more from it, either way.
pub fn discontinuous(has_resampler: bool, next_resample_in_ts: u64, timestamp: u64) -> bool {
    // `uint64_diff()`: the distance either way, not modular.
    has_resampler
        && next_resample_in_ts != 0
        && timestamp.abs_diff(next_resample_in_ts) >= SMOOTHING_THRESHOLD
}

/// `obs_source_audio_input_reset_needed()`: whether `process_audio()`
/// resets the resampler, from the source's `current` format, the
/// `packet`'s, the compensation flag as last applied and as now, and the
/// discontinuity.
pub fn reset_needed(
    current: Format,
    packet: Format,
    last_compensation: bool,
    compensation: bool,
    discontinuous: bool,
) -> bool {
    current.samples_per_sec != packet.samples_per_sec
        || current.format != packet.format
        || current.speakers != packet.speakers
        || last_compensation != compensation
        || discontinuous
}

/// `obs_source_audio_input_resampler_needed()`: whether
/// `reset_resampler()` creates a resampler for the `source` format into the
/// `output`'s, with the compensation on or off.
pub fn resampler_needed(source: Format, output: Format, compensation: bool) -> bool {
    // Clock drift compensation needs a resampler even when the formats
    // match; it passes the audio through untouched until it is needed.
    !(source.samples_per_sec == output.samples_per_sec
        && source.format == output.format
        && source.speakers == output.speakers
        && !compensation)
}

/// `obs_source_audio_input_balance_needed()`: an output that is not mono,
/// a [`SPEAKERS_STEREO`] source and a balance above `0.51` or below `0.49`
/// (so not NaN).
pub fn balance_needed(mono_output: bool, speakers: i32, balance: f32) -> bool {
    // The C's two compares, so a NaN is neither above nor below.
    !mono_output && speakers == SPEAKERS_STEREO && (balance > 0.51 || balance < 0.49)
}

/// `obs_source_audio_input_mono_needed()`: an output that is not mono and
/// the [`FORCE_MONO`] flag.
pub fn mono_needed(mono_output: bool, flags: u32) -> bool {
    !mono_output && (flags & FORCE_MONO) != 0
}

/// The sine law's left and right gains for `balance`, with the C's casts:
/// `sinf((float)((double)(1.0f - balance) * (M_PI / 2.0)))` and
/// `sinf((float)((double)balance * (M_PI / 2.0)))`.
pub fn sine_gains(balance: f32) -> (f32, f32) {
    // `1.0f - balance` in f32, widened to f64 (exact), times the f64
    // `M_PI / 2.0`, rounded back to f32 (the C's `(float)`, round to
    // nearest), then `sinf`. `f32::sin` is the platform's `sinf`.
    #[allow(clippy::as_conversions)]
    let left = ((f64::from(1.0f32 - balance) * (core::f64::consts::PI / 2.0)) as f32).sin();
    #[allow(clippy::as_conversions)]
    let right = ((f64::from(balance) * (core::f64::consts::PI / 2.0)) as f32).sin();
    (left, right)
}

/// `obs_source_audio_input_balance()`: applies `law` to every sample of
/// `left` and `right`.
pub fn balance(left: &mut [f32], right: &mut [f32], balance: f32, law: Law) {
    let (left_gain, right_gain) = match law {
        Law::Sine => sine_gains(balance),
        Law::Square => ((1.0f32 - balance).sqrt(), balance.sqrt()),
        Law::Linear => (1.0f32 - balance, balance),
    };
    for sample in left.iter_mut() {
        *sample *= left_gain;
    }
    for sample in right.iter_mut() {
        *sample *= right_gain;
    }
}

/// The first loop of the downmix: adds `other` into `first`, sample by
/// sample, over the shorter of the two.
pub fn add_plane(first: &mut [f32], other: &[f32]) {
    for (sum, sample) in first.iter_mut().zip(other) {
        *sum += *sample;
    }
}

/// The second loop of the downmix: multiplies `first` by the `f32`
/// reciprocal of `channels`, `1.0f / (float)channels` (+inf for 0).
pub fn scale_plane(first: &mut [f32], channels: usize) {
    // `(float)channels` rounds to nearest, as `as f32` does.
    #[allow(clippy::as_conversions)]
    let channels_i = 1.0f32 / channels as f32;
    for sample in first.iter_mut() {
        *sample *= channels_i;
    }
}

/// The third loop of the downmix: copies `first` to `other`, over the
/// shorter of the two.
pub fn copy_plane(first: &[f32], other: &mut [f32]) {
    for (copy, sample) in other.iter_mut().zip(first) {
        *copy = *sample;
    }
}

/// `obs_source_audio_input_downmix()`: `planes[0]` becomes the mean of the
/// first `channels` planes (each added in turn, then times the reciprocal of
/// `channels`), and is copied to the others. `planes` holds the planes the C
/// reads: `channels` of them, or the first one alone when `channels` is 0
/// (it is then scaled by +inf). Planes past `channels` are not touched.
pub fn downmix(planes: &mut [&mut [f32]], channels: usize) {
    let Some((first, others)) = planes.split_first_mut() else {
        return;
    };
    let others_count = channels.saturating_sub(1);
    for other in others.iter().take(others_count) {
        add_plane(first, other);
    }
    scale_plane(first, channels);
    for other in others.iter_mut().take(others_count) {
        copy_plane(first, other);
    }
}
