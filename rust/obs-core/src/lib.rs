//! Rust ports of the units extracted from `libobs/obs.c.h`, one module per
//! file: so far `libobs/obs-video-plan.c.h`.
//!
//! The safe core is [`video_plan`]. The C ABI shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod video_plan;

#[allow(unsafe_code)]
pub mod ffi;
