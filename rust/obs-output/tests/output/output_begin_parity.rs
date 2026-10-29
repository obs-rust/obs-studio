//! Tier 3 parity: the shims of `obs_output::ffi::output_begin` (and the safe
//! core under them) against the original `libobs/obs-output-begin.c.h`,
//! compiled as `oracle_obs_output_begin_*` (obs-c-oracle,
//! oracle/output_begin.c).
//!
//! Every call runs the shim, the C and the safe core, and compares the
//! results exactly.
//!
//! Runs: exhaustive over frame rates, `fps_den` values and divisors at the
//! common values and the integer edges for the video interval, over sample
//! rates and frame sizes for the audio interval, every list of up to 4
//! intervals out of 9 (common, equal, far apart, near the doubling's wrap),
//! and every combination of the pairing gates; encoder sets of 1 to 6 video
//! and 1 to 6 audio encoders through the whole plan, C next to the shim; and
//! proptests over the full ranges.
//!
//! The C is undefined for an `fps_num`, sample rate or interval of 0, and on
//! MSVC x64 for an audio quotient above 64 bits. Those inputs are not given
//! to the C; there the shim is compared with the core, which gives the
//! intentional differences. A NULL interval array with a non-zero count is
//! not given to the C (it dereferences it); the shim answers 0.
//! `test_output_begin_steps` runs the C test against the C; the real-libobs
//! output tests run against the hybrid libobs, where these shims replace
//! the C.

use obs_c_oracle::output_begin::{
    oracle_obs_output_begin_audio_interval, oracle_obs_output_begin_batch_size,
    oracle_obs_output_begin_pair_audio, oracle_obs_output_begin_pair_video,
    oracle_obs_output_begin_video_interval,
};
use obs_output::ffi::output_begin::{
    obs_output_begin_audio_interval, obs_output_begin_batch_size, obs_output_begin_pair_audio,
    obs_output_begin_pair_video, obs_output_begin_video_interval,
};
use obs_output::output_begin::{self as core, NS_PER_SEC};
use proptest::prelude::*;

const FPS_NUMS: [u32; 12] = [
    0,
    1,
    24,
    25,
    30,
    60,
    120,
    30000,
    60000,
    1_000_000_000,
    0x8000_0000,
    u32::MAX,
];
const FPS_DENS: [u32; 9] = [
    0,
    1,
    2,
    1000,
    1001,
    65536,
    0x8000_0000,
    0x8000_0001,
    u32::MAX,
];
const DIVISORS: [u32; 8] = [0, 1, 2, 3, 4, 65536, 0x8000_0000, u32::MAX];
const RATES: [u32; 11] = [
    0,
    1,
    8000,
    16000,
    22050,
    44100,
    48000,
    96000,
    192_000,
    0x8000_0000,
    u32::MAX,
];
const FRAME_SIZES: [usize; 12] = [
    0,
    1,
    128,
    480,
    960,
    1024,
    2048,
    4096,
    1 << 33,
    1 << 40,
    1 << 60,
    usize::MAX,
];

// ---------------------------------------------------------------------------
// one call on each side

fn video_all(fps_num: u32, fps_den: u32, divisor: u32) -> u64 {
    let shim = obs_output_begin_video_interval(fps_num, fps_den, divisor);
    assert_eq!(
        shim,
        core::video_interval(fps_num, fps_den, divisor),
        "shim vs core: {fps_num}/{fps_den} / {divisor}"
    );
    if fps_num != 0 {
        // SAFETY: plain values; C is defined for a non-zero fps_num.
        let c = unsafe { oracle_obs_output_begin_video_interval(fps_num, fps_den, divisor) };
        assert_eq!(shim, c, "shim vs C: {fps_num}/{fps_den} / {divisor}");
    }
    shim
}

/// C is defined for this audio interval: a sample rate above 0 and, on
/// MSVC x64, a quotient that fits in 64 bits.
fn audio_defined(sample_rate: u32, frame_size: usize) -> bool {
    if sample_rate == 0 {
        return false;
    }
    if !obs_util::platform::MUL_DIV64_IS_UDIV128 {
        return true;
    }
    let exact = u128::from(NS_PER_SEC) * u128::try_from(frame_size).unwrap_or(u128::MAX)
        / u128::from(sample_rate);
    u64::try_from(exact).is_ok()
}

fn audio_all(sample_rate: u32, frame_size: usize) -> u64 {
    let shim = obs_output_begin_audio_interval(sample_rate, frame_size);
    assert_eq!(
        shim,
        core::audio_interval(sample_rate, frame_size),
        "shim vs core: {sample_rate} Hz, {frame_size}"
    );
    if audio_defined(sample_rate, frame_size) {
        // SAFETY: plain values; C is defined here.
        let c = unsafe { oracle_obs_output_begin_audio_interval(sample_rate, frame_size) };
        assert_eq!(shim, c, "shim vs C: {sample_rate} Hz, {frame_size}");
    }
    shim
}

fn batch_all(intervals: &[u64]) -> usize {
    // SAFETY: intervals.len() initialized values, read only.
    let shim = unsafe { obs_output_begin_batch_size(intervals.as_ptr(), intervals.len()) };
    assert_eq!(
        shim,
        core::batch_size(intervals),
        "shim vs core: {intervals:?}"
    );
    if !intervals.contains(&0) {
        // SAFETY: the same array; C is defined without a 0 interval.
        let c = unsafe { oracle_obs_output_begin_batch_size(intervals.as_ptr(), intervals.len()) };
        assert_eq!(shim, c, "shim vs C: {intervals:?}");
    }
    shim
}

