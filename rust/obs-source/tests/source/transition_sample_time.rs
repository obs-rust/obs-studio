//! Tier 1: `obs_source::transition_sample_time`, the time of each audio
//! sample a transition mixes.
//!
//! The `steps_*` cases port `test/cmocka/test_transition_sample_time_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations, the "characterized, not endorsed" cases included.
//!
//! The `edge_*` cases are new: a sample rate of 0 (the intentional
//! difference: an offset of 0 where the C divides by zero), the offset
//! against the exact quotient and against this host's `util_mul_div64`
//! formula for any sample and rate, and the sample time against the clock
//! at the wrapped sum for any clock.

use obs_c_oracle::parity::cases;
use obs_source::transition_clock::{Clock, MODE_MANUAL, time};
use obs_source::transition_sample_time::{offset, sample_time};
use proptest::prelude::*;

const AUTO: i32 = 0;
const MANUAL: i32 = MODE_MANUAL;

/// `AUDIO_OUTPUT_FRAMES`
const FRAMES: usize = 1024;
const TWO_25: u64 = 1 << 25;
const TWO_25_F: f32 = 33_554_432.0;

#[track_caller]
fn check_bits(actual: f32, expected: f32, what: &str) {
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "{what} is {actual:e} ({:#010x}), expected {expected:e} ({:#010x})",
        actual.to_bits(),
        expected.to_bits()
    );
}

fn clock(mode: i32, manual_val: f32, start_time: u64, duration: u64) -> Clock {
    Clock {
        mode,
        manual_val,
        start_time,
        duration,
    }
}

/// `obs_source_transition_sample_time()` with the C's argument order.
fn st(
    mode: i32,
    manual_val: f32,
    start_time: u64,
    duration: u64,
    ts: u64,
    sample_rate: usize,
    sample: usize,
) -> f32 {
    sample_time(
        clock(mode, manual_val, start_time, duration),
        ts,
        sample_rate,
        sample,
    )
}

/// `obs_source_transition_clock_time()` with the C's argument order.
fn clock_time(mode: i32, manual_val: f32, start_time: u64, duration: u64, ts: u64) -> f32 {
    time(clock(mode, manual_val, start_time, duration), ts)
}

/// The exact quotient `sample * 10^9 / rate`, saturated to `u64`.
fn exact_offset(sample_rate: usize, sample: usize) -> u64 {
    let quotient =
        u128::try_from(sample).unwrap() * 1_000_000_000 / u128::try_from(sample_rate).unwrap();
    u64::try_from(quotient).unwrap_or(u64::MAX)
}

/// The cmocka test's `expected()`: the clock at the block's time plus the
/// exact offset.
fn expected(
    mode: i32,
    manual_val: f32,
    start_time: u64,
    duration: u64,
    ts: u64,
    sample_rate: usize,
    sample: usize,
) -> f32 {
    clock_time(
        mode,
        manual_val,
        start_time,
        duration,
        ts.wrapping_add(exact_offset(sample_rate, sample)),
    )
}

const RATES: [usize; 14] = [
    1, 2, 3, 7, 8000, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 192_000,
];

#[test]
fn steps_sample_zero_is_the_block_time() {
    let ts_values = [0, 1, 999, 1000, 1001, 5000, 1000 + TWO_25, u64::MAX];

    for rate in RATES {
        for ts in ts_values {
            check_bits(
                st(AUTO, 0.25, 1000, TWO_25, ts, rate, 0),
                clock_time(AUTO, 0.25, 1000, TWO_25, ts),
                "auto",
            );
            check_bits(st(MANUAL, 0.25, 1000, TWO_25, ts, rate, 0), 0.25, "manual");
        }
    }
}

#[test]
fn steps_every_sample_of_a_block() {
    let ts = 5_000_000;
    let start = 5_000_000 + 7_000_000;
    let duration = 900_000_000;

    for rate in RATES {
        for sample in 0..FRAMES {
            check_bits(
                st(AUTO, 0.0, start, duration, ts, rate, sample),
                expected(AUTO, 0.0, start, duration, ts, rate, sample),
                "block",
            );
        }
    }
}

#[test]
fn steps_last_sample_of_a_block() {
    assert_eq!(
        st(AUTO, 0.0, 1000, TWO_25, 1000, 48000, FRAMES - 1),
        21_312_500.0 / TWO_25_F
    );
    assert_eq!(
        st(AUTO, 0.0, 1000, TWO_25, 1000, 44100, FRAMES - 1),
        23_197_278.0 / TWO_25_F
    );
    assert_eq!(
        st(AUTO, 0.0, 1000, TWO_25, 1000, 96000, FRAMES - 1),
        10_656_250.0 / TWO_25_F
    );
}

