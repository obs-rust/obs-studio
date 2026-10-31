//! Safe core of `libobs/media-io/audio-resampler-drift.c.h`: the decisions of
//! the libswresample resampler (`audio-resampler-ffmpeg.c`).
//!
//! - [`sample_format`]: the libavutil sample format for an `enum
//!   audio_format`.
//! - [`upmix_row`]: the matrix row for a mono input to more channels.
//! - the clock drift compensation, after Norihiro Kamae's upstream
//!   obs-studio PR 6351: a capture device on its own clock delivers a few
//!   samples per second more or fewer than the system clock consumes, and a
//!   lag-lead loop filter turns the error the source reports into a slight
//!   resampling ratio, the way a PLL locks to a reference. [`set_error`]
//!   takes the error less the dead band ([`DEAD_BAND_NS`]); [`step`] advances
//!   the filter by a packet and says whether a delta is set (and whether it
//!   engages, which the C logs first); [`delta`] then gives the
//!   `sample_delta` for `swr_set_compensation()`; [`count`] keeps the sample
//!   counts while it is engaged; [`reports_drift`] gates the report logged
//!   when the resampler is destroyed and [`input_rate`] gives its estimated
//!   input rate.
//! - [`output_frames`]: the room the compensation needs.
//!
//! `audio-resampler-ffmpeg.c` keeps every libswresample and libavutil call
//! (`swr_set_matrix()` and `swr_set_compensation()` among them), the output
//! buffer and its size, and every log. The state is the C's
//! [`Compensation`], which stays in the resampler; nothing here keeps state of
//! its own, takes a lock or allocates.
//!
//! The float arithmetic is the C's, operation for operation and in the same
//! order; Rust never fuses a multiply and an add, and the C file is built
//! with `-ffp-contract=off` so it doesn't either. The counters wrap as
//! `uint64_t` does. Nothing here can panic on any input.

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

use core::ffi::c_int;

use crate::audio_io::{AudioFormat, MAX_AUDIO_CHANNELS, audio_format_from_c};

/// `AUDIO_RESAMPLER_COMP_DEAD_BAND_NS`: errors within this are arrival
/// jitter or a start offset, not drift, so a device on the system clock is
/// never resampled.
pub const DEAD_BAND_NS: i64 = 10_000_000;
/// `AUDIO_RESAMPLER_COMP_MAX_PPM`: the bound on the correction.
pub const MAX_PPM: f64 = 1000.0;
/// `AUDIO_RESAMPLER_COMP_DISTANCE`: swr applies the delta over this many
/// samples (0.06 ppm steps).
pub const DISTANCE: c_int = 1 << 24;
/// `AUDIO_RESAMPLER_COMP_GAIN`: the charge pump's gain.
pub const GAIN: f64 = 3.756e-2;
/// `AUDIO_RESAMPLER_COMP_C1`: the capacitor the charge pump feeds.
pub const C1: f64 = 8.250;
/// `AUDIO_RESAMPLER_COMP_C2`: the capacitor in series with 1 ohm.
pub const C2: f64 = 107.1;

/// The libavutil sample formats (`enum AVSampleFormat`) the resampler uses,
/// as `AUDIO_RESAMPLER_FMT_*` gives them.
pub mod fmt {
    use core::ffi::c_int;

    /// `AV_SAMPLE_FMT_U8`.
    pub const U8: c_int = 0;
    /// `AV_SAMPLE_FMT_S16`.
    pub const S16: c_int = 1;
    /// `AV_SAMPLE_FMT_S32`.
    pub const S32: c_int = 2;
    /// `AV_SAMPLE_FMT_FLT`.
    pub const FLT: c_int = 3;
    /// `AV_SAMPLE_FMT_U8P`.
    pub const U8P: c_int = 5;
    /// `AV_SAMPLE_FMT_S16P`.
    pub const S16P: c_int = 6;
    /// `AV_SAMPLE_FMT_S32P`.
    pub const S32P: c_int = 7;
    /// `AV_SAMPLE_FMT_FLTP`.
    pub const FLTP: c_int = 8;
}

