//! Tier 3: the Rust core behaves exactly like `util_mul_div64` in the
//! original `util/util_uint64.h`, compiled as an oracle.
//!
//! No intentional differences from C. The C picks its math per platform
//! (exact 128-bit on MSVC x64, wrapping u64 elsewhere), and so does the
//! port, so this runs on each CI platform against that platform's C.
//!
//! Excluded inputs (C UB or fault, the port panics instead):
//! - `div == 0`, on every platform.
//! - On MSVC x64, quotients that do not fit in 64 bits (`_udiv128`
//!   faults). Off MSVC x64 they wrap, and are compared.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use obs_c_oracle::util_uint64 as c;
use obs_util::util_uint64::mul_div64;
use proptest::prelude::*;

const EXACT: bool = cfg!(all(target_env = "msvc", target_arch = "x86_64"));

/// Inputs the C cannot run: see the module docs.
fn excluded(num: u64, mul: u64, div: u64) -> bool {
    div == 0
        || (EXACT && u128::from(num) * u128::from(mul) / u128::from(div) > u128::from(u64::MAX))
}

fn check(num: u64, mul: u64, div: u64) {
    if excluded(num, mul, div) {
        return;
    }
    // SAFETY: plain integer arguments; the inputs the C cannot run are
    // excluded above.
    let theirs = unsafe { c::oracle_util_mul_div64(num, mul, div) };
    assert_eq!(
        mul_div64(num, mul, div),
        theirs,
        "util_mul_div64({num:#x}, {mul:#x}, {div:#x})"
    );
}

/// Values around the 32- and 64-bit edges, where the products overflow.
const EDGES: [u64; 16] = [
    0,
    1,
    2,
    3,
    1000,
    48000,
    1_000_000_000,
    (1 << 32) - 1,
    1 << 32,
    (1 << 32) + 1,
    (1 << 63) - 1,
    1 << 63,
    (1 << 63) + 1,
    u64::MAX - 2,
    u64::MAX - 1,
    u64::MAX,
];

#[test]
fn edge_values_match_c_oracle() {
    for num in EDGES {
        for mul in EDGES {
            for div in EDGES {
                check(num, mul, div);
            }
        }
    }
}

fn edgy() -> impl Strategy<Value = u64> {
    prop_oneof![
        prop::sample::select(&EDGES[..]),
        any::<u64>(),
        any::<u32>().prop_map(u64::from),
        (0u32..64).prop_flat_map(|bits| any::<u64>().prop_map(move |v| v >> bits)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn matches_c_oracle(num in any::<u64>(), mul in any::<u64>(), div in any::<u64>()) {
        check(num, mul, div);
    }

    #[test]
    fn matches_c_oracle_near_edges(num in edgy(), mul in edgy(), div in edgy()) {
        check(num, mul, div);
    }

    #[test]
    fn matches_c_oracle_remainder_product_overflow(
        div in 2u64..,
        num_seed in any::<u64>(),
        mul in any::<u64>(),
        quotient in any::<u32>(),
    ) {
        // Small quotient, large remainder: rem * mul overflows while the
        // exact result still fits, so this hits the paths' difference.
        let num = u64::from(quotient % 4).wrapping_mul(div).wrapping_add(num_seed % div);
        check(num, mul, div);
    }
}
