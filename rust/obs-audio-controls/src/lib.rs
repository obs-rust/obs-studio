//! Rust port of `libobs/obs-audio-controls.c.h`.
//!
//! [`Fader`] and [`Volmeter`] are the safe core. They do not call into obs.
//! A fader remembers an attached source volume. A volmeter eats planar
//! samples and a speaker layout. The shim owns the obs source, the signals,
//! and the one `bzalloc` the cmocka test counts.

#![deny(unsafe_code)]

#[allow(unsafe_code)]
pub mod ffi;

mod curves;
mod fader;
/// The peak kernels, public for the `port_comparison` benches (#3003).
#[doc(hidden)]
pub mod peak;
mod volmeter;

pub use fader::{Fader, FaderCb};
pub use obs_media_io::audio_io::{SpeakerLayout, speaker_layout_to_c};
pub use volmeter::{AudioPacket, MeterCb, Volmeter};

/// `MAX_AUDIO_CHANNELS` in `media-io/audio-io.h`.
pub const MAX_AUDIO_CHANNELS: usize = 8;

/// `OBS_FADER_CUBIC`.
pub const FADER_CUBIC: i32 = 0;
/// `OBS_FADER_IEC`.
pub const FADER_IEC: i32 = 1;
/// `OBS_FADER_LOG`.
pub const FADER_LOG: i32 = 2;

/// `SAMPLE_PEAK_METER`.
pub const SAMPLE_PEAK_METER: i32 = 0;
/// `TRUE_PEAK_METER`.
pub const TRUE_PEAK_METER: i32 = 1;

/// `obs_mul_to_db`. The body is the header inline in `audio-math.h`,
/// already ported in `obs-media-io`.
pub fn mul_to_db(mul: f32) -> f32 {
    obs_media_io::audio_math::mul_to_db(mul)
}

/// `obs_db_to_mul`. The body is the header inline in `audio-math.h`,
/// already ported in `obs-media-io`.
pub fn db_to_mul(db: f32) -> f32 {
    obs_media_io::audio_math::db_to_mul(db)
}
