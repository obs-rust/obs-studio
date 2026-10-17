//! CEA-608 caption emission for OBS outputs.
//!
//! This crate produces `cc_data` pair streams; it does not know about OBS.
//! The roll-up emitter mirrors the proven control sequence and pacing of the
//! `spikes/2026-10-12/rollup` C harness, which FFmpeg's `subcc` filter decoded
//! from real H.264 streams at 30 fps and 60 fps.

pub mod eia608;
pub mod rollup;
