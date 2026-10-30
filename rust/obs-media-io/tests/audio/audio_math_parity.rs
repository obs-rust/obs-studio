//! Tier 3: the safe cores of the `static inline` helpers in
//! `libobs/media-io/audio-math.h` behave exactly like the header functions,
//! compiled as non-inline oracle wrappers.
//!
//! No intentional differences from C. The C is scalar, so per the Phase 5
//! rule results are compared bit for bit (any NaN equals any NaN), with no
//! ULP tolerance. Both sides call the same platform `log10f`/`powf`, so the
//! last bit agrees even where glibc, the UCRT and libSystem differ from
//! each other. A mismatch on one OS most likely means a compiler rewrote
//! `powf(10, x)` into `exp10f` on one side only.
//!
//! Inputs: special values (zeros, infinities, NaNs, subnormals, the float
//! extremes, the cmocka test's values), every 4096th bit pattern, and
//! proptest over arbitrary bits and over the ranges libobs passes.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use obs_c_oracle::audio_math as c;
use obs_media_io::audio_math::{db_to_mul, mul_to_db};
use proptest::prelude::*;

/// Bit-exact equality, except that any NaN equals any NaN.
fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn check(x: f32) -> Result<(), TestCaseError> {
    // SAFETY: the oracle wrappers take any float.
    let (c_db, c_mul) = unsafe { (c::oracle_mul_to_db(x), c::oracle_db_to_mul(x)) };
    let db = mul_to_db(x);
    prop_assert!(
        same(db, c_db),
        "mul_to_db({x:e} = {:#x}): Rust {db:e} != C {c_db:e}",
        x.to_bits()
    );
    let mul = db_to_mul(x);
    prop_assert!(
        same(mul, c_mul),
        "db_to_mul({x:e} = {:#x}): Rust {mul:e} != C {c_mul:e}",
        x.to_bits()
    );
    Ok(())
}

fn special() -> Vec<f32> {
    let mut v = vec![
        0.0,
        1.0,
        10.0,
        0.1,
        0.5,
        2.0,
        0.001,
        0.25,
        6.0,
        12.0,
        20.0,
        60.0,
        96.0,
        770.0,
        890.0,
        1000.0,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::from_bits(0x007f_ffff),
        f32::MAX,
        f32::EPSILON,
        1.0 - f32::EPSILON / 2.0,
        1.0 + f32::EPSILON,
        f32::INFINITY,
        f32::NAN,
        f32::from_bits(0x7f80_0001),
        f32::from_bits(0x7fff_ffff),
    ];
    v.extend(v.clone().into_iter().map(|x| -x));
    v
}

#[test]
fn special_values_match_c_oracle() {
    for x in special() {
        check(x).unwrap();
    }
}

#[test]
fn bit_pattern_sweep_matches_c_oracle() {
    for bits in (0..=u32::MAX).step_by(4096) {
        check(f32::from_bits(bits)).unwrap();
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn any_bits_match_c_oracle(bits in any::<u32>()) {
        check(f32::from_bits(bits))?;
    }

    /// Multipliers and dB values in the range libobs uses (volume faders,
    /// meters, gain filters).
    #[test]
    fn audio_range_matches_c_oracle(x in prop_oneof![0.0f32..=4.0, -200.0f32..=40.0]) {
        check(x)?;
    }
}
