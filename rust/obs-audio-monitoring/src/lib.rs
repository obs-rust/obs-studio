//! Rust port of `libobs/audio-monitoring/`.
//!
//! [`pulse_stream_flags`] is the safe core of
//! `audio-monitoring/pulse/pulseaudio-stream-flags.c.h`. Its C ABI shims
//! live in [`ffi`].

#![deny(unsafe_code)]

pub mod pulse_stream_flags;

#[allow(unsafe_code)]
pub mod ffi;

pub use pulse_stream_flags::{Backlog, reset_on_sink_event, stream_flags};
