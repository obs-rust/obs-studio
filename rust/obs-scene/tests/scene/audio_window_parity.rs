//! Tier 3: the shims of `obs_scene::ffi::scene_audio_window` against the
//! original `obs-scene-audio-window.c.h` (the oracle).
//!
//! Each run gives both sides the same values and compares the results byte
//! for byte: bools as bools, the child as the C's enum value, the earliest
//! timestamp as a `u64`, the window as `mix`, `pos` and `count`.
//!
//! Runs:
//! - the layout: the enum's size and values, the window struct's size,
//!   alignment and offsets, and `AUDIO_OUTPUT_FRAMES`, against the C;
//! - exhaustive over the flags: every combination of the pick's, the
//!   count's and the mix's three flags, and both duplicated values;
//! - the fold step over every pair of 22 special timestamps (0, 1, 2, near
//!   the tick, near `u64::MAX`), and `silent()` over them;
//! - the window over every combination of the pending flag, 30 source
//!   timestamps and 8 tick timestamps (the edges of frames 1023 to 1025 at
//!   each rate, before the tick, both ends of `u64`) and 24 rates;
//! - the first ns of every frame from 0 to 1025, and the ns before it, at
//!   the ten rates OBS offers;
//! - `scene_audio_render()`'s decisions in its order, one copy on each side,
//!   over random scenes (a proptest): the pick, the count and the fold of
//!   the first loop, the no-audio case, and the window, the mix and the add
//!   of the second;
//! - proptests (20,000 cases each) over full-range `u64` timestamps (half
//!   near the tick), any rate from 8000 to 384000 or any `usize`, and any
//!   flags.
//!
//! On MSVC, `util_mul_div64` is 128-bit math that faults when the quotient
//! does not fit in 64 bits (a rate of 1 GHz or more, with a long enough
//! distance), and the Rust saturates there, so those rates are not given on
//! MSVC.

use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::scene_audio_window::{
    LAYOUT_NAMES, OracleAudioWindow, oracle_obs_scene_audio_window_adds,
    oracle_obs_scene_audio_window_counts, oracle_obs_scene_audio_window_earliest,
    oracle_obs_scene_audio_window_frames, oracle_obs_scene_audio_window_mixes,
    oracle_obs_scene_audio_window_pick, oracle_obs_scene_audio_window_silent,
    oracle_scene_audio_window_layout,
};
use obs_scene::audio_window::AUDIO_OUTPUT_FRAMES;
use obs_scene::ffi::scene_audio_window::{
    OBS_SCENE_AUDIO_WINDOW_HIDE, OBS_SCENE_AUDIO_WINDOW_SHOW, OBS_SCENE_AUDIO_WINDOW_SOURCE,
    obs_scene_audio_window, obs_scene_audio_window_adds, obs_scene_audio_window_counts,
    obs_scene_audio_window_earliest, obs_scene_audio_window_frames, obs_scene_audio_window_mixes,
    obs_scene_audio_window_pick, obs_scene_audio_window_silent,
};
use proptest::prelude::*;

const NS: u64 = 1_000_000_000;
const TICK: u64 = 1_000_000_000_000;

/// Whether rates of 1 GHz and up may be given: not where the C faults.
const BIG_RATES: bool = !cfg!(all(target_env = "msvc", target_arch = "x86_64"));

/// The rates OBS offers.
const REAL_RATES: [usize; 10] = [
    8000, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 192000, 384000,
];

fn rates() -> Vec<usize> {
    let mut r: Vec<usize> = REAL_RATES.to_vec();
    r.extend([0, 1, 7, 11025, 999_999_999]);
    if BIG_RATES {
        r.extend([
            1_000_000_000,
            1_000_000_007,
            1 << 32,
            1 << 40,
            1 << 62,
            1 << 63,
            (1 << 63) + 1,
            usize::MAX - 1,
            usize::MAX,
        ]);
    }
    r
}