// ---------------------------------------------------------------------------
// exhaustive

#[test]
fn video_interval_every_value() {
    let mut defined = 0;
    for fps_num in FPS_NUMS {
        for fps_den in FPS_DENS {
            for divisor in DIVISORS {
                video_all(fps_num, fps_den, divisor);
                defined += u32::from(fps_num != 0);
            }
        }
    }
    assert_eq!(defined, 11 * 9 * 8);
}

#[test]
fn audio_interval_every_value() {
    for sample_rate in RATES {
        for frame_size in FRAME_SIZES {
            audio_all(sample_rate, frame_size);
        }
    }
}

#[test]
fn batch_every_short_list() {
    const VALUES: [u64; 9] = [
        1,
        20_000_000,
        21_333_333,
        33_333_333,
        33_366_666,
        1_000_000_000,
        0x7fff_ffff_ffff_ffff,
        0x8000_0000_0000_0000,
        u64::MAX,
    ];
    batch_all(&[]);
    let mut n = 1;
    for len in 1..=4 {
        let count = VALUES.len().pow(len);
        for code in 0..count {
            let mut rest = code;
            let list: Vec<u64> = (0..len)
                .map(|_| {
                    let v = VALUES[rest % VALUES.len()];
                    rest /= VALUES.len();
                    v
                })
                .collect();
            batch_all(&list);
            n += 1;
        }
    }
    assert_eq!(n, 1 + 9 + 81 + 729 + 6561);
}

#[test]
fn pair_gates_every_value() {
    for active in [false, true] {
        // SAFETY: plain values.
        let c = unsafe { oracle_obs_output_begin_pair_video(active) };
        assert_eq!(obs_output_begin_pair_video(active), c);
        assert_eq!(core::pair_video(active), c);
        for paired in [0, 1, 2, 3, usize::MAX] {
            // SAFETY: plain values.
            let c = unsafe { oracle_obs_output_begin_pair_audio(active, paired) };
            assert_eq!(obs_output_begin_pair_audio(active, paired), c);
            assert_eq!(core::pair_audio(active, paired), c);
        }
    }
}

// ---------------------------------------------------------------------------
// the whole plan: an output's encoders

/// `calculate_batch_size()` on both sides: video encoders first, then audio,
/// as the C walks the slots.
fn plan_all(fps_num: u32, fps_den: u32, divisors: &[u32], audio: &[(u32, usize)]) -> usize {
    let mut intervals: Vec<u64> = divisors
        .iter()
        .map(|&d| video_all(fps_num, fps_den, d))
        .collect();
    intervals.extend(audio.iter().map(|&(rate, size)| audio_all(rate, size)));
    batch_all(&intervals)
}

#[test]
fn plan_of_common_outputs() {
    // stream: 30 fps + AAC
    assert_eq!(plan_all(30, 1, &[1], &[(48000, 1024)]), 5);
    // recording with six AAC tracks at 29.97 fps
    assert_eq!(plan_all(30000, 1001, &[1], &[(48000, 1024); 6]), 2 + 6 * 3);
    // multitrack video: 60 fps, 30 fps, 30 fps, Opus
    assert_eq!(plan_all(60, 1, &[1, 2, 2], &[(48000, 960)]), 4 + 2 + 2 + 3);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn plan_random_outputs(
        fps in prop_oneof![
            Just((30u32, 1u32)), Just((30000, 1001)), Just((60, 1)), Just((25, 1)),
            (1u32..240, 1u32..4),
        ],
        divisors in prop::collection::vec(1u32..=6, 1..=6),
        audio in prop::collection::vec(
            (prop_oneof![Just(48000u32), Just(44100), 8000u32..192_000],
             prop_oneof![Just(1024usize), Just(960), 1usize..8192]),
            1..=6,
        ),
    ) {
        let b = plan_all(fps.0, fps.1, &divisors, &audio);
        prop_assert!(b >= 2 * (divisors.len() + audio.len()));
    }

    #[test]
    fn intervals_full_range(
        fps_num in any::<u32>(), fps_den in any::<u32>(), divisor in any::<u32>(),
        sample_rate in any::<u32>(), frame_size in any::<usize>(),
        small_size in 0usize..(1 << 32),
    ) {
        video_all(fps_num, fps_den, divisor);
        audio_all(sample_rate, frame_size);
        audio_all(sample_rate, small_size);
    }

    #[test]
    fn batch_full_range(intervals in prop::collection::vec(any::<u64>(), 0..10)) {
        batch_all(&intervals);
    }

    #[test]
    fn batch_without_wrap(intervals in prop::collection::vec(1u64..1_000_000_000_000, 0..10)) {
        batch_all(&intervals);
    }
}

#[test]
fn null_intervals_give_zero() {
    // SAFETY: NULL with a non-zero count is the documented answer 0; the C
    // is not called with it.
    assert_eq!(
        unsafe { obs_output_begin_batch_size(std::ptr::null(), 3) },
        0
    );
    // SAFETY: NULL with a count of 0 is allowed on both sides.
    let c = unsafe { oracle_obs_output_begin_batch_size(std::ptr::null(), 0) };
    // SAFETY: as above.
    assert_eq!(
        unsafe { obs_output_begin_batch_size(std::ptr::null(), 0) },
        c
    );
}
