//! Safe core of `libobs/graphics/effect-params.c.h`: the parameter values
//! of an effect.
//!
//! Every `gs_effect_set_*` setter stores the parameter's new value as bytes
//! in its `cur_val` ([`set_val`]): the store is resized when the size
//! changes, and `changed` is set when the size or the bytes differ, not when
//! the same bytes are set again. The getters copy at most the stored size
//! ([`copy_out`]). `gs_effect_set_color` converts `0xAARRGGBB` to
//! `{r, g, b, a} / 255` ([`bgra_to_vec4`]), and `gs_effect_set_next_sampler`
//! only applies to a texture parameter ([`takes_next_sampler`]).
//!
//! The shim (`crate::ffi::effect_params`) reads `struct gs_effect_param`,
//! keeps the C `DARRAY(uint8_t)` through a [`ByteStore`], logs the C's error
//! lines and allocates the getters' copies with bmem. Nothing here touches
//! the graphics subsystem, keeps state or logs.
//!
//! Characterized, not endorsed (`test_effect_param_values`), and kept as
//! the C has them, in the shim: `gs_effect_set_vec3` stores 12 bytes, not
//! `sizeof(struct vec3)`; `gs_effect_set_bool` stores an `int`;
//! `gs_effect_set_default` without a default passes the NULL array on and
//! logs "invalid data".

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]
#![allow(unused_variables)]

/// `GS_SHADER_PARAM_TEXTURE` of `enum gs_shader_param_type`
/// (`graphics/graphics.h`).
pub const GS_SHADER_PARAM_TEXTURE: i32 = 12;

/// A parameter's stored value, `DARRAY(uint8_t)` in C.
pub trait ByteStore {
    fn bytes(&self) -> &[u8];
    fn bytes_mut(&mut self) -> &mut [u8];
    /// `da_resize`: new bytes are zero.
    fn resize(&mut self, size: usize);
}

/// `effect_setval_inline` with a valid parameter and data: store `data`,
/// resizing when the size differs, and set `changed` when the size or the
/// bytes differ.
pub fn set_val<S: ByteStore + ?Sized>(cur: &mut S, changed: &mut bool, data: &[u8]) {
    todo!()
}

/// `effect_getval_inline` / `effect_getdefaultval_inline`: copy at most the
/// stored bytes into `dst`; returns the bytes copied.
pub fn copy_out(src: &[u8], dst: &mut [u8]) -> usize {
    todo!()
}

/// `vec4_from_bgra` (`graphics/vec4.h`): the bytes of `bgra` in memory
/// order, the first and third swapped, each divided by 255. On the
/// little-endian hosts libobs builds for, `0xAARRGGBB` gives
/// `{r, g, b, a} / 255`.
pub fn bgra_to_vec4(bgra: u32) -> [f32; 4] {
    todo!()
}

/// `gs_effect_set_next_sampler`: only a texture parameter takes a sampler.
pub fn takes_next_sampler(param_type: i32) -> bool {
    todo!()
}
