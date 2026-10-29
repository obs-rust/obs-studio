//! Tier 1: `obs_output::output_begin`, the begin-capture plan of
//! `obs-output.c`.
//!
//! The `steps_*` cases port `test/cmocka/test_output_begin_steps.c` 1:1: its
//! 11 cases under the same names, in the same order, with the same values
//! and expectations, the "characterized, not endorsed" ones included. The C
//! calls the hidden `obs_output_begin_*` functions; here the safe core is
//! called directly.
//!
//! The `edge_*` cases are new: proptests of the intervals against the exact
//! quotient in `u128` (where the C's formula is exact), of the batch size
//! against a model written from the C's expressions, its invariants, any
//! input without a panic, and the intentional differences where C is
//! undefined.

#![allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use obs_output::output_begin::{
    NS_PER_SEC, audio_interval, batch_size, mul_div64, pair_audio, pair_video, video_interval,
};
use proptest::prelude::*;

/// `fps_den` of 29.97 fps (`fps_num` 30000).
const NTSC_DEN: u32 = 1001;

// ---------------------------------------------------------------------------
// step 1: the intervals

#[test]
fn steps_video_interval_common_rates() {
    assert_eq!(video_interval(30, 1, 1), 33_333_333, "30 fps");
    assert_eq!(video_interval(30000, NTSC_DEN, 1), 33_366_666, "29.97 fps");
    assert_eq!(video_interval(25, 1, 1), 40_000_000, "25 fps");
    assert_eq!(video_interval(60, 1, 1), 16_666_666, "60 fps");
    assert_eq!(video_interval(60000, NTSC_DEN, 1), 16_683_333, "59.94 fps");
    assert_eq!(video_interval(24, 1, 1), 41_666_666, "24 fps");
}

#[test]
fn steps_video_interval_with_divisor() {
    assert_eq!(video_interval(60, 1, 2), 33_333_333, "60 fps / 2");
    assert_eq!(video_interval(60, 1, 3), 50_000_000, "60 fps / 3");
    assert_eq!(video_interval(30, 1, 2), 66_666_666, "30 fps / 2");
    assert_eq!(
        video_interval(60000, NTSC_DEN, 2),
        33_366_666,
        "59.94 fps / 2"
    );
}

#[test]
fn steps_video_interval_at_the_limits() {
    // characterized, not endorsed: fps_den * divisor wraps as uint32_t
    assert_eq!(
        video_interval(30, 65536, 65536),
        0,
        "65536 * 65536 wraps to 0"
    );
    assert_eq!(
        video_interval(30, 0x8000_0000, 2),
        0,
        "0x80000000 * 2 wraps to 0"
    );
    assert_eq!(video_interval(30, 0x8000_0001, 2), 66_666_666, "wraps to 2");
    assert_eq!(
        video_interval(1, u32::MAX, 1),
        4_294_967_295_000_000_000,
        "the largest product"
    );
    assert_eq!(video_interval(u32::MAX, 1, 1), 0, "the largest fps_num");
    assert_eq!(
        video_interval(1_000_000_000, 1, 1),
        1,
        "fps_num of a billion"
    );
}

#[test]
fn steps_audio_interval_common_rates() {
    assert_eq!(audio_interval(48000, 1024), 21_333_333, "AAC 48 kHz");
    assert_eq!(audio_interval(44100, 1024), 23_219_954, "AAC 44.1 kHz");
    assert_eq!(audio_interval(48000, 960), 20_000_000, "Opus 48 kHz");
    assert_eq!(audio_interval(96000, 1024), 10_666_666, "AAC 96 kHz");
    assert_eq!(audio_interval(48000, 1), 20833, "one sample at 48 kHz");
}

#[test]
fn steps_audio_interval_at_the_limits() {
    // characterized, not endorsed: a frame size of 0 gives an interval of 0
    assert_eq!(audio_interval(48000, 0), 0, "frame size 0");
    assert_eq!(
        audio_interval(1, 1 << 33),
        8_589_934_592_000_000_000,
        "2^33 samples at 1 Hz"
    );
    assert_eq!(
        audio_interval(u32::MAX, 1024),
        238,
        "the largest sample rate"
    );
}

// ---------------------------------------------------------------------------
// step 2: the batch size

#[test]
fn steps_batch_of_one_encoder() {
    assert_eq!(batch_size(&[33_333_333]), 2, "video only");
    assert_eq!(batch_size(&[21_333_333]), 2, "audio only");
}

#[test]
fn steps_batch_of_video_and_audio() {
    assert_eq!(batch_size(&[33_333_333, 21_333_333]), 2 + 3, "30 fps + AAC");
    assert_eq!(batch_size(&[21_333_333, 33_333_333]), 3 + 2, "either order");
    assert_eq!(
        batch_size(&[33_366_666, 23_219_954]),
        2 + 2,
        "29.97 + 44.1 kHz"
    );
    assert_eq!(batch_size(&[16_666_666, 21_333_333]), 2 + 2, "60 fps + AAC");
}

