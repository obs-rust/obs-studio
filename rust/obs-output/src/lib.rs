//! Rust port of `libobs/obs-output-delay.c.h`, the output stream delay.
//!
//! [`delay`] is the safe core: when a held packet, start or stop leaves the
//! queue, and what start, stop and cleanup do to the output.

#![deny(unsafe_code)]

pub mod delay;

pub use delay::{
    Decision, DelayOutput, Entry, NS_PER_SEC, NullLog, OBS_OUTPUT_DELAY_PRESERVE,
    OBS_OUTPUT_ENCODED, SET_DELAY, Signal, active_delay_secs, pop_decision,
};
