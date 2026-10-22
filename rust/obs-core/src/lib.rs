//! Rust ports of the units extracted from `libobs/obs.c.h`, one module per
//! file: so far `libobs/obs-video-plan.c.h` and `libobs/obs-context-names.c.h`.
//!
//! The safe cores are [`video_plan`] and [`context_names`]. The C ABI shims
//! live in [`ffi`].

#![deny(unsafe_code)]

pub mod context_names;
pub mod video_plan;

#[allow(unsafe_code)]
pub mod ffi;
