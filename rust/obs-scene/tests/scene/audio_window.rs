//! Tier 1: `obs_scene::audio_window`, the audio window decisions of
//! `scene_audio_render()`.
//!
//! The `steps_*` cases port `test/cmocka/test_scene_audio_window_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations, the characterized-not-endorsed cases included.
//!
//! The `edge_*` cases are new: the frame against exact 128-bit math at the
//! rates OBS offers, the wrapping formula over the whole `u64` range, the
//! window's invariants, the minimum against a model in any order, the
//! values the C does not ask for left unread, and the truth tables against
//! their boolean rules.

use obs_scene::audio_window::{
    AUDIO_OUTPUT_FRAMES, Child, Frames, adds, counts, earliest, frames, min_timestamp, mixes,
    ns_to_audio_frames, pick, silent,
};
use proptest::prelude::*;

const FRAMES: usize = AUDIO_OUTPUT_FRAMES;
const NS: u64 = 1_000_000_000;
/// A tick timestamp well away from 0, which the C treats as "no timestamp".
const TICK: u64 = 1_000_000_000_000;

/// `util_mul_div64()` is the wrapping `uint64_t` formula everywhere but
/// MSVC x64, where it is exact 128-bit math that faults when the quotient
/// does not fit.
const MUL_DIV64_WRAPS: bool = !cfg!(all(target_env = "msvc", target_arch = "x86_64"));

/// The rates OBS offers.
const RATES: [u64; 10] = [
    8000, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 192000, 384000,
];

/// `frames()` as the C's (mix, pos, count).
fn window(w: Option<Frames>) -> (bool, usize, usize) {
    match w {
        Some(f) => (true, f.pos, f.count),
        None => (false, 0, 0),
    }
}

/// The expected (mix, pos, count) for a window from `pos`, or none.
fn expect(mix: bool, pos: usize) -> (bool, usize, usize) {
    if mix {
        (true, pos, FRAMES - pos)
    } else {
        (false, 0, 0)
    }
}

fn at(source_ts: u64, timestamp: u64, rate: u64) -> (bool, usize, usize) {
    window(frames(false, source_ts, timestamp, rate))
}

/// The first ns of frame `k` at `rate`.
fn frame_start(k: u64, rate: u64) -> u64 {
    (k * NS).div_ceil(rate)
}

/// The C's wrapping `util_mul_div64(ns, rate, 1e9)`.
fn wrapping_frame(ns: u64, rate: u64) -> u64 {
    (ns / NS)
        .wrapping_mul(rate)
        .wrapping_add((ns % NS).wrapping_mul(rate) / NS)
}

// ---------------------------------------------------------------------------
// The source read

#[test]
fn steps_pick_every_combination() {
    // visible, show active, hide active -> child
    let cases = [
        (false, false, false, Child::Source),
        (false, false, true, Child::Hide),
        // a show transition of a hidden item is not read
        (false, true, false, Child::Source),
        (false, true, true, Child::Hide),
        (true, false, false, Child::Source),
        // a hide transition of a visible item is not read
        (true, false, true, Child::Source),
        (true, true, false, Child::Show),
        (true, true, true, Child::Show),
    ];
    for (visible, show, hide, child) in cases {
        assert_eq!(
            pick(visible, show, hide),
            child,
            "pick({visible}, {show}, {hide})"
        );
    }
}

#[test]
fn steps_child_values() {
    // The C enum's values are pinned by the shim (ffi) and Tier 3; the core
    // enum has the three children, distinct.
    let all = [Child::Source, Child::Show, Child::Hide];
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a, b);
        }
    }
    assert_eq!(FRAMES, 1024);
}

// ---------------------------------------------------------------------------
// The earliest timestamp

#[test]
fn steps_counts_every_combination() {
    for bits in 0..8u8 {
        let pending = bits & 1 != 0;
        let visible = bits & 2 != 0;
        let hiding = bits & 4 != 0;
        assert_eq!(
            counts(pending, visible, hiding),
            !pending && (visible || hiding),
            "counts({pending}, {visible}, {hiding})"
        );
    }
}

/// One item of the first loop: pending, visible, hiding, timestamp.
type Timed = (bool, bool, bool, u64);

/// The first loop of `scene_audio_render()` over the items.
fn earliest_of(items: &[Timed]) -> u64 {
    let mut timestamp = 0;
    for &(pending, visible, hiding, ts) in items {
        if counts(pending, visible, hiding) {
            timestamp = earliest(timestamp, ts);
        }
    }
    timestamp
}