#[test]
fn steps_offset_is_floored() {
    assert_eq!(st(AUTO, 0.0, 22675, TWO_25, 0, 44100, 1), 0.0);
    assert_eq!(st(AUTO, 0.0, 22674, TWO_25, 0, 44100, 1), 1.0 / TWO_25_F);

    assert_eq!(st(AUTO, 0.0, 333_333_333, TWO_25, 0, 3, 1), 0.0);
    assert_eq!(st(AUTO, 0.0, 333_333_332, TWO_25, 0, 3, 1), 1.0 / TWO_25_F);
    assert_eq!(st(AUTO, 0.0, 666_666_666, TWO_25, 0, 3, 2), 0.0);
    assert_eq!(st(AUTO, 0.0, 666_666_665, TWO_25, 0, 3, 2), 1.0 / TWO_25_F);

    assert_eq!(
        st(AUTO, 0.0, 146_142_857_142, TWO_25, 0, 7, FRAMES - 1),
        0.0
    );
    assert_eq!(
        st(AUTO, 0.0, 146_142_857_141, TWO_25, 0, 7, FRAMES - 1),
        1.0 / TWO_25_F
    );
}

#[test]
fn steps_manual_mode_does_not_move() {
    let manual_bits: [u32; 9] = [
        0x0000_0000,
        0x8000_0000,
        0x3e80_0000,
        0x3f80_0000,
        0x7f80_0000,
        0x7fc0_0001,
        0xff80_0000,
        0x0000_0001,
        0x4b00_0000,
    ];

    for bits in manual_bits {
        let manual_val = f32::from_bits(bits);
        for sample in (0..FRAMES).step_by(31) {
            check_bits(
                st(MANUAL, manual_val, 0, TWO_25, 1000, 48000, sample),
                manual_val,
                "manual",
            );
        }
    }
}

#[test]
fn steps_other_modes_are_automatic() {
    // characterized, not endorsed for the values outside the enum
    for mode in [AUTO, 2, 7, -1] {
        assert_eq!(st(mode, 0.75, 0, TWO_25, 0, 48000, 0), 0.0);
        assert_eq!(
            st(mode, 0.75, 0, TWO_25, 0, 48000, FRAMES - 1),
            21_312_500.0 / TWO_25_F
        );
    }
}

#[test]
fn steps_block_before_straddling_and_after() {
    let span: u64 = 21_312_500;
    let start: u64 = 1_000_000_000;
    let duration = TWO_25;

    for sample in 0..FRAMES {
        check_bits(
            st(AUTO, 0.0, start, duration, start - span, 48000, sample),
            0.0,
            "before",
        );
    }

    let ts = start + duration - span / 2;
    let mut first_one = FRAMES;
    for sample in 0..FRAMES {
        let t = st(AUTO, 0.0, start, duration, ts, 48000, sample);
        if t == 1.0 && first_one == FRAMES {
            first_one = sample;
        }
        if first_one == FRAMES {
            assert!(t < 1.0, "sample {sample}: {t}");
        } else {
            assert_eq!(t, 1.0, "sample {sample}");
        }
    }
    assert_eq!(first_one, 512);
    // 33544015 rounds to an even f32, as the C's (float) does
    assert_eq!(
        st(AUTO, 0.0, start, duration, ts, 48000, 511),
        33_544_015.0 / TWO_25_F
    );

    for sample in 0..FRAMES {
        check_bits(
            st(AUTO, 0.0, start, duration, start + duration, 48000, sample),
            1.0,
            "after",
        );
    }

    assert_eq!(st(AUTO, 0.0, start, 0, start, 48000, 0), 0.0);
    assert_eq!(st(AUTO, 0.0, start, 0, start, 48000, 1), 1.0);
}

#[test]
fn steps_timestamp_sum_wraps() {
    // characterized, not endorsed
    assert_eq!(
        st(AUTO, 0.0, 0, TWO_25, u64::MAX - 10, 48000, 1),
        20822.0 / TWO_25_F
    );
    assert_eq!(st(AUTO, 0.0, 0, TWO_25, u64::MAX - 10, 48000, 0), 1.0);
    assert_eq!(st(AUTO, 0.0, 0, TWO_25, u64::MAX - 20833, 48000, 1), 1.0);
    assert_eq!(st(AUTO, 0.0, 0, TWO_25, u64::MAX - 20832, 48000, 1), 0.0);
}

#[test]
fn steps_slow_rates() {
    assert_eq!(
        st(AUTO, 0.0, 0, 2_048_000_000_000, 0, 1, FRAMES - 1),
        1023.0 / 2048.0
    );
    assert_eq!(
        st(AUTO, 0.0, 0, 2_048_000_000_000, 0, 2, FRAMES - 1),
        511.5 / 2048.0
    );
}

/// The intentional difference: a sample rate of 0 gives an offset of 0,
/// where the C divides by zero.
#[test]
fn edge_zero_rate_takes_the_block_time() {
    for sample in [0, 1, 2, 511, FRAMES - 1, FRAMES, usize::MAX] {
        assert_eq!(offset(0, sample), 0, "sample {sample}");
    }
    for ts in [0, 1, 1000, 5000, u64::MAX] {
        for sample in [0, 1, FRAMES - 1] {
            check_bits(
                st(AUTO, 0.0, 1000, TWO_25, ts, 0, sample),
                clock_time(AUTO, 0.0, 1000, TWO_25, ts),
                "rate 0",
            );
            check_bits(st(MANUAL, 0.5, 1000, TWO_25, ts, 0, sample), 0.5, "rate 0");
        }
    }
}

