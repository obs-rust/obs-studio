//! Host conventions of the C that the safe cores follow.

/// Whether `util_mul_div64` (`util/util_uint64.h`) is exact 128-bit math
/// (`_umul128`, `_udiv128`), as on MSVC x64, instead of the wrapping
/// `uint64_t` formula every other host compiles.
pub(crate) const MUL_DIV64_IS_UDIV128: bool =
    cfg!(all(target_env = "msvc", target_arch = "x86_64"));
