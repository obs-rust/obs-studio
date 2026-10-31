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
//!   the filter by a packet and gives the `sample_delta` for
//!   `swr_set_compensation()`; [`count`] keeps the sample counts while it is
//!   engaged and [`drift_report`] gives the estimated input rate logged when
//!   the resampler is destroyed.
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
    clippy::unimplemented,
    clippy::unreachable
)]

use core::ffi::c_int;

use crate::audio_io::MAX_AUDIO_CHANNELS;

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

/// `enum audio_resampler_comp_action`, with the delta beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// `AUDIO_RESAMPLER_COMP_IDLE`: not engaged, and the error is 0.
    Idle,
    /// `AUDIO_RESAMPLER_COMP_ENGAGE`: the error left the dead band for the
    /// first time; log it, then set the delta.
    Engage {
        /// The `sample_delta` for `swr_set_compensation()`.
        delta: c_int,
    },
    /// `AUDIO_RESAMPLER_COMP_SET`: set the delta.
    Set {
        /// The `sample_delta` for `swr_set_compensation()`.
        delta: c_int,
    },
}

impl Action {
    /// The C value of `enum audio_resampler_comp_action`.
    pub fn to_c(self) -> u32 {
        match self {
            Self::Idle => 0,
            Self::Engage { .. } => 1,
            Self::Set { .. } => 2,
        }
    }

    /// The delta, unless [`Action::Idle`].
    pub fn delta(self) -> Option<c_int> {
        match self {
            Self::Idle => None,
            Self::Engage { delta } | Self::Set { delta } => Some(delta),
        }
    }
}

/// `audio_resampler_sample_format()`: the sample format ([`fmt`]) for the
/// `enum audio_format` value `format`; S16 for unknown and for any value
/// outside the enum.
pub fn sample_format(format: c_int) -> c_int {
    let _ = (format,);
    todo!()
}

/// `audio_resampler_upmix_row()`: the matrix row for a mono input to
/// `output_ch` channels, or `None` for 0 or more than
/// [`MAX_AUDIO_CHANNELS`].
pub fn upmix_row(output_ch: u32) -> Option<[f64; MAX_AUDIO_CHANNELS]> {
    let _ = (output_ch,);
    todo!()
}

/// `audio_resampler_comp_set_error()`: the first call turns compensation on
/// and clears the filter; then the error less the dead band (0 within it).
pub fn set_error(c: &mut Compensation, error_ns: i64) {
    let _ = (c, error_ns);
    todo!()
}

/// `audio_resampler_comp_step()`: one step of the lag-lead loop filter over
/// `in_frames` frames at `input_freq`. Both states advance, clamped to
/// [`MAX_PPM`] of the input rate. While not engaged and the error is 0,
/// nothing more happens; otherwise it engages (once) and gives the delta:
/// a positive drift (the device delivers too many samples) drops samples,
/// a negative delta.
pub fn step(c: &mut Compensation, input_freq: u32, in_frames: u32) -> Action {
    let _ = (c, input_freq, in_frames);
    todo!()
}

/// `audio_resampler_output_frames()`: `estimated`, padded by 1/512 and 2
/// frames while engaged for the samples compensation adds. Wraps where the
/// C's `int` would overflow (no estimate comes near).
pub fn output_frames(estimated: c_int, engaged: bool) -> c_int {
    let _ = (estimated, engaged);
    todo!()
}

/// `audio_resampler_comp_count()`: while engaged, counts `in_frames` input
/// and `converted` output samples (`converted` cast to `uint64_t`).
pub fn count(c: &mut Compensation, in_frames: u32, converted: c_int) {
    let _ = (c, in_frames, converted);
    todo!()
}

/// `audio_resampler_comp_drift_report()`: when engaged with output counted,
/// the input rate the counts give at `output_freq` (divided first, as the
/// C).
pub fn drift_report(c: &Compensation, output_freq: u32) -> Option<f64> {
    let _ = (c, output_freq);
    todo!()
}
