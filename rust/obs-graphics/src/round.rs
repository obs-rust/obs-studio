//! Inline `ceilf`/`floorf` for the vector cores (obs-rust/obs-studio#1298).
//!
//! On the x86_64 baseline (SSE2, no SSE4.1 `roundss`) LLVM lowers
//! `f32::ceil`/`f32::floor` to a libm call. GCC, building the C original,
//! inlines `ceilf`/`floorf` as a truncate-and-adjust sequence instead:
//! `|x| < 2^23` ? `copysign(trunc(x) ± (x ≷ trunc(x)), x)` : `x`. These
//! helpers do the same in portable Rust (rounding with the `±2^23` add
//! trick in place of the truncating cast), so they compile to inline SSE2
//! code without raising the CPU baseline.
//!
//! Results equal `f32::ceil`/`f32::floor` bit for bit for every non-NaN
//! input (the category suite checks all 2^32 bit patterns). NaNs follow
//! the host C convention: the GCC-inlined x86 sequence preserves their bits;
//! MSVC and macOS quiet signaling NaNs.
//!
//! Targets with a single-instruction round (`platform::NATIVE_ROUND`,
//! aarch64) use `f32::ceil`/`f32::floor` directly, as C does there (#1428).

/// 2^23: every `f32` with at least this magnitude is already an integer.
const TWO_POW_23: f32 = 8_388_608.0;

/// Rounds `x` (with `|x| < 2^23`) to an integer, ties to even.
///
/// Adding then subtracting `±2^23` pushes the fraction bits out of the
/// mantissa; both steps are exact apart from that one rounding. This is
/// cheaper than `x as i32 as f32`, whose saturating Rust cast adds compares
/// and conditional moves on x86_64 that GCC's C `(int)` cast does not have.
#[inline]
fn round_small(x: f32) -> f32 {
    let s = TWO_POW_23.copysign(x);
    (x + s) - s
}

/// `ceilf(x)`, inline.
#[inline]
#[must_use]
pub(crate) fn ceil_f32(x: f32) -> f32 {
    if crate::platform::NATIVE_ROUND {
        return x.ceil();
    }
    // False for NaN: the host boundary handles its signaling convention.
    if x.abs() < TWO_POW_23 {
        let t = round_small(x);
        let up = if x > t { 1.0 } else { 0.0 };
        // Keeps -0.0 for inputs in (-1, -0.0].
        (t + up).copysign(x)
    } else {
        core::hint::cold_path();
        crate::platform::round_large(x)
    }
}

/// `floorf(x)`, inline.
#[inline]
#[must_use]
pub(crate) fn floor_f32(x: f32) -> f32 {
    if crate::platform::NATIVE_ROUND {
        return x.floor();
    }
    if x.abs() < TWO_POW_23 {
        let t = round_small(x);
        let down = if x < t { 1.0 } else { 0.0 };
        (t - down).copysign(x)
    } else {
        core::hint::cold_path();
        crate::platform::round_large(x)
    }
}
