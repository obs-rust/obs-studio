//! C ABI shims for the exported functions in `libobs/graphics/vec2.h`.
//!
//! The shims only convert to the safe core in [`crate::vec2`]. Each reads
//! its input by value before writing `dst`, so C callers may alias `dst`
//! and the input.

use core::ffi::c_int;
use core::ptr;

use crate::vec2::Vec2;

/// `struct vec2`. The C union with `float ptr[2]` has the same layout:
/// size 8, align 4, `x` at 0, `y` at 4, `ptr` at 0.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct vec2 {
    pub x: f32,
    pub y: f32,
}

impl From<vec2> for Vec2 {
    fn from(v: vec2) -> Self {
        Self::new(v.x, v.y)
    }
}

impl From<Vec2> for vec2 {
    fn from(v: Vec2) -> Self {
        Self { x: v.x, y: v.y }
    }
}

/// Writes `v` to `dst` as one 8-byte store, as GCC does for the C original.
///
/// Two 4-byte stores (what `ptr::write` of the struct lowers to after the
/// branchy inline ceil/floor) cannot forward to a following 8-byte load of
/// `*dst`, which stalls a caller that reads the whole struct back (#1298).
///
/// With the native round (`platform::NATIVE_ROUND`, aarch64) a plain
/// struct write lets LLVM keep both lanes in one vector register
/// (`frintm.2s` and one 8-byte store, as C does), so this is skipped there
/// (#1428).
///
/// # Safety
///
/// `dst` must be valid for writes of a `vec2` (align 4 suffices).
#[inline]
unsafe fn write_packed(dst: *mut vec2, v: Vec2) {
    if crate::platform::NATIVE_ROUND {
        // SAFETY: the caller guarantees `dst` is writable.
        unsafe { ptr::write(dst, v.into()) };
        return;
    }
    let mut bytes = [0u8; 8];
    bytes[..4].copy_from_slice(&v.x.to_ne_bytes());
    bytes[4..].copy_from_slice(&v.y.to_ne_bytes());
    // SAFETY: the caller guarantees `dst` is writable for 8 bytes; the
    // unaligned write needs no 8-byte alignment.
    unsafe { ptr::write_unaligned(dst.cast::<u64>(), u64::from_ne_bytes(bytes)) };
}

/// Writes the per-component absolute value of `*v` to `*dst`.
///
/// # Safety
///
/// `v` must be valid for reads and `dst` valid for writes of a `vec2`.
/// `dst` may equal `v`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vec2_abs(dst: *mut vec2, v: *const vec2) {
    // SAFETY: the caller guarantees `v` is readable; read before writing `dst`.
    let src = unsafe { ptr::read(v) };
    // SAFETY: the caller guarantees `dst` is writable.
    unsafe { ptr::write(dst, Vec2::from(src).abs().into()) };
}

/// Writes the per-component floor of `*v` to `*dst`.
///
/// # Safety
///
/// `v` must be valid for reads and `dst` valid for writes of a `vec2`.
/// `dst` may equal `v`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vec2_floor(dst: *mut vec2, v: *const vec2) {
    // SAFETY: the caller guarantees `v` is readable; read before writing `dst`.
    let src = unsafe { ptr::read(v) };
    // SAFETY: the caller guarantees `dst` is writable.
    unsafe { write_packed(dst, Vec2::from(src).floor()) };
}

/// Writes the per-component ceiling of `*v` to `*dst`.
///
/// # Safety
///
/// `v` must be valid for reads and `dst` valid for writes of a `vec2`.
/// `dst` may equal `v`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vec2_ceil(dst: *mut vec2, v: *const vec2) {
    // SAFETY: the caller guarantees `v` is readable; read before writing `dst`.
    let src = unsafe { ptr::read(v) };
    // SAFETY: the caller guarantees `dst` is writable.
    unsafe { write_packed(dst, Vec2::from(src).ceil()) };
}

/// Returns 1 when `*v1` and `*v2` differ by at most `epsilon` per
/// component, else 0.
///
/// # Safety
///
/// `v1` and `v2` must be valid for reads of a `vec2`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vec2_close(v1: *const vec2, v2: *const vec2, epsilon: f32) -> c_int {
    // SAFETY: the caller guarantees both pointers are readable.
    let (a, b) = unsafe { (ptr::read(v1), ptr::read(v2)) };
    c_int::from(Vec2::from(a).close(Vec2::from(b), epsilon))
}

/// Writes the normalized `*v` to `*dst`. When the length is zero or NaN,
/// `*dst` is left unchanged.
///
/// # Safety
///
/// `v` must be valid for reads of a `vec2`, and `dst` valid for writes
/// unless the result is left unchanged. `dst` may equal `v`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vec2_norm(dst: *mut vec2, v: *const vec2) {
    // SAFETY: the caller guarantees `v` is readable; read before writing `dst`.
    let src = unsafe { ptr::read(v) };
    if let Some(n) = Vec2::from(src).norm() {
        // SAFETY: the caller guarantees `dst` is writable.
        unsafe { ptr::write(dst, n.into()) };
    }
}
