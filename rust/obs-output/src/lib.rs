//! Rust ports of `libobs/obs-output-delay.c.h`, the output stream delay,
//! `libobs/obs-output-interleave.c.h`, the encoded packet interleaver,
//! `libobs/obs-output-caption-text.c.h`, the caption text wrap,
//! `libobs/obs-output-protocols.c.h`, the output protocol-list match, and
//! `libobs/obs-encoder-roi.c.h`, the encoder region of interest checks and
//! scaling.
//!
//! [`delay`], [`interleave`], [`caption_text`], [`protocols`] and
//! [`encoder_roi`] are the safe cores: [`delay`] decides when a held
//! packet, start or stop leaves the delay queue, and what start, stop and
//! cleanup do to the output; [`interleave`] queues an output's encoded
//! packets until every track has started, starts the tracks together at
//! timestamp 0 and sends the packets in `dts` order; [`caption_text`] wraps
//! caption text into rows of 32 characters and pages of 15 rows;
//! [`protocols`] counts the segments of a service output's `;`-separated
//! protocol list that equal a protocol; [`encoder_roi`] decides which
//! regions of interest an encoder stores and scales them to its scaled
//! size. The C ABI shims live in [`ffi`]. Later units of `obs-output.c`
//! join this crate.

#![deny(unsafe_code)]

pub mod caption_text;
pub mod delay;
pub mod encoder_roi;
pub mod interleave;
pub mod protocols;
pub mod service;

#[allow(unsafe_code)]
pub mod ffi;

pub use delay::{
    Decision, DelayOutput, Entry, NS_PER_SEC, NullLog, OBS_OUTPUT_DELAY_PRESERVE,
    OBS_OUTPUT_ENCODED, SET_DELAY, Signal, active_delay_secs, pop_decision,
};
