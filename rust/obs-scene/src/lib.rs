//! Rust ports of the units extracted from `libobs/obs-scene.c`, one module
//! per file: `libobs/obs-scene-bounds.c.h`, `libobs/obs-scene-blend.c.h`,
//! `libobs/obs-scene-bounds-place.c.h`, `libobs/obs-scene-coords.c.h`,
//! `libobs/obs-scene-group-transform.c.h`, and
//! `libobs/obs-scene-audio-actions.c.h`.
//!
//! The safe cores are [`bounds`], [`bounds_place`], [`blend`], [`coords`],
//! [`group_transform`], and [`audio_actions`]. The C ABI shims live in
//! [`ffi`].

#![deny(unsafe_code)]

pub mod audio_actions;
pub mod blend;
pub mod bounds;
pub mod bounds_place;
pub mod coords;
pub mod group_transform;
mod platform;

#[allow(unsafe_code)]
pub mod ffi;
