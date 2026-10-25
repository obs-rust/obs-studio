//! Host conventions of the C that the safe cores follow.

/// Whether `util_mul_div64` (`util/util_uint64.h`) is exact 128-bit math
/// (`_umul128`, `_udiv128`), as on MSVC x64, instead of the wrapping
/// `uint64_t` formula every other host compiles.
pub(crate) const MUL_DIV64_IS_UDIV128: bool =
    cfg!(all(target_env = "msvc", target_arch = "x86_64"));

// `quat_from_axisang`'s sinf() and cosf(), as `obs-graphics` calls them on
// each host (`item_transform` writes the rotation out, so the obs-scene
// object does not call into obs-graphics).
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
