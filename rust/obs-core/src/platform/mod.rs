//! Host conventions of the C that the safe cores follow.

/// Whether `util_mul_div64` (`util/util_uint64.h`) is exact 128-bit math
/// (`_umul128`, `_udiv128`), as on MSVC x64, instead of the wrapping
/// `uint64_t` formula every other host compiles.
pub(crate) const MUL_DIV64_IS_UDIV128: bool =
    cfg!(all(target_env = "msvc", target_arch = "x86_64"));

// Runtime AVX2 for the mix add loop (#3218, #3412): the same scalar loop,
// compiled again with AVX2 so LLVM widens it. Nothing is fused, so both
// paths give the same bits.
cfg_select! {
    target_arch = "x86_64" => {
        #[target_feature(enable = "avx2")]
        fn mix_add_avx2(dst: &mut [f32], src: &[f32]) {
            crate::audio_mix::mix_add_scalar(dst, src);
        }

        pub(crate) fn mix_add(dst: &mut [f32], src: &[f32]) {
            if std::arch::is_x86_feature_detected!("avx2") {
                // SAFETY: the CPU has AVX2, checked on the line above.
                #[allow(unsafe_code)]
                unsafe {
                    mix_add_avx2(dst, src);
                }
            } else {
                crate::audio_mix::mix_add_scalar(dst, src);
            }
        }
    }
    _ => {
        pub(crate) fn mix_add(dst: &mut [f32], src: &[f32]) {
            crate::audio_mix::mix_add_scalar(dst, src);
        }
    }
}
