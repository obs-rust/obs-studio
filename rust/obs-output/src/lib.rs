//! Rust ports of `libobs/obs-output-delay.c.h`, the output stream delay,
//! `libobs/obs-output-interleave.c.h`, the encoded packet interleaver,
//! `libobs/obs-output-caption-text.c.h`, the caption text wrap,
//! `libobs/obs-output-protocols.c.h`, the output protocol-list match,
//! `libobs/obs-encoder-roi.c.h`, the encoder region of interest checks and
//! scaling, `libobs/obs-output-video-checks.c.h`, the output video size,
//! conversion and capture checks, `libobs/obs-encoder-size.c.h`, the
//! encoder size and info choices, `libobs/obs-encoder-audio-start.c.h`, the
//! encoder audio start alignment and setter gates, and
//! `libobs/obs-output-stop-stats.c.h`, the output stop statistics.
//!
//! [`delay`], [`interleave`], [`caption_text`], [`protocols`],
//! [`encoder_roi`], [`video_checks`], [`encoder_size`],
//! [`encoder_audio_start`] and [`stop_stats`] are the safe cores: [`delay`] decides when a held
//! packet, start or stop leaves the delay queue, and what start, stop and
//! cleanup do to the output;
//! [`interleave`] queues an output's encoded packets until every track has
//! started, starts the tracks together at timestamp 0 and sends the packets
//! in `dts` order; [`caption_text`] wraps caption text into rows of 32
//! characters and pages of 15 rows; [`protocols`] counts the segments of a
//! service output's `;`-separated protocol list that equal a protocol;
//! [`encoder_roi`] decides which regions of interest an encoder stores and
//! scales them to its scaled size; [`video_checks`] picks an output's first
//! mixer, where its size comes from and its video conversion, checks that
//! it has what it needs to begin capturing, and clamps its congestion;
//! [`encoder_size`] fills an encoder's audio info from the mix, applies the
//! scaling cap to its video info, decides whether a scaled size is refused,
//! disabled or set, and where its width, height and sample rate come from;
//! [`encoder_audio_start`] decides when an encoder's audio starts against
//! its paired video (wait, truncate, clear or start from the buffer), the
//! pts steps, and the first refusal and divisor rule of the setters;
//! [`stop_stats`] decides the numbers and lines an output logs when it
//! stops (frames output, drawn, lagged and dropped, and their percentages).
//! The C ABI shims live in [`ffi`]. Later units of `obs-output.c` join this
//! crate.

#![deny(unsafe_code)]

pub mod caption_text;
pub mod delay;
pub mod encoder_audio_start;
pub mod encoder_roi;
pub mod encoder_size;
pub mod interleave;
pub mod protocols;
pub mod service;
pub mod stop_stats;
pub mod video_checks;

#[allow(unsafe_code)]
pub mod ffi;

pub use delay::{
    Decision, DelayOutput, Entry, NS_PER_SEC, NullLog, OBS_OUTPUT_DELAY_PRESERVE,
    OBS_OUTPUT_ENCODED, SET_DELAY, Signal, active_delay_secs, pop_decision,
};