fn rust_frames(pending: bool, source_ts: u64, timestamp: u64, rate: usize) -> (bool, usize, usize) {
    let w: obs_scene_audio_window =
        obs_scene_audio_window_frames(pending, source_ts, timestamp, rate);
    (w.mix, w.pos, w.count)
}

fn c_frames(pending: bool, source_ts: u64, timestamp: u64, rate: usize) -> (bool, usize, usize) {
    // SAFETY: the oracle takes plain values and touches no memory.
    let w: OracleAudioWindow =
        unsafe { oracle_obs_scene_audio_window_frames(pending, source_ts, timestamp, rate) };
    (w.mix, w.pos, w.count)
}

fn check_frames(pending: bool, source_ts: u64, timestamp: u64, rate: usize) {
    assert_eq!(
        rust_frames(pending, source_ts, timestamp, rate),
        c_frames(pending, source_ts, timestamp, rate),
        "frames({pending}, {source_ts}, {timestamp}, {rate})"
    );
}

fn bools3() -> impl Iterator<Item = (bool, bool, bool)> {
    (0..8u8).map(|b| (b & 1 != 0, b & 2 != 0, b & 4 != 0))
}

#[test]
fn layout_matches_the_c() {
    let rust: [usize; 10] = [
        size_of::<core::ffi::c_int>(),
        usize::try_from(OBS_SCENE_AUDIO_WINDOW_SOURCE).unwrap(),
        usize::try_from(OBS_SCENE_AUDIO_WINDOW_SHOW).unwrap(),
        usize::try_from(OBS_SCENE_AUDIO_WINDOW_HIDE).unwrap(),
        size_of::<obs_scene_audio_window>(),
        align_of::<obs_scene_audio_window>(),
        offset_of!(obs_scene_audio_window, mix),
        offset_of!(obs_scene_audio_window, pos),
        offset_of!(obs_scene_audio_window, count),
        AUDIO_OUTPUT_FRAMES,
    ];
    for (i, (name, value)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: the oracle returns a constant for each index.
        let c = unsafe { oracle_scene_audio_window_layout(i) };
        assert_eq!(value, c, "{name}");
    }
    assert_eq!(
        size_of::<OracleAudioWindow>(),
        size_of::<obs_scene_audio_window>()
    );
}

#[test]
fn flags_exhaustive() {
    for (a, b, c) in bools3() {
        // SAFETY: the oracle takes plain values.
        unsafe {
            assert_eq!(
                obs_scene_audio_window_pick(a, b, c),
                oracle_obs_scene_audio_window_pick(a, b, c),
                "pick({a}, {b}, {c})"
            );
            assert_eq!(
                obs_scene_audio_window_counts(a, b, c),
                oracle_obs_scene_audio_window_counts(a, b, c),
                "counts({a}, {b}, {c})"
            );
            assert_eq!(
                obs_scene_audio_window_mixes(a, b, c),
                oracle_obs_scene_audio_window_mixes(a, b, c),
                "mixes({a}, {b}, {c})"
            );
        }
    }
    for d in [false, true] {
        // SAFETY: the oracle takes plain values.
        let c = unsafe { oracle_obs_scene_audio_window_adds(d) };
        assert_eq!(obs_scene_audio_window_adds(d), c, "adds({d})");
    }
}

fn special_timestamps() -> Vec<u64> {
    vec![
        0,
        1,
        2,
        3,
        TICK - 1,
        TICK,
        TICK + 1,
        TICK + 2,
        NS,
        u64::MAX / 2,
        u64::MAX / 2 + 1,
        u64::MAX - 21_333_334,
        u64::MAX - 21_333_333,
        u64::MAX - 21_312_500,
        u64::MAX - 3,
        u64::MAX - 2,
        u64::MAX - 1,
        u64::MAX,
        1 << 32,
        (1 << 32) - 1,
        1 << 63,
        (1 << 63) - 1,
    ]
}

