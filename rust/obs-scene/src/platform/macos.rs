//! Keep the C's separate sinf() and cosf() calls (`quat_from_axisang`).
//!
//! Apple's paired `__sincosf_stret` can differ by one ULP from separate
//! calls, and LLVM pairs Rust's intrinsics at opt-level 3, while the C
//! calls each one. Separate non-inlined functions keep that, as
//! `obs-graphics`' platform module does for the same C.

#[inline(never)]
fn sin(x: f32) -> f32 {
    x.sin()
}

#[inline(never)]
fn cos(x: f32) -> f32 {
    x.cos()
}

pub(crate) fn sin_cos(x: f32) -> (f32, f32) {
    (sin(x), cos(x))
}
