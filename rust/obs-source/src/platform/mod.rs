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