#[test]
fn steps_earliest_over_item_sets() {
    // no items
    assert_eq!(earliest_of(&[]), 0, "no items");

    // one item
    assert_eq!(
        earliest_of(&[(false, true, false, TICK)]),
        TICK,
        "one visible item"
    );
    assert_eq!(
        earliest_of(&[(false, true, false, 0)]),
        0,
        "one item with no timestamp"
    );
    assert_eq!(
        earliest_of(&[(false, false, true, TICK)]),
        TICK,
        "one hiding item"
    );
    assert_eq!(
        earliest_of(&[(false, false, false, TICK)]),
        0,
        "one hidden item, no hide transition"
    );
    assert_eq!(
        earliest_of(&[(true, true, false, TICK)]),
        0,
        "one pending item"
    );

    // three items, in every order
    let three = [TICK + 7, TICK + 3, TICK + 9];
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for order in orders {
        let items: Vec<Timed> = order
            .iter()
            .map(|&i| (false, true, false, three[i]))
            .collect();
        assert_eq!(earliest_of(&items), TICK + 3, "three items {order:?}");
    }

    // items with no timestamp do not count, wherever they are
    assert_eq!(
        earliest_of(&[
            (false, true, false, 0),
            (false, true, false, TICK + 4),
            (false, true, false, 0)
        ]),
        TICK + 4,
        "zeros around one timestamp"
    );
    assert_eq!(
        earliest_of(&[
            (false, true, false, 0),
            (false, false, true, 0),
            (false, true, true, 0)
        ]),
        0,
        "no timestamps"
    );

    // all pending: no audio, whatever the timestamps
    assert_eq!(
        earliest_of(&[
            (true, true, false, TICK),
            (true, false, true, TICK + 1),
            (true, true, true, 1)
        ]),
        0,
        "all pending"
    );

    // a pending or hidden item does not lower the minimum
    assert_eq!(
        earliest_of(&[
            (false, true, false, TICK + 5),
            (true, true, false, TICK + 1),
            (false, false, false, TICK + 2),
            (false, false, true, TICK + 4),
        ]),
        TICK + 4,
        "pending and hidden items excluded"
    );

    // equal timestamps
    assert_eq!(
        earliest_of(&[(false, true, false, TICK), (false, true, false, TICK)]),
        TICK,
        "equal timestamps"
    );

    // the u64 edges: 1 is the smallest timestamp, u64::MAX the largest
    assert_eq!(
        earliest_of(&[
            (false, true, false, u64::MAX),
            (false, true, false, 1),
            (false, true, false, u64::MAX - 1),
        ]),
        1,
        "1 and u64::MAX"
    );
    assert_eq!(
        earliest_of(&[(false, true, false, u64::MAX)]),
        u64::MAX,
        "u64::MAX alone"
    );
}

#[test]
fn steps_earliest_step() {
    assert_eq!(earliest(0, 0), 0, "0, 0");
    assert_eq!(earliest(0, 5), 5, "first timestamp");
    assert_eq!(earliest(5, 0), 5, "no timestamp keeps it");
    assert_eq!(earliest(5, 4), 4, "earlier");
    assert_eq!(earliest(5, 5), 5, "equal");
    assert_eq!(earliest(5, 6), 5, "later");
    assert_eq!(earliest(u64::MAX, 1), 1, "1 under u64::MAX");
    assert_eq!(earliest(1, u64::MAX), 1, "u64::MAX over 1");
    assert_eq!(earliest(0, u64::MAX), u64::MAX, "u64::MAX first");
}

#[test]
fn steps_silent() {
    assert!(silent(0), "0");
    assert!(!silent(1), "1");
    assert!(!silent(TICK), "TICK");
    assert!(!silent(u64::MAX), "u64::MAX");
}

// ---------------------------------------------------------------------------
// The item window