#[test]
fn earliest_and_silent_pairs() {
    let ts = special_timestamps();
    for &a in &ts {
        // SAFETY: the oracle takes plain values.
        unsafe {
            assert_eq!(
                obs_scene_audio_window_silent(a),
                oracle_obs_scene_audio_window_silent(a),
                "silent({a})"
            );
        }
        for &b in &ts {
            // SAFETY: as above.
            let c = unsafe { oracle_obs_scene_audio_window_earliest(a, b) };
            assert_eq!(
                obs_scene_audio_window_earliest(a, b),
                c,
                "earliest({a}, {b})"
            );
        }
    }
}

#[test]
fn window_grid() {
    let ticks = [
        0,
        1,
        NS,
        TICK,
        u64::MAX / 2,
        u64::MAX - 21_354_167,
        u64::MAX - 1,
        u64::MAX,
    ];
    let rates = rates();
    for &rate in &rates {
        let r = u64::try_from(rate).unwrap();
        for &timestamp in &ticks {
            let mut sources: Vec<u64> = vec![
                0,
                1,
                timestamp,
                timestamp.wrapping_sub(1),
                timestamp.wrapping_add(1),
                u64::MAX,
                u64::MAX - 1,
                timestamp.wrapping_sub(NS),
                timestamp.wrapping_add(NS),
                timestamp.wrapping_add(2 * NS),
                timestamp.wrapping_add(999_999_999),
            ];
            if r != 0 {
                for k in [1022u64, 1023, 1024, 1025] {
                    let start =
                        u64::try_from((u128::from(k) * u128::from(NS)).div_ceil(u128::from(r)))
                            .unwrap_or(u64::MAX);
                    for d in [start.wrapping_sub(1), start, start.wrapping_add(1)] {
                        sources.push(timestamp.wrapping_add(d));
                        sources.push(timestamp.wrapping_sub(d));
                    }
                }
            }
            for &source_ts in &sources {
                for pending in [false, true] {
                    check_frames(pending, source_ts, timestamp, rate);
                }
            }
        }
    }
}

#[test]
fn every_frame_start() {
    for rate in REAL_RATES {
        let r = u64::try_from(rate).unwrap();
        for k in 0..=1025u64 {
            let start = (k * NS).div_ceil(r);
            for d in [start.wrapping_sub(1), start] {
                check_frames(false, TICK.wrapping_add(d), TICK, rate);
            }
        }
    }
}

/// One item of a scene: what each side's C would read.
#[derive(Debug, Clone)]
struct Item {
    visible: bool,
    show_active: bool,
    hide_active: bool,
    pending: bool,
    ts: u64,
    apply_buf: bool,
    duplicated: bool,
}

/// The decisions `scene_audio_render()` makes for a scene, in its order,
/// with the reads the C asks for (a value it does not ask for is false or
/// 0), through one side's seven functions.
struct Side {
    pick: fn(bool, bool, bool) -> i32,
    counts: fn(bool, bool, bool) -> bool,
    earliest: fn(u64, u64) -> u64,
    silent: fn(u64) -> bool,
    frames: fn(bool, u64, u64, usize) -> (bool, usize, usize),
    mixes: fn(bool, bool, bool) -> bool,
    adds: fn(bool) -> bool,
}

