//! Host math conventions used by the unchanged C graphics implementation.

cfg_select! {
    target_os = "macos" => {
        mod macos;
        pub(crate) use macos::sin_cos;
    }
    _ => {
        pub(crate) fn sin_cos(x: f32) -> (f32, f32) {
            (x.sin(), x.cos())
        }
    }
}

/// C's ceil/floor leave large finite values unchanged. Clang/macOS and
/// MSVC also quiet signaling NaNs; GCC's inline x86 sequence preserves them.
#[inline]
pub(crate) fn round_large(x: f32) -> f32 {
    cfg_select! {
        any(target_os = "macos", windows, target_arch = "aarch64") => {
            if x.is_nan() { f32::from_bits(x.to_bits() | 0x0040_0000) } else { x }
        }
        _ => { x }
    }
}

/// True where `f32::ceil`/`f32::floor` are one instruction, as C's
/// `ceilf`/`floorf` are: aarch64 `frintp`/`frintm`, which also quiet
/// signaling NaNs like [`round_large`] (#1428). Elsewhere, baseline x86_64
/// lowers them to libm calls, so `round.rs` uses its inline sequence (#1298).
pub(crate) const NATIVE_ROUND: bool = cfg_select! {
    target_arch = "aarch64" => { true }
    _ => { false }
};
