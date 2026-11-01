//! Host conventions of the C that the safe cores follow.

/// Whether `util_mul_div64` (`util/util_uint64.h`) is exact 128-bit math
/// (`_umul128`, `_udiv128`), as on MSVC x64, instead of the wrapping
/// `uint64_t` formula every other host compiles.
pub(crate) const MUL_DIV64_IS_UDIV128: bool =
    cfg!(all(target_env = "msvc", target_arch = "x86_64"));

/// Whether the two-stream audio kernels (`volume_mul::mul_data`) walk
/// 4-sample groups instead of a plain zip loop. Each is the form LLVM turns
/// into the faster loop on that target: on x86_64 the zip loop was 17 %
/// slower than GCC's and MSVC's vectorized C, the grouped one 3 %; on
/// aarch64 the grouped loop is one vector per iteration and 49 % slower
/// than clang's C, the zip loop level with it.
pub(crate) const GROUPED_AUDIO_LOOPS: bool = cfg!(target_arch = "x86_64");

// Runtime AVX2 for the per-frame volume multiply (#3218, #3413): the same
// scalar loop, compiled again with AVX2 so LLVM widens it. Nothing is
// fused, so both paths give the same bits.
cfg_select! {
    target_arch = "x86_64" => {
        #[target_feature(enable = "avx2")]
        fn mul_data_avx2(out: &mut [f32], vol: &[f32]) {
            crate::volume_mul::mul_data_scalar(out, vol);
        }

        pub(crate) fn mul_data(out: &mut [f32], vol: &[f32]) {
            if std::arch::is_x86_feature_detected!("avx2") {
                // SAFETY: the CPU has AVX2, checked on the line above.
                #[allow(unsafe_code)]
                unsafe {
                    mul_data_avx2(out, vol);
                }
            } else {
                crate::volume_mul::mul_data_scalar(out, vol);
            }
        }
    }
    _ => {
        pub(crate) fn mul_data(out: &mut [f32], vol: &[f32]) {
            crate::volume_mul::mul_data_scalar(out, vol);
        }
    }
}
