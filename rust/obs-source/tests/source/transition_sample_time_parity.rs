//! Tier 3: the `obs_source_transition_sample_time` shim against the
//! original C (`obs-c-oracle`, `oracle/source_transition_sample_time.c`: the
//! unmodified `libobs/obs-source-transition-sample-time.c.h`, renamed and
//! built twice).
//!
//! Every call compares, bit for bit:
//! - the shim with the C over the Rust clock
//!   (`oracle_obs_source_transition_sample_time_rust_clock`), on every
//!   host: the clock is the same on both sides, so this compares the moved
//!   lines, the offset and the wrapping sum;
//! - the C over the C clock (`oracle_obs_source_transition_sample_time`,
//!   as libobs-c-reference composes them) with the C clock at the block's
//!   time plus the core's [`offset`], on every host: the C's offset is the
//!   core's;
//! - the shim with the C over the C clock where the C's `long double` is
//!   the x87's 80 bits (x86_64 Linux and macOS). Elsewhere the clock's
//!   ratio can differ in the last bit near an `f32` rounding tie, as those
//!   platforms' libobs already differ from x86_64's; `transition_clock_parity`
//!   covers that.
//!
//! Runs every sample of a block at 18 rates from 1 Hz to `u32::MAX` Hz over
//! clocks and timestamps before, across and after the transition, with a
//! sum that wraps, every mode and special manual values; the floor at every
//! sample of 3, 7, 44100 and 88200 Hz around the start; and proptests over
//! any clock, timestamp and rate with a sample in the block or below 2^34
//! (where `util_mul_div64()`'s per-platform forms all agree, and none
//! faults); and samples past 2^34, where the forms differ, against each
//! host's own C (where the oracle is built with MSVC, only where its
//! `_udiv128` does not fault; built with clang-cl, every quotient, cut to 64
//! bits).
//!
//! Intentional difference, excluded from the comparison and checked on the
//! shim only: a sample rate of 0. The C divides by zero; the shim gives
//! every sample the block's own time.

use obs_c_oracle::parity::cases;
use obs_c_oracle::source_transition_clock::{
    LDBL_MANT_DIG_INDEX, oracle_obs_source_transition_clock_time,
    oracle_source_transition_clock_layout,
};
use obs_c_oracle::source_transition_sample_time::{
    oracle_obs_source_transition_sample_time, oracle_obs_source_transition_sample_time_rust_clock,
    oracle_source_transition_sample_time_mul_div_faults,
};
use obs_source::ffi::source_transition_clock::obs_source_transition_clock_time;
use obs_source::ffi::source_transition_sample_time::obs_source_transition_sample_time;
use obs_source::transition_clock::MODE_MANUAL;
use obs_source::transition_sample_time::offset;
use proptest::prelude::*;

/// `AUDIO_OUTPUT_FRAMES`
const FRAMES: usize = 1024;
const AUTO: i32 = 0;
const TWO_25: u64 = 1 << 25;
const TWO_34: usize = 1 << 34;

const RATES: [usize; 18] = [
    1,
    2,
    3,
    7,
    8000,
    11025,
    16000,
    22050,
    32000,
    44100,
    48000,
    88200,
    96000,
    192_000,
    384_000,
    1_000_000_000,
    1_000_000_001,
    4_294_967_295,
];

const MODES: [i32; 6] = [AUTO, MODE_MANUAL, 2, 7, -1, i32::MIN];

const MANUAL_BITS: [u32; 7] = [
    0x0000_0000,
    0x8000_0000,
    0x3f00_0000,
    0x3f80_0000,
    0x7f80_0000,
    0x7fc0_0001,
    0x0000_0001,
];

#[derive(Debug, Clone, Copy)]
struct Call {
    mode: i32,
    manual_bits: u32,
    start_time: u64,
    duration: u64,
    ts: u64,
    sample_rate: usize,
    sample: usize,
}