fn render(side: &Side, items: &[Item], rate: usize) -> Vec<String> {
    let mut log = Vec::new();
    let pick = |it: &Item| {
        let show = it.visible && it.show_active;
        let hide = !show && !it.visible && it.hide_active;
        (side.pick)(it.visible, show, hide)
    };
    let heard = |it: &Item| (it.visible, !it.visible && it.hide_active);
    let mut timestamp = 0;
    for it in items {
        let child = pick(it);
        let (visible, hiding) = if it.pending {
            (false, false)
        } else {
            heard(it)
        };
        let counted = (side.counts)(it.pending, visible, hiding);
        if counted {
            timestamp = (side.earliest)(timestamp, it.ts);
        }
        log.push(format!("pick {child} counts {counted} ts {timestamp}"));
    }
    let silent = (side.silent)(timestamp);
    log.push(format!("silent {silent}"));
    if silent {
        return log;
    }
    for it in items {
        let child = pick(it);
        let source_ts = if it.pending { 0 } else { it.ts };
        let w = (side.frames)(it.pending, source_ts, timestamp, rate);
        let mut line = format!("pick {child} window {w:?}");
        if w.0 {
            let (visible, hiding) = if it.apply_buf {
                (false, false)
            } else {
                heard(it)
            };
            let mixed = (side.mixes)(it.apply_buf, visible, hiding);
            line.push_str(&format!(" mixes {mixed}"));
            if mixed {
                line.push_str(&format!(" adds {}", (side.adds)(it.duplicated)));
            }
        }
        log.push(line);
    }
    log
}

fn rust_side() -> Side {
    Side {
        pick: |a, b, c| obs_scene_audio_window_pick(a, b, c),
        counts: |a, b, c| obs_scene_audio_window_counts(a, b, c),
        earliest: |a, b| obs_scene_audio_window_earliest(a, b),
        silent: |a| obs_scene_audio_window_silent(a),
        frames: rust_frames,
        mixes: |a, b, c| obs_scene_audio_window_mixes(a, b, c),
        adds: |a| obs_scene_audio_window_adds(a),
    }
}

fn c_side() -> Side {
    // SAFETY (each closure): the oracle takes plain values and touches no
    // memory.
    Side {
        pick: |a, b, c| unsafe { oracle_obs_scene_audio_window_pick(a, b, c) },
        counts: |a, b, c| unsafe { oracle_obs_scene_audio_window_counts(a, b, c) },
        earliest: |a, b| unsafe { oracle_obs_scene_audio_window_earliest(a, b) },
        silent: |a| unsafe { oracle_obs_scene_audio_window_silent(a) },
        frames: c_frames,
        mixes: |a, b, c| unsafe { oracle_obs_scene_audio_window_mixes(a, b, c) },
        adds: |a| unsafe { oracle_obs_scene_audio_window_adds(a) },
    }
}

fn any_ts() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0u64),
        any::<u64>(),
        (0u64..40_000_000).prop_map(|d| TICK + d),
        (0u64..40_000_000).prop_map(|d| TICK - d),
        (0u64..40_000_000).prop_map(|d| u64::MAX - d),
        (0u64..40_000_000),
    ]
}

fn any_rate() -> impl Strategy<Value = usize> {
    let real = prop_oneof![
        (0usize..10).prop_map(|i| REAL_RATES[i]),
        8000usize..=384_000,
        0usize..1_000_000_000,
    ];
    if BIG_RATES {
        prop_oneof![real, any::<usize>()].boxed()
    } else {
        real.boxed()
    }
}

fn any_item() -> impl Strategy<Value = Item> {
    (any::<[bool; 6]>(), any_ts()).prop_map(|(f, ts)| Item {
        visible: f[0],
        show_active: f[1],
        hide_active: f[2],
        pending: f[3],
        ts,
        apply_buf: f[4],
        duplicated: f[5],
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn frames_any(pending in any::<bool>(), source_ts in any_ts(), timestamp in any_ts(), rate in any_rate()) {
        prop_assert_eq!(
            rust_frames(pending, source_ts, timestamp, rate),
            c_frames(pending, source_ts, timestamp, rate)
        );
    }

    #[test]
    fn earliest_any(a in any_ts(), b in any_ts()) {
        let c = c_side();
        prop_assert_eq!(obs_scene_audio_window_earliest(a, b), (c.earliest)(a, b));
        prop_assert_eq!(obs_scene_audio_window_silent(a), (c.silent)(a));
    }

    #[test]
    fn scene_render_decisions(items in prop::collection::vec(any_item(), 0..9), rate in any_rate()) {
        prop_assert_eq!(render(&rust_side(), &items, rate), render(&c_side(), &items, rate));
    }
}
