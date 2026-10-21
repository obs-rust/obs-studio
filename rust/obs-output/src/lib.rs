//! Rust ports of `libobs/obs-output-delay.c.h`, the output stream delay,
//! `libobs/obs-output-interleave.c.h`, the encoded packet interleaver, and
//! `libobs/obs-output-caption-text.c.h`, the caption text wrap.
//!
//! [`delay`], [`interleave`] and [`caption_text`] are the safe cores:
//! [`delay`] decides when a held packet, start or stop leaves the delay
//! queue, and what start, stop and cleanup do to the output; [`interleave`]
//! queues an output's encoded packets until every track has started, starts
//! the tracks together at timestamp 0 and sends the packets in `dts` order;
//! [`caption_text`] wraps caption text into rows of 32 characters and pages
//! of 15 rows. The C ABI shims live in [`ffi`]. Later units of
//! `obs-output.c` join this crate.

#![deny(unsafe_code)]

pub mod caption_text;
pub mod delay;
pub mod interleave;

#[allow(unsafe_code)]
pub mod ffi;

pub use delay::{
    Decision, DelayOutput, Entry, NS_PER_SEC, NullLog, OBS_OUTPUT_DELAY_PRESERVE,
    OBS_OUTPUT_ENCODED, SET_DELAY, Signal, active_delay_secs, pop_decision,
};