/// Whether the oracle's `long double` is the x87's, as the Rust clock's.
fn x87_long_double() -> bool {
    // SAFETY: a pure lookup.
    unsafe { oracle_source_transition_clock_layout(LDBL_MANT_DIG_INDEX) == 64 }
}

fn shim(c: Call) -> u32 {
    obs_source_transition_sample_time(
        c.mode,
        f32::from_bits(c.manual_bits),
        c.start_time,
        c.duration,
        c.ts,
        c.sample_rate,
        c.sample,
    )
    .to_bits()
}

/// Compares one call. Never with a rate of 0: the C divides by zero.
#[track_caller]
fn check(c: Call, x87: bool) {
    assert_ne!(c.sample_rate, 0, "the C divides by zero");
    let manual_val = f32::from_bits(c.manual_bits);
    let got = shim(c);
    // SAFETY: plain values, and a rate other than 0.
    let (c_rust_clock, c_c_clock, c_clock_at_offset) = unsafe {
        (
            oracle_obs_source_transition_sample_time_rust_clock(
                c.mode,
                manual_val,
                c.start_time,
                c.duration,
                c.ts,
                c.sample_rate,
                c.sample,
            )
            .to_bits(),
            oracle_obs_source_transition_sample_time(
                c.mode,
                manual_val,
                c.start_time,
                c.duration,
                c.ts,
                c.sample_rate,
                c.sample,
            )
            .to_bits(),
            oracle_obs_source_transition_clock_time(
                c.mode,
                manual_val,
                c.start_time,
                c.duration,
                c.ts.wrapping_add(offset(c.sample_rate, c.sample)),
            )
            .to_bits(),
        )
    };
    assert_eq!(got, c_rust_clock, "shim vs C over the Rust clock: {c:?}");
    assert_eq!(
        c_c_clock, c_clock_at_offset,
        "C vs the C clock at the core's offset: {c:?}"
    );
    if x87 {
        assert_eq!(got, c_c_clock, "shim vs C: {c:?}");
    }
}

#[test]
fn parity_every_sample_of_a_block() {
    let x87 = x87_long_double();
    let start: u64 = 5_000_000_000;
    // (ts, start, duration): before, from the start, across the end, after
    // it, a duration of 0, one longer than any block, and a sum that wraps
    let blocks = [
        (start - 3_000_000_000, start, 900_000_000),
        (start, start, 900_000_000),
        (start + 900_000_000 - 10_000_000, start, 900_000_000),
        (start + 2_000_000_000, start, 900_000_000),
        (start, start, 0),
        (start - 1, start, u64::MAX),
        (u64::MAX - 10_000_000, 0, TWO_25),
    ];
    let mut n = 0_usize;
    for rate in RATES {
        for (ts, start_time, duration) in blocks {
            for sample in 0..FRAMES {
                check(
                    Call {
                        mode: AUTO,
                        manual_bits: 0x3f00_0000,
                        start_time,
                        duration,
                        ts,
                        sample_rate: rate,
                        sample,
                    },
                    x87,
                );
                n += 1;
            }
        }
    }
    assert_eq!(n, RATES.len() * blocks.len() * FRAMES);
}

#[test]
fn parity_every_mode_and_manual_value() {
    let x87 = x87_long_double();
    for mode in MODES {
        for manual_bits in MANUAL_BITS {
            for sample in (0..FRAMES).step_by(17).chain([FRAMES - 1]) {
                for rate in [44100, 48000, 3] {
                    check(
                        Call {
                            mode,
                            manual_bits,
                            start_time: 1000,
                            duration: TWO_25,
                            ts: 1000,
                            sample_rate: rate,
                            sample,
                        },
                        x87,
                    );
                }
            }
        }
    }
}

/// The floor: for every sample, a start one nanosecond either side of the
/// sample's time.
#[test]
fn parity_floor_around_the_start() {
    let x87 = x87_long_double();
    for rate in [3, 7, 44100, 88200] {
        for sample in 0..FRAMES {
            let at = offset(rate, sample);
            for start_time in [at.saturating_sub(1), at, at + 1] {
                check(
                    Call {
                        mode: AUTO,
                        manual_bits: 0,
                        start_time,
                        duration: TWO_25,
                        ts: 0,
                        sample_rate: rate,
                        sample,
                    },
                    x87,
                );
            }
        }
    }
}

