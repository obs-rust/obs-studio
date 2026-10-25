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
#[inline(always)]
pub fn set_val<S: ByteStore + ?Sized>(cur: &mut S, changed: &mut bool, data: &[u8]) {
    // Every length below is `data.len()`, a constant in each inlined
    // setter, so the comparison and the copy are inlined as the C's are.
    let n = data.len();
    let size_changed = cur.bytes().len() != n;
    if size_changed {
        cur.resize(n);
    }
    let same = !size_changed && cur.bytes().get(..n).is_some_and(|c| bytes_eq(c, data));
    if !same {
        if let Some(dst) = cur.bytes_mut().get_mut(..n) {
            dst.copy_from_slice(data);
        }
        *changed = true;
    }
}

/// Whether two values of the same length are equal. Up to 64 bytes (every
/// typed setter: a matrix4 is the largest) it ORs the XOR of 8-byte chunks,
/// which the compiler unrolls into a few loads for the constant length of
/// an inlined setter, as GCC expands the C's `memcmp`; LLVM calls `bcmp`
/// for a 64-byte slice comparison. Longer values (`gs_effect_set_val`) use
/// the slice comparison.
#[inline(always)]
fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    if a.len() > 64 {
        return a == b;
    }
    let (a8, a_rest) = a.as_chunks::<8>();
    let (b8, b_rest) = b.as_chunks::<8>();
    let mut diff = 0u64;
    for (x, y) in a8.iter().zip(b8) {
        diff |= u64::from_ne_bytes(*x) ^ u64::from_ne_bytes(*y);
    }
    for (x, y) in a_rest.iter().zip(b_rest) {
        diff |= u64::from(x ^ y);
    }
    diff == 0
}

/// `effect_getval_inline` / `effect_getdefaultval_inline`: copy at most the
/// stored bytes into `dst`; returns the bytes copied.
pub fn copy_out(src: &[u8], dst: &mut [u8]) -> usize {
    let bytes = src.len().min(dst.len());
    if let (Some(to), Some(from)) = (dst.get_mut(..bytes), src.get(..bytes)) {
        to.copy_from_slice(from);
    }
    bytes
}

/// `vec4_from_bgra` (`graphics/vec4.h`): the bytes of `bgra` in memory
/// order, the first and third swapped, each divided by 255. On the
/// little-endian hosts libobs builds for, `0xAARRGGBB` gives
/// `{r, g, b, a} / 255`.
pub fn bgra_to_vec4(bgra: u32) -> [f32; 4] {
    let [b0, b1, b2, b3] = bgra.to_ne_bytes();
    [b2, b1, b0, b3].map(|u| f32::from(u) / 255.0)
}

/// `gs_effect_set_next_sampler`: only a texture parameter takes a sampler.
pub fn takes_next_sampler(param_type: i32) -> bool {
    param_type == GS_SHADER_PARAM_TEXTURE
}
