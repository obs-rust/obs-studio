//! Rust ports of `libobs/graphics`, and of the units extracted from
//! `libobs/graphics/graphics.c.h`: so far `graphics-sprite.c.h`, the sprite
//! and texture upload geometry ([`sprite`]).
//!
//! Safe cores live at the crate root; the C ABI shims that replace the
//! original `EXPORT` symbols live in [`ffi`]. See
//! `docs/rust-port/testing-policy.md`.

#![deny(unsafe_code)]

#[cfg(test)]
use obs_c_oracle as _; // links the test bmalloc/bfree (oracle/test_bmem.c)

pub mod axisang;
pub mod bounds;
pub mod effect_lookups;
pub mod effect_parser_lifecycle;
#[allow(unsafe_code)] // the C ABI shims
pub mod ffi;
pub mod image_file;
pub mod math_extra;
pub mod matrix3;
pub mod matrix4;
pub mod plane;
mod platform;
pub mod quat;
mod round;
pub mod shader_param_names;
pub mod shader_sampler_convert;
pub mod sprite;
pub mod texture_render;
pub mod vec2;
pub mod vec3;
pub mod vec4;
