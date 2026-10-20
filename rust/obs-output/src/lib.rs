//! Rust port of `libobs/obs-output-delay.c.h`, the output stream delay.
//!
//! [`delay`] is the safe core: when a held packet, start or stop leaves the
//! queue, and what start, stop and cleanup do to the output. The C ABI
//! shims live in [`ffi`]. Later units of `obs-output.c` join this crate.

#![deny(unsafe_code)]

pub mod delay;

#[allow(unsafe_code)]
pub mod ffi;

pub use delay::{
    Decision, DelayOutput, Entry, NS_PER_SEC, NullLog, OBS_OUTPUT_DELAY_PRESERVE,
    OBS_OUTPUT_ENCODED, SET_DELAY, Signal, active_delay_secs, pop_decision,
};
