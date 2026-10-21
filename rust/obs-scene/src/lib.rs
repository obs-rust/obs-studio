//! Rust ports of the units extracted from `libobs/obs-scene.c`, one module
//! per file: `libobs/obs-scene-bounds.c.h` and `libobs/obs-scene-blend.c.h`.
//!
//! The safe cores are [`bounds`] and [`blend`]. The C ABI shims live in
//! [`ffi`].

#![deny(unsafe_code)]

pub mod blend;
pub mod bounds;

#[allow(unsafe_code)]
pub mod ffi;
