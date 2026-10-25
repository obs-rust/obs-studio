//! Tier 1: the safe core of `libobs/obs-audio-min-ts.c.h`.
//!
//! The first 4 tests mirror the `obs-audio-min-ts.c` cases of
//! `test/cmocka/test_audio_buffering_subsample.c` one to one, with the same
//! names, in the same order, with the same inputs and the same expected
//! values. The fifth cmocka case, `calc_min_ts_through_the_list`, checks the
//! walk in `obs-audio.c` and has no counterpart here.
//!
//! The cases after those (`edge_*`) pin the edges of the safe API, and
//! proptests check that no input panics and the invariants.

use obs_core::audio_min_ts::{BufferingChoice, buffering, earlier, insufficient};
use obs_core::audio_mix::AUDIO_OUTPUT_FRAMES;
use obs_media_io::audio_io::audio_frames_to_ns;
use proptest::prelude::*;

const START: u64 = 1_000_000_000;
const FRAMES: u64 = AUDIO_OUTPUT_FRAMES as u64;
const F32: usize = 4;

#[test]
fn min_ts_takes_only_earlier_valid_sources() {
    let mut min_ts = START;

    // Pending, or without a timestamp: not taken.
    assert!(!earlier(true, START - 10, &mut min_ts));
    assert!(!earlier(false, 0, &mut min_ts));
    assert_eq!(min_ts, START);

    // Equal: not taken (strict).
    assert!(!earlier(false, START, &mut min_ts));

    // Earlier: taken, and the next one must be earlier still.
    assert!(earlier(false, START - 10, &mut min_ts));
    assert_eq!(min_ts, START - 10);
    assert!(!earlier(false, START - 10, &mut min_ts));
    assert!(!earlier(false, START - 5, &mut min_ts));
    assert!(earlier(false, START - 11, &mut min_ts));
    assert_eq!(min_ts, START - 11);
}

#[test]
fn insufficient_skips_render_pending_and_untimed() {
    let mut pending = false;
    assert!(!insufficient(true, &mut pending, START, 0, 48000, START));
    assert!(!pending);

    let mut pending = true;
    assert!(!insufficient(false, &mut pending, START, 0, 48000, START));
    assert!(pending);

    let mut pending = false;
    assert!(!insufficient(false, &mut pending, 0, 0, 48000, START));
    assert!(!pending);
}

#[test]
fn insufficient_needs_the_rest_of_the_tick() {
    let tick = AUDIO_OUTPUT_FRAMES * F32;

    // On min_ts and on min_ts - 1: a whole tick is enough, one sample
    // less is short and marks the source pending.
    for off in 0..2 {
        let mut pending = false;
        assert!(!insufficient(
            false,
            &mut pending,
            START - off,
            tick,
            48000,
            START
        ));
        assert!(!pending);
        assert!(insufficient(
            false,
            &mut pending,
            START - off,
            tick - F32,
            48000,
            START
        ));
        assert!(pending);
    }

    // From frame 512's time (frame 511): 513 frames needed.
    let at = START + audio_frames_to_ns(48000, 512);
    let mut pending = false;
    assert!(!insufficient(
        false,
        &mut pending,
        at,
        (AUDIO_OUTPUT_FRAMES - 511) * F32,
        48000,
        START
    ));
    assert!(insufficient(
        false,
        &mut pending,
        at,
        (AUDIO_OUTPUT_FRAMES - 512) * F32,
        48000,
        START
    ));

    // A start point at and past AUDIO_OUTPUT_FRAMES: never short. 8 kHz,
    // where one tick is exactly 128 ms.
    let tick8 = audio_frames_to_ns(8000, FRAMES);
    let mut pending = false;
    assert!(!insufficient(
        false,
        &mut pending,
        START + tick8,
        0,
        8000,
        START
    ));
    assert!(!insufficient(
        false,
        &mut pending,
        START + 2 * tick8,
        0,
        8000,
        START
    ));
    assert!(!pending);
}

#[test]
fn buffering_choice() {
    assert_eq!(buffering(true, false, START, START), BufferingChoice::Fixed);
    assert_eq!(
        buffering(true, false, START - 1, START),
        BufferingChoice::Fixed
    );
    assert_eq!(
        buffering(true, true, START - 1, START),
        BufferingChoice::None
    );
    assert_eq!(
        buffering(false, false, START - 1, START),
        BufferingChoice::Add
    );
    assert_eq!(
        buffering(false, true, START - 1, START),
        BufferingChoice::Add
    );
    assert_eq!(buffering(false, false, START, START), BufferingChoice::None);
    assert_eq!(
        buffering(false, false, START + 1, START),
        BufferingChoice::None
    );
}

#[test]
fn edge_min_ts_zero_wraps_the_one_ns_slack() {
    // min_ts - 1 wraps at 0, as in C: a source at u64::MAX counts as on
    // the tick and needs a whole tick.
    let mut pending = false;
    assert!(insufficient(false, &mut pending, u64::MAX, 0, 48000, 0));
    assert!(pending);
}

#[test]
fn edge_rate_zero_starts_on_frame_0() {
    // A rate of 0 maps every offset to frame 0, as util_mul_div64 does in
    // C (the rate is the multiplier there): a whole tick is needed.
    let mut pending = false;
    assert!(insufficient(
        false,
        &mut pending,
        START + 5_000_000,
        0,
        0,
        START
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5000))]

    /// earlier() only lowers min_ts, to the source's timestamp, and only
    /// for a valid earlier source.
    #[test]
    fn prop_earlier(pending in any::<bool>(), audio_ts in any::<u64>(), min in any::<u64>()) {
        let mut min_ts = min;
        let taken = earlier(pending, audio_ts, &mut min_ts);
        prop_assert_eq!(taken, !pending && audio_ts != 0 && audio_ts < min);
        prop_assert_eq!(min_ts, if taken { audio_ts } else { min });
    }

    /// insufficient() sets pending exactly when it returns true, and never
    /// clears it.
    #[test]
    fn prop_insufficient(
        audio_render in any::<bool>(),
        was_pending in any::<bool>(),
        audio_ts in any::<u64>(),
        buffered in any::<usize>(),
        rate in any::<usize>(),
        min_ts in any::<u64>(),
    ) {
        let mut pending = was_pending;
        let short = insufficient(audio_render, &mut pending, audio_ts, buffered, rate, min_ts);
        prop_assert_eq!(pending, was_pending || short);
        if audio_render || was_pending || audio_ts == 0 || buffered >= AUDIO_OUTPUT_FRAMES * F32 {
            prop_assert!(!short);
        }
    }
}