/// `struct audio_resampler_comp`: the compensation state the resampler
/// holds. The layout is the C's.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Compensation {
    /// Set by the first error the source reports.
    pub enabled: bool,
    /// Set once the error first leaves the dead band; swr resamples from
    /// then on.
    pub engaged: bool,
    /// The error less the dead band, in nanoseconds.
    pub error_ns: i64,
    /// Loop filter state, in samples per second: the correction.
    pub vc1: f64,
    /// Loop filter state, in samples per second: the second capacitor.
    pub vc2: f64,
    /// Input samples counted while engaged.
    pub input_samples: u64,
    /// Output samples counted while engaged.
    pub output_samples: u64,
}

/// `enum audio_resampler_comp_action`: what follows a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// `AUDIO_RESAMPLER_COMP_IDLE`: not engaged, and the error is 0.
    Idle,
    /// `AUDIO_RESAMPLER_COMP_ENGAGE`: the error left the dead band for the
    /// first time; log it, then set the [`delta`].
    Engage,
    /// `AUDIO_RESAMPLER_COMP_SET`: set the [`delta`].
    Set,
}

impl Action {
    /// The C value of `enum audio_resampler_comp_action`.
    pub fn to_c(self) -> u32 {
        match self {
            Self::Idle => 0,
            Self::Engage => 1,
            Self::Set => 2,
        }
    }
}

/// `audio_resampler_sample_format()`: the sample format ([`fmt`]) for the
/// `enum audio_format` value `format`; S16 for unknown and for any value
/// outside the enum.
pub fn sample_format(format: c_int) -> c_int {
    match audio_format_from_c(format) {
        Some(AudioFormat::U8Bit) => fmt::U8,
        Some(AudioFormat::Bit32) => fmt::S32,
        Some(AudioFormat::Float) => fmt::FLT,
        Some(AudioFormat::U8BitPlanar) => fmt::U8P,
        Some(AudioFormat::Bit16Planar) => fmt::S16P,
        Some(AudioFormat::Bit32Planar) => fmt::S32P,
        Some(AudioFormat::FloatPlanar) => fmt::FLTP,
        Some(AudioFormat::Unknown | AudioFormat::Bit16) | None => fmt::S16,
    }
}

/// The upmix rows of a mono input to 1 to 8 channels: the front pair and the
/// centre take the signal, the LFE doesn't.
const UPMIX: [[f64; MAX_AUDIO_CHANNELS]; MAX_AUDIO_CHANNELS] = [
    [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0],
    [1.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0],
    [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0],
    [1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0],
    [1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0],
];

/// `audio_resampler_upmix_row()`: the matrix row for a mono input to
/// `output_ch` channels, or `None` for 0 or more than
/// [`MAX_AUDIO_CHANNELS`].
pub fn upmix_row(output_ch: u32) -> Option<[f64; MAX_AUDIO_CHANNELS]> {
    let index = usize::try_from(output_ch).ok()?.checked_sub(1)?;
    UPMIX.get(index).copied()
}

/// `audio_resampler_comp_set_error()`: the first call turns compensation on
/// and clears the filter; then the error less the dead band (0 within it).
pub fn set_error(c: &mut Compensation, error_ns: i64) {
    if !c.enabled {
        c.enabled = true;
        c.vc1 = 0.0;
        c.vc2 = 0.0;
    }

    c.error_ns = if error_ns > DEAD_BAND_NS {
        error_ns.wrapping_sub(DEAD_BAND_NS)
    } else if error_ns < -DEAD_BAND_NS {
        error_ns.wrapping_add(DEAD_BAND_NS)
    } else {
        0
    };
}

/// `clamp_drift()`: `drift` within `max`; a NaN stays NaN.
fn clamp_drift(drift: f64, max: f64) -> f64 {
    if drift > max {
        max
    } else if drift < -max {
        -max
    } else {
        drift
    }
}

