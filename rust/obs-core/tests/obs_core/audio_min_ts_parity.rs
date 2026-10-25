//! Tier 3: the shims against the original `libobs/obs-audio-min-ts.c.h`.
//!
//! The oracle (`rust/obs-c-oracle/oracle/audio_min_ts.c`) is the C file,
//! unmodified, with its three functions renamed. Both sides get the same
//! values through their C signatures, and are compared: the result, the
//! minimum after `obs_audio_min_ts_earlier`, and `pending` after
//! `obs_audio_min_ts_insufficient`.
//!
//! Runs: the enum's size and values; every start frame at 44.1 and 48 kHz
//! on the frame's time and 1 ns either side, with the buffer one sample
//! short, exact and one sample over; 50 000 random cases per function
//! (timestamps near the minimum, near 0 and near `u64::MAX`, any buffered
//! size, rates from 0 to 384 kHz and any).
//!
//! Excluded, the intentional difference, tested on the shim alone below:
//! null pointers. `test_audio_buffering_subsample` compiles the C into
//! itself, so this is what compares the swap; the real-mixer tests run it
//! through libobs.

use core::ptr;

use obs_c_oracle::audio_min_ts::{
    LAYOUT_NAMES, oracle_audio_min_ts_layout, oracle_obs_audio_min_ts_buffering,
    oracle_obs_audio_min_ts_earlier, oracle_obs_audio_min_ts_insufficient,
};
use obs_core::audio_mix::AUDIO_OUTPUT_FRAMES;
use obs_core::ffi::audio_min_ts::{
    OBS_AUDIO_BUFFERING_ADD, OBS_AUDIO_BUFFERING_FIXED, OBS_AUDIO_BUFFERING_NONE,
    obs_audio_min_ts_buffering, obs_audio_min_ts_earlier, obs_audio_min_ts_insufficient,
};
use obs_media_io::audio_io::audio_frames_to_ns;
use proptest::prelude::*;

const START: u64 = 1_000_000_000;
const F32: usize = 4;

#[test]
fn layout() {
    let values = [
        size_of::<i32>(),
        OBS_AUDIO_BUFFERING_NONE as usize,
        OBS_AUDIO_BUFFERING_FIXED as usize,
        OBS_AUDIO_BUFFERING_ADD as usize,
    ];
    for (which, (name, value)) in LAYOUT_NAMES.iter().zip(values).enumerate() {
        // SAFETY: a pure C function.
        assert_eq!(
            unsafe { oracle_audio_min_ts_layout(which) },
            value,
            "{name}"
        );
    }
}

fn compare_earlier(pending: bool, audio_ts: u64, min: u64) {
    let (mut r, mut c) = (min, min);
    // SAFETY: valid pointers.
    let (rr, cr) = unsafe {
        (
            obs_audio_min_ts_earlier(pending, audio_ts, &mut r),
            oracle_obs_audio_min_ts_earlier(pending, audio_ts, &mut c),
        )
    };
    assert_eq!((rr, r), (cr, c), "earlier({pending}, {audio_ts}, {min})");
}

fn compare_insufficient(
    audio_render: bool,
    pending: bool,
    audio_ts: u64,
    buffered: usize,
    rate: usize,
    min_ts: u64,
) {
    let (mut rp, mut cp) = (pending, pending);
    // SAFETY: valid pointers.
    let (r, c) = unsafe {
        (
            obs_audio_min_ts_insufficient(audio_render, &mut rp, audio_ts, buffered, rate, min_ts),
            oracle_obs_audio_min_ts_insufficient(
                audio_render,
                &mut cp,
                audio_ts,
                buffered,
                rate,
                min_ts,
            ),
        )
    };
    assert_eq!(
        (r, rp),
        (c, cp),
        "insufficient({audio_render}, {pending}, {audio_ts}, {buffered}, {rate}, {min_ts})"
    );
}

#[test]
fn every_start_frame() {
    for rate in [44100usize, 48000] {
        for frame in 0..=AUDIO_OUTPUT_FRAMES as u64 {
            let at = START + audio_frames_to_ns(rate, frame);
            for audio_ts in [at - 1, at, at + 1] {
                let needed = (AUDIO_OUTPUT_FRAMES.saturating_sub(frame as usize)) * F32;
                for buffered in [needed.saturating_sub(F32), needed, needed + F32] {
                    compare_insufficient(false, false, audio_ts, buffered, rate, START);
                }
            }
        }
    }
}

/// The shim alone: null pointers do nothing.
#[test]
fn shim_null_pointers() {
    // SAFETY: null pointers are allowed and do nothing.
    unsafe {
        assert!(!obs_audio_min_ts_earlier(false, 5, ptr::null_mut()));
        assert!(!obs_audio_min_ts_insufficient(
            false,
            ptr::null_mut(),
            START,
            0,
            48000,
            START
        ));
    }
}

fn timestamp() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0u64),
        Just(START),
        Just(START - 1),
        START - 50_000_000..START + 50_000_000,
        Just(u64::MAX),
        any::<u64>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50_000))]

    #[test]
    fn earlier_matches(pending in any::<bool>(), audio_ts in timestamp(), min in timestamp()) {
        compare_earlier(pending, audio_ts, min);
    }

    #[test]
    fn insufficient_matches(
        audio_render in any::<bool>(),
        pending in any::<bool>(),
        audio_ts in timestamp(),
        buffered in prop_oneof![0usize..10_000, any::<usize>()],
        rate in prop_oneof![Just(44100usize), Just(48000usize), 0usize..384_000, any::<usize>()],
        min_ts in timestamp(),
    ) {
        compare_insufficient(audio_render, pending, audio_ts, buffered, rate, min_ts);
    }

    #[test]
    fn buffering_matches(
        fixed in any::<bool>(),
        maxed in any::<bool>(),
        min_ts in timestamp(),
        tick_start in timestamp(),
    ) {
        // SAFETY: plain values.
        let c = unsafe { oracle_obs_audio_min_ts_buffering(fixed, maxed, min_ts, tick_start) };
        prop_assert_eq!(obs_audio_min_ts_buffering(fixed, maxed, min_ts, tick_start), c);
    }
}