/// At 48 kHz frame f starts at f * 1e9 / 48000 ns: frame 1023 at exactly
/// 21312500 ns, frame 1024 at 21333333.3 ns, frame 1025 at 21354166.7 ns.
#[test]
fn steps_window_edges_48k() {
    let rate = 48000;
    assert_eq!(at(TICK, TICK, rate), expect(true, 0), "at the tick");
    assert_eq!(at(TICK + 1, TICK, rate), expect(true, 0), "1 ns in");
    assert_eq!(
        at(TICK + 20833, TICK, rate),
        expect(true, 0),
        "end of frame 0"
    );
    assert_eq!(at(TICK + 20834, TICK, rate), expect(true, 1), "frame 1");
    assert_eq!(
        at(TICK + 21312499, TICK, rate),
        expect(true, 1022),
        "end of 1022"
    );
    // pos = AUDIO_OUTPUT_FRAMES - 1: one frame mixed
    assert_eq!(
        at(TICK + 21312500, TICK, rate),
        expect(true, 1023),
        "frame 1023"
    );
    assert_eq!(
        at(TICK + 21333333, TICK, rate),
        expect(true, 1023),
        "end of 1023"
    );
    // pos = AUDIO_OUTPUT_FRAMES: skipped
    assert_eq!(
        at(TICK + 21333334, TICK, rate),
        expect(false, 0),
        "frame 1024"
    );
    assert_eq!(
        at(TICK + 21354166, TICK, rate),
        expect(false, 0),
        "end of 1024"
    );
    // pos = AUDIO_OUTPUT_FRAMES + 1
    assert_eq!(
        at(TICK + 21354167, TICK, rate),
        expect(false, 0),
        "frame 1025"
    );
    assert_eq!(
        at(TICK + 1000000000, TICK, rate),
        expect(false, 0),
        "a second on"
    );
}

/// At 44.1 kHz, frame 1023 starts at 23197278.9 ns and frame 1024 at
/// 23219954.6 ns.
#[test]
fn steps_window_edges_44k1() {
    let rate = 44100;
    assert_eq!(
        at(TICK + 23197278, TICK, rate),
        expect(true, 1022),
        "end of 1022"
    );
    assert_eq!(
        at(TICK + 23197279, TICK, rate),
        expect(true, 1023),
        "frame 1023"
    );
    assert_eq!(
        at(TICK + 23219954, TICK, rate),
        expect(true, 1023),
        "end of 1023"
    );
    assert_eq!(
        at(TICK + 23219955, TICK, rate),
        expect(false, 0),
        "frame 1024"
    );
    assert_eq!(
        at(TICK + 23242631, TICK, rate),
        expect(false, 0),
        "frame 1025"
    );
}

/// The other rates OBS offers, and the ends of the range: frame 1023 is in,
/// frame 1024 is out.
#[test]
fn steps_window_edges_other_rates() {
    // rate, first ns of frame 1023, first ns of frame 1024
    let cases: [(u64, u64, u64); 8] = [
        (8000, 127875000, 128000000),
        (16000, 63937500, 64000000),
        (22050, 46394558, 46439910),
        (32000, 31968750, 32000000),
        (88200, 11598640, 11609978),
        (96000, 10656250, 10666667),
        (192000, 5328125, 5333334),
        (384000, 2664063, 2666667),
    ];
    for (rate, f1023, f1024) in cases {
        assert_eq!(
            at(TICK + f1023 - 1, TICK, rate),
            expect(true, 1022),
            "end of 1022 at {rate}"
        );
        assert_eq!(
            at(TICK + f1023, TICK, rate),
            expect(true, 1023),
            "frame 1023 at {rate}"
        );
        assert_eq!(
            at(TICK + f1024 - 1, TICK, rate),
            expect(true, 1023),
            "end of 1023 at {rate}"
        );
        assert_eq!(
            at(TICK + f1024, TICK, rate),
            expect(false, 0),
            "frame 1024 at {rate}"
        );
    }
}

#[test]
fn steps_window_skips() {
    // pending: skipped, whatever the timestamp (the caller does not ask for
    // it and passes 0)
    assert_eq!(
        window(frames(true, 0, TICK, 48000)),
        expect(false, 0),
        "pending"
    );
    assert_eq!(
        window(frames(true, TICK, TICK, 48000)),
        expect(false, 0),
        "pending at the tick"
    );
    // no timestamp
    assert_eq!(at(0, TICK, 48000), expect(false, 0), "no timestamp");
    assert_eq!(at(0, 1, 48000), expect(false, 0), "no timestamp, tick 1");
}

/// An item left out of the minimum can start before it: the subtraction
/// wraps to a frame far past the tick, so the item is skipped, not mixed
/// from frame 0. Characterized, not endorsed.
#[test]
fn steps_window_before_the_minimum() {
    for rate in [8000, 44100, 48000, 96000, 384000] {
        assert_eq!(
            at(TICK - 1, TICK, rate),
            expect(false, 0),
            "1 ns before at {rate}"
        );
        assert_eq!(
            at(TICK - 21333333, TICK, rate),
            expect(false, 0),
            "a tick before at {rate}"
        );
        assert_eq!(at(1, TICK, rate), expect(false, 0), "at 1, {rate}");
    }

    // Across the end of u64 the wrap lands back in the tick: 1 is 2 ns
    // after u64::MAX - 1 and is mixed from frame 0. Characterized, not
    // endorsed: timestamps are ns of the monotonic clock.
    assert_eq!(
        at(1, u64::MAX - 1, 48000),
        expect(true, 0),
        "1 after u64::MAX - 1"
    );
    assert_eq!(
        at(21312499, u64::MAX - 1, 48000),
        expect(true, 1023),
        "frame 1023 across the end"
    );
    assert_eq!(
        at(21333332, u64::MAX - 1, 48000),
        expect(false, 0),
        "frame 1024 across the end"
    );
}