/// `lround()` of a value well within `c_int`, cast to `int`: half away from
/// zero. The correction is clamped to [`MAX_PPM`], so the value is at most
/// about 16,777 either way; a NaN (a zero input rate, which `swr_init()`
/// rejects first) is 0, as glibc's `lround()` truncated to `int`.
#[allow(clippy::as_conversions)] // bounded above; NaN saturates to 0 as in C
fn round_to_int(value: f64) -> c_int {
    value.round() as c_int
}

/// `audio_resampler_comp_step()`: one step of the lag-lead loop filter over
/// `in_frames` frames at `input_freq`. Both states advance, clamped to
/// [`MAX_PPM`] of the input rate. While not engaged and the error is 0,
/// nothing more happens; otherwise it engages (once), and the caller sets
/// the [`delta`].
pub fn step(c: &mut Compensation, input_freq: u32, in_frames: u32) -> Action {
    let fs = f64::from(input_freq);
    let max = MAX_PPM * 1e-6 * fs;

    // Lag-lead filter: a charge pump (GAIN) into C1 in parallel with a
    // series 1 ohm and C2, advanced by in_frames samples.
    #[allow(clippy::as_conversions)] // (double)c->error_ns
    let icp = GAIN * (c.error_ns as f64) * 1e-9;
    let ic2 = (c.vc1 - c.vc2) / fs;
    let ic1 = icp - ic2;

    c.vc1 = clamp_drift(c.vc1 + ic1 / C1 * f64::from(in_frames), max);
    c.vc2 = clamp_drift(c.vc2 + ic2 / C2 * f64::from(in_frames), max);

    if c.engaged {
        return Action::Set;
    }
    if c.error_ns == 0 {
        return Action::Idle;
    }
    c.engaged = true;
    Action::Engage
}

/// `audio_resampler_comp_delta()`: the `sample_delta` for
/// `swr_set_compensation()` over [`DISTANCE`] samples from the correction at
/// `input_freq`. A positive drift (the device delivers too many samples)
/// drops samples, a negative delta.
pub fn delta(c: &Compensation, input_freq: u32) -> c_int {
    let fs = f64::from(input_freq);
    round_to_int(c.vc1 * f64::from(DISTANCE) / fs).wrapping_neg()
}

/// `audio_resampler_output_frames()`: `estimated`, padded by 1/512 and 2
/// frames while engaged for the samples compensation adds. Wraps where the
/// C's `int` would overflow (no estimate comes near).
pub fn output_frames(estimated: c_int, engaged: bool) -> c_int {
    if engaged {
        estimated.wrapping_add(estimated.wrapping_div(512).wrapping_add(2))
    } else {
        estimated
    }
}

/// `audio_resampler_comp_count()`: while engaged, counts `in_frames` input
/// and `converted` output samples (`converted` cast to `uint64_t`).
pub fn count(c: &mut Compensation, in_frames: u32, converted: c_int) {
    if c.engaged {
        c.input_samples = c.input_samples.wrapping_add(u64::from(in_frames));
        c.output_samples = c
            .output_samples
            .wrapping_add(i64::from(converted).cast_unsigned());
    }
}

/// `audio_resampler_comp_reports_drift()`: whether the resampler reports
/// the drift when it is destroyed: engaged, with output counted.
pub fn reports_drift(c: &Compensation) -> bool {
    c.engaged && c.output_samples != 0
}

/// `audio_resampler_comp_input_rate()`: the input rate the counts give at
/// `output_freq` (divided first, as the C). Asked only when
/// [`reports_drift`]; with no output counted it is the C's division by 0
/// (infinity, or NaN with no input either).
#[allow(clippy::as_conversions)] // (double) of the uint64_t counts
pub fn input_rate(c: &Compensation, output_freq: u32) -> f64 {
    c.input_samples as f64 / c.output_samples as f64 * f64::from(output_freq)
}
