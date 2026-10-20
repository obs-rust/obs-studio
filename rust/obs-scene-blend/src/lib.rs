//! Rust port of `libobs/obs-scene-blend.c.h`.
//!
//! [`blend`] is the safe core: the blend factors and operation each scene
//! item blend mode draws with. The C ABI table lives in [`ffi`].

#![deny(unsafe_code)]

pub mod blend;

#[allow(unsafe_code)]
pub mod ffi;

pub use blend::{
    BlendMode, BlendOp, BlendParams, BlendType, MODE_COUNT, blend_mode_params, blend_params,
};