/// The u64 edges.
#[test]
fn steps_window_u64_edges() {
    assert_eq!(
        at(u64::MAX, u64::MAX, 48000),
        expect(true, 0),
        "u64::MAX both"
    );
    assert_eq!(
        at(u64::MAX, u64::MAX - 21312500, 48000),
        expect(true, 1023),
        "frame 1023 below u64::MAX"
    );
    assert_eq!(
        at(u64::MAX, u64::MAX - 21333334, 48000),
        expect(false, 0),
        "frame 1024 below u64::MAX"
    );
    assert_eq!(at(u64::MAX, 1, 48000), expect(false, 0), "1 to u64::MAX");
    assert_eq!(at(2, 1, 48000), expect(true, 0), "1 ns from 1");

    // rate 0: every item on frame 0. Characterized, not endorsed.
    assert_eq!(at(u64::MAX, 1, 0), expect(true, 0), "rate 0");
    assert_eq!(at(1, u64::MAX, 0), expect(true, 0), "rate 0, before");
    // rate 1: one frame a second
    assert_eq!(
        at(TICK + 1023999999999, TICK, 1),
        expect(true, 1023),
        "rate 1"
    );
    assert_eq!(
        at(TICK + 1024000000000, TICK, 1),
        expect(false, 0),
        "rate 1, 1024"
    );

    if MUL_DIV64_WRAPS {
        // Rates of 1 GHz and up: (ns / 1e9) * rate wraps. 2 s at 2^63 Hz is
        // 2^64 frames, which wraps to frame 0 and is mixed. Characterized,
        // not endorsed: no audio device runs at these rates.
        assert_eq!(
            at(TICK + 2000000000, TICK, 1 << 63),
            expect(true, 0),
            "2^64 frames wrap to 0"
        );
        assert_eq!(
            at(TICK + 1000000000, TICK, 1 << 63),
            expect(false, 0),
            "2^63 frames"
        );
        assert_eq!(
            at(TICK + 1, TICK, u64::MAX),
            expect(false, 0),
            "1 ns at SIZE_MAX"
        );
        assert_eq!(
            at(TICK, TICK, u64::MAX),
            expect(true, 0),
            "0 ns at SIZE_MAX"
        );
        // (ns % 1e9) * rate wraps too: 999999999 ns at 2^62 Hz
        assert_eq!(
            at(TICK + 999999999, TICK, 1 << 62),
            expect(false, 0),
            "remainder wraps"
        );
    }
}

#[test]
fn steps_mixes_every_combination() {
    for bits in 0..8u8 {
        let apply_buf = bits & 1 != 0;
        let visible = bits & 2 != 0;
        let hiding = bits & 4 != 0;
        // only a hidden item with no volume buffer and no hide transition is
        // left out
        assert_eq!(
            mixes(apply_buf, visible, hiding),
            apply_buf || visible || hiding,
            "mixes({apply_buf}, {visible}, {hiding})"
        );
    }
}

#[test]
fn steps_adds_unless_duplicated() {
    assert!(adds(false), "not duplicated");
    assert!(!adds(true), "duplicated");
}

// ---------------------------------------------------------------------------
// Edge cases

/// The values the C does not ask for do not change the decision: the hide
/// transition of a visible item, the show transition of a hidden one, the
/// visibility of a pending item and the timestamp of a pending source.
#[test]
fn edge_unasked_values_unread() {
    for a in [false, true] {
        for b in [false, true] {
            assert_eq!(
                pick(true, a, false),
                pick(true, a, b),
                "visible: hide unread"
            );
            assert_eq!(
                pick(false, false, a),
                pick(false, b, a),
                "hidden: show unread"
            );
            assert!(!counts(true, a, b), "pending: visibility unread");
            assert!(mixes(true, a, b), "volume buffer: visibility unread");
        }
    }
    for ts in [0, 1, TICK, u64::MAX] {
        assert_eq!(
            frames(true, ts, TICK, 48000),
            None,
            "pending: timestamp unread"
        );
    }
}

