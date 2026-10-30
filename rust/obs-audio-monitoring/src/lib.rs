//! Rust port of `libobs/audio-monitoring/`.
//!
//! [`pulse_stream_flags`] is the safe core of
//! `audio-monitoring/pulse/pulseaudio-stream-flags.c.h`, [`pulse_format`]
//! that of `audio-monitoring/pulse/pulseaudio-monitor-format.c.h`, and
//! [`pulse_monitor`] that of `audio-monitoring/pulse/pulseaudio-monitor-steps.c.h`
//! (the monitor's open and write steps of `pulseaudio-output.c`), and
//! [`monitor_delay`] that of `audio-monitoring/monitor-delay.c.h` (the stale
//! cut and the front packet's verdict of the WASAPI monitor's
//! `process_audio_delay()`). Their C ABI shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod monitor_delay;
pub mod pulse_format;
pub mod pulse_monitor;
pub mod pulse_stream_flags;

#[allow(unsafe_code)]
pub mod ffi;

pub use pulse_stream_flags::{Backlog, reset_on_sink_event, stream_flags};
