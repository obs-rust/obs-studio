//! Port of the `static inline` `util_mul_div64` in
//! `libobs/util/util_uint64.h`. Header-inline, so nothing is exported or
//! swapped.

/// `util_mul_div64`: `num * mul / div`, truncated.
///
/// Matches the C on each platform. On an MSVC x64 target the C uses exact
/// 128-bit math: `_umul128`/`_udiv128` with MSVC, an `unsigned __int128`
/// division with clang-cl. Everywhere else it computes
/// `(num / div) * mul + (num % div * mul) / div` in wrapping `u64`, which
/// differs from the exact result once `num % div * mul` overflows.
///
/// # Panics
///
/// If `div` is 0 (UB in C), and on an MSVC x64 target if the quotient does
/// not fit in 64 bits, where MSVC's `_udiv128` faults and clang-cl's
/// `(uint64_t)` cast truncates it.
pub fn mul_div64(num: u64, mul: u64, div: u64) -> u64 {
    if crate::platform::MUL_DIV64_IS_UDIV128 {
        let quotient = u128::from(num) * u128::from(mul) / u128::from(div);
        u64::try_from(quotient).expect("util_mul_div64 quotient overflows 64 bits")
    } else {
        let rem = num % div;
        (num / div)
            .wrapping_mul(mul)
            .wrapping_add(rem.wrapping_mul(mul) / div)
    }
}