#[test]
fn edge_offset_of_sample_zero_and_one_second() {
    for rate in RATES {
        assert_eq!(offset(rate, 0), 0);
        assert_eq!(offset(rate, rate), 1_000_000_000, "rate {rate}");
    }
    assert_eq!(offset(1, 1), 1_000_000_000);
    assert_eq!(offset(1_000_000_000, 1), 1);
    assert_eq!(offset(1_000_000_001, 1), 0);
    assert_eq!(offset(usize::MAX, 1), 0);
}

/// `util_mul_div64(sample, 10^9, rate)` as this host's C computes it.
fn host_mul_div64(sample: usize, sample_rate: usize) -> u64 {
    let num = u64::try_from(sample).unwrap();
    let div = u64::try_from(sample_rate).unwrap();
    if obs_util::platform::MUL_DIV64_IS_UDIV128 {
        // clang-cl: the exact quotient cut to its low 64 bits (MSVC faults)
        let quotient =
            u128::try_from(sample).unwrap() * 1_000_000_000 / u128::try_from(sample_rate).unwrap();
        u64::try_from(quotient & u128::from(u64::MAX)).unwrap()
    } else {
        let rem = num % div;
        (num / div)
            .wrapping_mul(1_000_000_000)
            .wrapping_add(rem.wrapping_mul(1_000_000_000) / div)
    }
}

/// Past 2^34 samples the split formula wraps and differs from the exact
/// quotient; the offset follows the host's formula (on MSVC x64 the exact
/// quotient cut to 64 bits, as clang-cl's cast does; MSVC's `_udiv128`
/// faults there).
#[test]
fn edge_offset_of_huge_samples() {
    let huge = [1_usize << 34, (1 << 40) + 7, usize::MAX];
    for sample in huge {
        for rate in [1, 3, 48000, 1 << 40, usize::MAX] {
            assert_eq!(
                offset(rate, sample),
                host_mul_div64(sample, rate),
                "rate {rate} sample {sample}"
            );
        }
    }
}

/// A quotient past 64 bits: `usize::MAX` samples at 1 Hz. Every host's C
/// gives the same offset, `2^64 - 10^9` (the split formula wraps, clang-cl
/// cuts the exact quotient to 64 bits), so a block at 10^9 ns lands on 0,
/// the start; MSVC's `_udiv128` faults instead.
#[test]
fn edge_offset_past_64_bits_wraps_on_every_host() {
    assert_eq!(offset(1, usize::MAX), 0_u64.wrapping_sub(1_000_000_000));
    assert_eq!(
        st(AUTO, 0.0, 0, 1_000_000_000, 1_000_000_000, 1, usize::MAX),
        0.0
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(2000)))]

    /// Under 2^34 samples the offset is the exact floored quotient.
    #[test]
    fn edge_offset_is_the_exact_quotient(
        sample in 0_usize..(1 << 34),
        sample_rate in 1_usize..,
    ) {
        prop_assert_eq!(offset(sample_rate, sample), exact_offset(sample_rate, sample));
    }

    /// For any sample and rate other than 0, the offset is the host's
    /// `util_mul_div64()`.
    #[test]
    fn edge_offset_is_the_hosts_mul_div64(sample in any::<usize>(), sample_rate in 1_usize..) {
        prop_assert_eq!(offset(sample_rate, sample), host_mul_div64(sample, sample_rate));
    }

    /// The sample time is the clock at the wrapped sum of the block's time
    /// and the offset, for any clock and a sample in the block.
    #[test]
    fn edge_sample_time_is_the_clock_at_the_offset(
        mode in prop_oneof![Just(AUTO), Just(MANUAL), any::<i32>()],
        manual_bits in any::<u32>(),
        start_time in any::<u64>(),
        duration in prop_oneof![Just(0_u64), 1_u64..(1 << 40), any::<u64>()],
        ts in any::<u64>(),
        sample_rate in prop_oneof![Just(0_usize), Just(48000), Just(44100), 1_usize..],
        sample in 0..FRAMES,
    ) {
        let c = clock(mode, f32::from_bits(manual_bits), start_time, duration);
        let want = if sample_rate == 0 {
            time(c, ts)
        } else {
            time(c, ts.wrapping_add(exact_offset(sample_rate, sample)))
        };
        prop_assert_eq!(sample_time(c, ts, sample_rate, sample).to_bits(), want.to_bits());
    }

    /// In auto mode, with no wrap, the time never goes back within a block.
    #[test]
    fn edge_sample_time_never_goes_back(
        start_time in 0_u64..(1 << 40),
        duration in 0_u64..(1 << 36),
        ts in 0_u64..(1 << 40),
        sample_rate in 1_usize..400_000,
    ) {
        let c = clock(AUTO, 0.0, start_time, duration);
        let mut last = sample_time(c, ts, sample_rate, 0);
        for sample in (1..FRAMES).step_by(7) {
            let t = sample_time(c, ts, sample_rate, sample);
            prop_assert!(t >= last, "sample {}: {} after {}", sample, t, last);
            last = t;
        }
    }
}