/// Samples past 2^34, where the split formula of `util_mul_div64()` wraps
/// and the exact one does not: each host's C against the shim. Where the
/// oracle uses `_udiv128` (MSVC on x64), only quotients that fit in 64 bits:
/// past that the C faults. clang-cl's exact quotient is cut to 64 bits, and
/// every input is compared.
#[test]
fn parity_huge_samples() {
    let x87 = x87_long_double();
    // SAFETY: a pure lookup.
    let faults = unsafe { oracle_source_transition_sample_time_mul_div_faults() } != 0;
    let huge = [TWO_34, TWO_34 + 1, (1 << 40) + 7, 1 << 63, usize::MAX];
    let mut n = 0_usize;
    for sample in huge {
        for rate in RATES.into_iter().chain([1 << 40, usize::MAX]) {
            let exact =
                u128::try_from(sample).unwrap() * 1_000_000_000 / u128::try_from(rate).unwrap();
            if faults && u64::try_from(exact).is_err() {
                continue;
            }
            check(
                Call {
                    mode: AUTO,
                    manual_bits: 0,
                    start_time: 0,
                    duration: u64::MAX,
                    ts: 1000,
                    sample_rate: rate,
                    sample,
                },
                x87,
            );
            n += 1;
        }
    }
    assert!(n >= 30, "{n} huge samples compared");
}

/// The intentional difference, on the shim only: a rate of 0 is the
/// block's own time for every sample.
#[test]
fn shim_zero_rate_takes_the_block_time() {
    for ts in [0, 999, 1000, 1001, 1000 + TWO_25, u64::MAX] {
        for sample in [0, 1, 511, FRAMES - 1] {
            let c = Call {
                mode: AUTO,
                manual_bits: 0,
                start_time: 1000,
                duration: TWO_25,
                ts,
                sample_rate: 0,
                sample,
            };
            assert_eq!(
                shim(c),
                obs_source_transition_clock_time(AUTO, 0.0, 1000, TWO_25, ts).to_bits(),
                "{c:?}"
            );
        }
    }
}

fn mode() -> impl Strategy<Value = i32> {
    prop_oneof![Just(AUTO), Just(MODE_MANUAL), any::<i32>()]
}

fn duration() -> impl Strategy<Value = u64> {
    prop_oneof![Just(0_u64), 1_u64..(1 << 40), any::<u64>()]
}

fn rate() -> impl Strategy<Value = usize> {
    prop_oneof![
        Just(44100_usize),
        Just(48000),
        1_usize..400_000,
        1_usize..=4_294_967_295,
        1_usize..,
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(4000)))]

    /// Any clock, timestamp and rate, a sample in the block.
    #[test]
    fn proptest_block(
        mode in mode(),
        manual_bits in any::<u32>(),
        start_time in any::<u64>(),
        duration in duration(),
        ts in any::<u64>(),
        sample_rate in rate(),
        sample in 0..FRAMES,
    ) {
        check(
            Call { mode, manual_bits, start_time, duration, ts, sample_rate, sample },
            x87_long_double(),
        );
    }

    /// A clock near the block, so the ratio is neither 0 nor 1, and samples
    /// up to 2^34.
    #[test]
    fn proptest_near_the_block(
        start_time in 0_u64..(1 << 50),
        duration in 1_u64..(1 << 45),
        back in 0_u64..(1 << 40),
        sample_rate in rate(),
        sample in prop_oneof![0..FRAMES, 0..TWO_34],
    ) {
        check(
            Call {
                mode: AUTO,
                manual_bits: 0,
                start_time,
                duration,
                ts: start_time.saturating_sub(back),
                sample_rate,
                sample,
            },
            x87_long_double(),
        );
    }
}
