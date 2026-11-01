//! Host conventions of the C that the safe cores follow.

/// Whether `util_mul_div64` (`util/util_uint64.h`) is exact 128-bit math
/// (`_umul128`, `_udiv128`), as on MSVC x64, instead of the wrapping
/// `uint64_t` formula every other host compiles.
pub(crate) const MUL_DIV64_IS_UDIV128: bool =
    cfg!(all(target_env = "msvc", target_arch = "x86_64"));

/// Whether the two-stream audio kernels (`audio_mix::mix_add`) walk
/// 4-sample groups instead of a plain zip loop. Each is the form LLVM turns
/// into the faster loop on that target: on x86_64 the zip loop, unrolled to
/// two vectors with both loads before both stores, was 18 % slower than
/// GCC's and MSVC's vectorized C, the grouped one level with it; on aarch64
/// the grouped loop is one vector per iteration and 46 % slower than
/// clang's C, the zip loop (four vectors) level with it.
pub(crate) const GROUPED_AUDIO_LOOPS: bool = cfg!(target_arch = "x86_64");

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

// Runtime AVX2 for the mix kernels (#3218, #3412): the same scalar loops,
// compiled again with AVX2 so LLVM widens them. Nothing is fused (Rust
// never contracts on its own), so both paths give the same bits.
cfg_select! {
    target_arch = "x86_64" => {
        #[target_feature(enable = "avx2")]
        fn mix_add_avx2(out: &mut [f32], input: &[f32]) {
            crate::audio_mix::mix_add_scalar(out, input);
        }

        #[target_feature(enable = "avx2")]
        fn mix_with_buf_avx2(out: &mut [f32], input: &[f32], gain: &[f32]) {
            crate::audio_mix::mix_with_buf_scalar(out, input, gain);
        }

        pub(crate) fn mix_add(out: &mut [f32], input: &[f32]) {
            if std::arch::is_x86_feature_detected!("avx2") {
                // SAFETY: the CPU has AVX2, checked on the line above.
                #[allow(unsafe_code)]
                unsafe {
                    mix_add_avx2(out, input);
                }
            } else {
                crate::audio_mix::mix_add_scalar(out, input);
            }
        }

        pub(crate) fn mix_with_buf(out: &mut [f32], input: &[f32], gain: &[f32]) {
            if std::arch::is_x86_feature_detected!("avx2") {
                // SAFETY: the CPU has AVX2, checked on the line above.
                #[allow(unsafe_code)]
                unsafe {
                    mix_with_buf_avx2(out, input, gain);
                }
            } else {
                crate::audio_mix::mix_with_buf_scalar(out, input, gain);
            }
        }
    }
    _ => {
        pub(crate) fn mix_add(out: &mut [f32], input: &[f32]) {
            crate::audio_mix::mix_add_scalar(out, input);
        }

        pub(crate) fn mix_with_buf(out: &mut [f32], input: &[f32], gain: &[f32]) {
            crate::audio_mix::mix_with_buf_scalar(out, input, gain);
        }
    }
}
