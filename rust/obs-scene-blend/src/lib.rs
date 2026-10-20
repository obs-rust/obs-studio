//! Rust port of `libobs/obs-scene-blend.c.h`.
//!
//! [`blend`] is the safe core: the blend factors and operation each scene
//! item blend mode draws with.

#![deny(unsafe_code)]

pub mod blend;

pub use blend::{
    BlendMode, BlendOp, BlendParams, BlendType, MODE_COUNT, blend_mode_params, blend_params,
};