#[test]
fn steps_batch_of_a_multitrack_set() {
    let set = [16_666_666, 33_333_333, 21_333_333, 21_333_333, 21_333_333];
    assert_eq!(batch_size(&set), 4 + 2 + 3 * 3, "multitrack");
    assert_eq!(batch_size(&[33_333_333; 4]), 4 * 2, "four equal intervals");
}

#[test]
fn steps_batch_of_no_encoder() {
    assert_eq!(batch_size(&[]), 0, "none");
}

#[test]
fn steps_batch_at_the_limits() {
    assert_eq!(
        batch_size(&[1_000_000_000, 1]),
        2 + 2_000_000_000,
        "1 s and 1 ns"
    );
    // characterized, not endorsed: largest * 2 wraps to 0
    assert_eq!(batch_size(&[0x8000_0000_0000_0000, 1]), 0, "wraps to 0");
    assert_eq!(
        batch_size(&[0x7fff_ffff_ffff_ffff, 0x7fff_ffff_ffff_ffff]),
        4,
        "just fits"
    );
}

// ---------------------------------------------------------------------------
// pair_encoders()

#[test]
fn steps_pair_gates() {
    assert!(pair_video(false));
    assert!(!pair_video(true));

    assert!(pair_audio(false, 0));
    assert!(!pair_audio(true, 0));
    assert!(!pair_audio(false, 1));
    assert!(!pair_audio(true, 1));
    assert!(!pair_audio(false, 2));
    assert!(!pair_audio(false, usize::MAX));
}

// ---------------------------------------------------------------------------
// edges

/// The batch size from the C's expressions: the doubling wraps in `u64`.
fn model_batch(intervals: &[u64]) -> usize {
    let largest = intervals.iter().copied().max().unwrap_or(0);
    intervals
        .iter()
        .map(|&i| (largest.wrapping_mul(2) / i) as usize)
        .fold(0usize, usize::wrapping_add)
}

proptest! {
    #[test]
    fn edge_video_interval_is_exact(
        fps_num in 1u32..,
        fps_den in any::<u32>(),
        divisor in any::<u32>(),
    ) {
        // den < 2^32 and fps_num < 2^32: every platform's formula is exact.
        let den = fps_den.wrapping_mul(divisor);
        let exact = u128::from(NS_PER_SEC) * u128::from(den) / u128::from(fps_num);
        prop_assert_eq!(u128::from(video_interval(fps_num, fps_den, divisor)), exact);
    }

    #[test]
    fn edge_audio_interval_where_exact(
        sample_rate in 1u32..,
        frame_size in 0usize..(1 << 32),
    ) {
        let exact = u128::from(NS_PER_SEC) * frame_size as u128 / u128::from(sample_rate);
        prop_assert_eq!(u128::from(audio_interval(sample_rate, frame_size)), exact);
    }

    #[test]
    fn edge_batch_matches_the_model(
        intervals in prop::collection::vec(
            prop_oneof![1u64..200_000_000, 1u64.., Just(u64::MAX)],
            0..12,
        ),
    ) {
        prop_assert_eq!(batch_size(&intervals), model_batch(&intervals));
    }

    #[test]
    fn edge_batch_invariants(
        intervals in prop::collection::vec(1u64..1_000_000_000_000, 1..12),
    ) {
        // No wrap here: each encoder adds at least 2, the largest exactly 2,
        // and the order of the encoders does not matter.
        let b = batch_size(&intervals);
        prop_assert!(b >= 2 * intervals.len());
        let mut sorted = intervals.clone();
        sorted.sort_unstable();
        prop_assert_eq!(batch_size(&sorted), b);
        let largest = *sorted.last().unwrap_or(&1);
        prop_assert_eq!(batch_size(&[largest]), 2);
    }

    #[test]
    fn edge_any_input_without_a_panic(
        fps_num in any::<u32>(), fps_den in any::<u32>(), divisor in any::<u32>(),
        sample_rate in any::<u32>(), frame_size in any::<usize>(),
        intervals in prop::collection::vec(any::<u64>(), 0..8),
        num in any::<u64>(), mul in any::<u64>(), div in any::<u64>(),
    ) {
        let _ = video_interval(fps_num, fps_den, divisor);
        let _ = audio_interval(sample_rate, frame_size);
        let _ = batch_size(&intervals);
        let _ = mul_div64(num, mul, div);
    }
}

#[test]
fn edge_intentional_differences() {
    // A 0 fps_num or sample rate (C divides by 0): 0.
    assert_eq!(video_interval(0, 1, 1), 0);
    assert_eq!(audio_interval(0, 1024), 0);
    assert_eq!(mul_div64(5, 7, 0), 0);
    // An interval of 0 (C divides by 0) adds nothing; the others still count.
    assert_eq!(batch_size(&[0]), 0);
    assert_eq!(batch_size(&[33_333_333, 0, 21_333_333]), 2 + 3);
}