/// The frame boundaries of every rate OBS offers, computed rather than
/// listed: the first ns of frames 0 to 1025 and the ns before each.
#[test]
fn edge_every_frame_start() {
    for rate in RATES {
        for k in 0..=1025u64 {
            let start = frame_start(k, rate);
            let pos = usize::try_from(k).unwrap();
            assert_eq!(
                at(TICK + start, TICK, rate),
                expect(k < 1024, pos),
                "frame {k} at {rate}"
            );
            if k > 0 {
                let before = usize::try_from(k - 1).unwrap();
                assert_eq!(
                    at(TICK + start - 1, TICK, rate),
                    expect(k - 1 < 1024, before),
                    "end of frame {} at {rate}",
                    k - 1
                );
            }
        }
    }
}

#[test]
fn edge_ns_to_audio_frames_small() {
    assert_eq!(ns_to_audio_frames(48000, 0), 0);
    assert_eq!(ns_to_audio_frames(48000, NS), 48000);
    assert_eq!(ns_to_audio_frames(0, u64::MAX), 0);
    assert_eq!(ns_to_audio_frames(1, NS - 1), 0);
    assert_eq!(ns_to_audio_frames(384000, u64::MAX), 7083549724304467);
    if MUL_DIV64_WRAPS {
        assert_eq!(ns_to_audio_frames(1 << 63, 2 * NS), 0);
        assert_eq!(ns_to_audio_frames(u64::MAX, 1), 18446744073);
    } else {
        assert_eq!(ns_to_audio_frames(1 << 63, 2 * NS), u64::MAX);
    }
}

#[test]
fn edge_min_timestamp_sets() {
    assert_eq!(min_timestamp([]), 0);
    assert_eq!(min_timestamp([0, 0, 0]), 0);
    assert_eq!(min_timestamp([0, 9, 0, 3, 0]), 3);
    assert_eq!(min_timestamp([u64::MAX]), u64::MAX);
    assert_eq!(min_timestamp([u64::MAX, 1]), 1);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    /// At the rates OBS offers the frame is the exact 128-bit quotient, on
    /// every target, for any `u64` distance.
    #[test]
    fn edge_frame_exact_at_real_rates(ns in any::<u64>(), r in 0usize..10) {
        let rate = RATES[r];
        let exact = u64::try_from(u128::from(ns) * u128::from(rate) / u128::from(NS)).unwrap();
        prop_assert_eq!(ns_to_audio_frames(rate, ns), exact);
    }

    /// Off MSVC x64 the frame is the C's wrapping formula for any rate.
    #[test]
    fn edge_frame_wrapping_formula(ns in any::<u64>(), rate in any::<u64>()) {
        if MUL_DIV64_WRAPS {
            prop_assert_eq!(ns_to_audio_frames(rate, ns), wrapping_frame(ns, rate));
        } else {
            let exact = u128::from(ns) * u128::from(rate) / u128::from(NS);
            prop_assert_eq!(ns_to_audio_frames(rate, ns), u64::try_from(exact).unwrap_or(u64::MAX));
        }
    }

    /// The window against its rule, over full-range timestamps and rates.
    #[test]
    fn edge_window_model(
        pending in any::<bool>(),
        source_ts in prop_oneof![Just(0u64), any::<u64>(), (0u64..30_000_000).prop_map(|d| TICK + d)],
        timestamp in prop_oneof![Just(TICK), any::<u64>()],
        rate in prop_oneof![(0usize..10).prop_map(|r| RATES[r]), 0u64..400_000, any::<u64>()],
    ) {
        let got = frames(pending, source_ts, timestamp, rate);
        let frame = ns_to_audio_frames(rate, source_ts.wrapping_sub(timestamp));
        if pending || source_ts == 0 || frame >= 1024 {
            prop_assert_eq!(got, None);
        } else {
            let pos = usize::try_from(frame).unwrap();
            prop_assert_eq!(got, Some(Frames { pos, count: FRAMES - pos }));
        }
    }

    /// The minimum is the smallest non-zero timestamp, in any order.
    #[test]
    fn edge_min_timestamp_model(
        ts in prop::collection::vec(prop_oneof![Just(0u64), any::<u64>(), TICK..TICK + 100], 0..12),
        rotate in 0usize..12,
    ) {
        let want = ts.iter().copied().filter(|&t| t != 0).min().unwrap_or(0);
        prop_assert_eq!(min_timestamp(ts.iter().copied()), want);
        let mut turned = ts.clone();
        if !turned.is_empty() {
            let n = rotate % turned.len();
            turned.rotate_left(n);
        }
        turned.reverse();
        prop_assert_eq!(min_timestamp(turned), want);
        prop_assert_eq!(silent(want), ts.iter().all(|&t| t == 0));
    }
}
