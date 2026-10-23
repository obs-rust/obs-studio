//! Rust ports of the units extracted from `libobs/obs.c.h` and
//! `libobs/obs-module.c.h`, one module per file: so far
//! `libobs/obs-video-plan.c.h`, `libobs/obs-context-names.c.h` and
//! `libobs/obs-module-paths.c.h`.
//!
//! The safe cores are [`video_plan`], [`context_names`] and
//! [`module_paths`]. The C ABI shims live in [`ffi`].

#![deny(unsafe_code)]

pub mod context_names;
pub mod module_paths;
pub mod video_plan;

#[allow(unsafe_code)]
pub mod ffi;
