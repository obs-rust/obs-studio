//! Tier 3: the shims of `obs_scene::ffi::scene_audio_actions` against the
//! original `obs-scene-audio-actions.c.h` (the oracle).
//!
//! Each run gives both sides the same queue (a darray of `struct
//! item_action`, the same bytes in the same capacity, padding and unused
//! slots included), the same visibility, tick, rate and gain buffer, and
//! compares:
//! - the return value (the hide count) and the visibility after the call;
//! - the darray: `array` (not reallocated), `num`, `capacity`, and every
//!   byte of the capacity, so the in-place erase must leave the memory the
//!   C's `da_erase` leaves;
//! - the gain buffer, bit for bit, and the gain one past the tick, which
//!   neither side may write when there is a tick.
//!
//! and `obs_scene_audio_action_due()` for the same time, tick and rate.
//!
//! Runs: the layout; every queue of up to 3 actions over chosen times
//! (before the tick, frames 0, 1, 511, the last ns of 1023, 1024 and 2000,
//! and the first time where `since * rate` wraps a `u64`),
//! with each start visibility, with and without a buffer, at 48 and 44.1
//! kHz, on a tick and with `ts` 0; and proptests over random queues of up
//! to 40 actions in any time order, at 0, near the tick and near
//! `u64::MAX`, ticks at 0, near 0, anywhere and near `u64::MAX`, and rates
//! from 8000 to 192000 plus 0, 1, odd ones, 1 GHz and (not on MSVC) rates
//! where `util_mul_div64` wraps.
//!
//! Intentional differences, excluded exactly:
//! - `ts` 0 with a buffer: the C writes past the 1024 gains, the Rust stops
//!   at the buffer end. Where the C's writes fit in a large buffer, the
//!   oracle gets one and the first 1024 gains must match. Otherwise the
//!   oracle runs twice: on the queue as is with no buffer (the return value,
//!   visibility and darray), and on the queue up to its first action past
//!   the tick, with a buffer of the tick (the gains: that action fills the
//!   rest of the tick with the visibility before it, which is what the
//!   oracle's tail fill writes without it, and every C write after it is
//!   past the tick). The Rust side gets the 1024 gains and the guard, which
//!   it may not write.
//! - `obs_scene_audio_action_due()` with a rate of 0, where the C divides by
//!   0: checked on the Rust side only (true).
//! - a null queue or visibility, which the C dereferences: checked on the
//!   Rust side only (0, nothing written).
//!
//! On MSVC, `util_mul_div64` is 128-bit math that faults when the quotient
//! does not fit in 64 bits (a rate above 1 GHz), and the Rust saturates
//! there, so those rates are not generated on MSVC.

use std::ffi::c_void;
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::darray::OracleDarray;
use obs_c_oracle::scene_audio_actions::{
    LAYOUT_NAMES, oracle_obs_scene_audio_action_due, oracle_obs_scene_audio_actions_apply,
    oracle_scene_audio_actions_layout,
};
use obs_scene::audio_actions::AUDIO_OUTPUT_FRAMES;
use obs_scene::ffi::scene_audio_actions::{
    item_action, obs_scene_audio_action_due, obs_scene_audio_actions_apply,
    obs_scene_item_action_array_t,
};
use proptest::prelude::*;

const FRAMES: usize = AUDIO_OUTPUT_FRAMES;
const NS: u64 = 1_000_000_000;
const TICK: u64 = 1_000_000_000_000;
const ITEM: usize = size_of::<item_action>();
/// A NaN that neither side writes (they write exactly 1.0 and 0.0).
const UNWRITTEN: u32 = 0x7fc0_5a5a;
/// The largest buffer the oracle gets for `ts` 0, in gains.
const BIG: u64 = 1 << 16;
/// Whether `util_mul_div64` is the 128-bit math that faults on overflow.
const UDIV128: bool = cfg!(target_env = "msvc");

/// The C's `util_mul_div64` (the `obs-util` port, independent of the
/// core's own copy), used only to find where the C writes.
fn mul_div64(num: u64, mul: u64, div: u64) -> u64 {
    obs_util::util_uint64::mul_div64(num, mul, div)
}

/// The C's frame of an action in the tick at `ts` (not bounded).
fn c_frame(timestamp: u64, ts: u64, rate: u64) -> u64 {
    mul_div64(timestamp.max(ts) - ts, rate, NS)
}

/// The first time that the C places on frame `k` of the tick at `ts`.
fn at_frame(ts: u64, k: u64, rate: u64) -> u64 {
    ts + (k * NS).div_ceil(rate)
}

/// One run's inputs.
#[derive(Debug, Clone)]
struct Case {
    actions: Vec<(bool, u64)>,
    spare: usize,
    visible: bool,
    ts: u64,
    rate: usize,
    buf: bool,
}

/// What one side left behind.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    hides: usize,
    visible: bool,
    num: usize,
    capacity: usize,
    moved: bool,
    bytes: Vec<u8>,
}

/// The queue's memory, 8-aligned: each slot is the action's bytes (bool,
/// then a fill pattern in the padding, then the timestamp), the spare slots
/// a pattern too.
fn queue_bytes(case: &Case) -> Vec<u64> {
    let slots = case.actions.len() + case.spare;
    let mut words = vec![0u64; slots * ITEM / 8];
    let bytes: &mut [u8] =
        unsafe { std::slice::from_raw_parts_mut(words.as_mut_ptr().cast::<u8>(), slots * ITEM) };
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(37).wrapping_add(11);
    }
    for (i, &(visible, timestamp)) in case.actions.iter().enumerate() {
        let slot = &mut bytes[i * ITEM..(i + 1) * ITEM];
        slot[offset_of!(item_action, visible)] = u8::from(visible);
        let at = offset_of!(item_action, timestamp);
        slot[at..at + 8].copy_from_slice(&timestamp.to_ne_bytes());
    }
    words
}

fn as_bytes(words: &[u64]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_ne_bytes()).collect()
}

fn gains(len: usize) -> Vec<f32> {
    vec![f32::from_bits(UNWRITTEN); len]
}

fn bits(buf: &[f32]) -> Vec<u32> {
    buf.iter().map(|g| g.to_bits()).collect()
}

/// The Rust shim on `case`, with a buffer of the tick and one guard gain.
fn run_rust(case: &Case) -> (Outcome, Option<Vec<f32>>) {
    let mut words = queue_bytes(case);
    let base = words.as_mut_ptr().cast::<c_void>();
    let mut da = obs_scene_item_action_array_t {
        array: if words.is_empty() {
            std::ptr::null_mut()
        } else {
            base.cast::<item_action>()
        },
        num: case.actions.len(),
        capacity: case.actions.len() + case.spare,
    };
    let mut visible = case.visible;
    let mut buf = case.buf.then(|| gains(FRAMES + 1));
    let p = buf
        .as_mut()
        .map_or(std::ptr::null_mut(), |b| b.as_mut_ptr());
    let hides =
        unsafe { obs_scene_audio_actions_apply(&mut da, &mut visible, case.ts, case.rate, p) };
    let out = Outcome {
        hides,
        visible,
        num: da.num,
        capacity: da.capacity,
        moved: !words.is_empty() && da.array.cast::<c_void>() != base,
        bytes: as_bytes(&words),
    };
    (out, buf)
}

/// The oracle on `actions` with the rest of `case`, and a buffer of `len`
/// gains (none for 0).
fn run_oracle(case: &Case, actions: &[(bool, u64)], len: usize) -> (Outcome, Option<Vec<f32>>) {
    let case = Case {
        actions: actions.to_vec(),
        ..case.clone()
    };
    let mut words = queue_bytes(&case);
    let base = words.as_mut_ptr().cast::<c_void>();
    let mut da = OracleDarray {
        array: if words.is_empty() {
            std::ptr::null_mut()
        } else {
            base
        },
        num: case.actions.len(),
        capacity: case.actions.len() + case.spare,
    };
    let mut visible = case.visible;
    let mut buf = (len != 0).then(|| gains(len));
    let p = buf
        .as_mut()
        .map_or(std::ptr::null_mut(), |b| b.as_mut_ptr());
    let hides = unsafe {
        oracle_obs_scene_audio_actions_apply(&mut da, &mut visible, case.ts, case.rate, p)
    };
    let out = Outcome {
        hides,
        visible,
        num: da.num,
        capacity: da.capacity,
        moved: !words.is_empty() && da.array != base,
        bytes: as_bytes(&words),
    };
    (out, buf)
}

/// Compares one run, and the window of each action's time.
fn check(case: &Case) {
    let (rust, rust_buf) = run_rust(case);
    let rate = case.rate as u64;

    if !case.buf || case.ts != 0 {
        let (oracle, oracle_buf) =
            run_oracle(case, &case.actions, if case.buf { FRAMES + 1 } else { 0 });
        assert_eq!(rust, oracle, "{case:?}");
        assert_eq!(
            rust_buf.as_deref().map(bits),
            oracle_buf.as_deref().map(bits),
            "gains {case:?}"
        );
    } else {
        // ts 0 with a buffer: the intentional difference (see the header).
        let rust_buf = rust_buf.unwrap();
        assert_eq!(rust_buf[FRAMES].to_bits(), UNWRITTEN, "guard {case:?}");
        let reach = case
            .actions
            .iter()
            .map(|&(_, t)| c_frame(t, 0, rate))
            .max()
            .unwrap_or(0)
            .max(FRAMES as u64);
        let (oracle, oracle_buf) = if reach <= BIG {
            run_oracle(case, &case.actions, reach as usize + 1)
        } else {
            let (oracle, _) = run_oracle(case, &case.actions, 0);
            let cut = case
                .actions
                .iter()
                .position(|&(_, t)| c_frame(t, 0, rate) >= FRAMES as u64)
                .unwrap();
            let (_, gains) = run_oracle(case, &case.actions[..cut], FRAMES + 1);
            (oracle, gains)
        };
        assert_eq!(rust, oracle, "{case:?}");
        assert_eq!(
            bits(&rust_buf[..FRAMES]),
            bits(&oracle_buf.unwrap()[..FRAMES]),
            "gains {case:?}"
        );
    }

    if rate != 0 {
        for &(_, t) in &case.actions {
            let r = obs_scene_audio_action_due(t, case.ts, case.rate);
            let o = unsafe { oracle_obs_scene_audio_action_due(t, case.ts, case.rate) };
            assert_eq!(r, o, "due({t}, {}, {})", case.ts, case.rate);
        }
    } else {
        for &(_, t) in &case.actions {
            assert!(obs_scene_audio_action_due(t, case.ts, 0));
        }
    }
}

#[test]
fn layout_matches_the_c() {
    let rust = [
        size_of::<item_action>(),
        align_of::<item_action>(),
        offset_of!(item_action, visible),
        offset_of!(item_action, timestamp),
        size_of::<obs_scene_item_action_array_t>(),
        offset_of!(obs_scene_item_action_array_t, array),
        offset_of!(obs_scene_item_action_array_t, num),
        offset_of!(obs_scene_item_action_array_t, capacity),
        AUDIO_OUTPUT_FRAMES,
    ];
    for (i, (name, value)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        assert_eq!(
            value,
            unsafe { oracle_scene_audio_actions_layout(i) },
            "{name}"
        );
    }
}

/// Every queue of up to 3 actions over the chosen times.
#[test]
fn small_queues_match_the_oracle() {
    let mut runs = 0;
    for rate in [48000u64, 44100] {
        for ts in [TICK, 0] {
            // with ts 0 the times are from 0, as from a tick at 0
            let base = if ts == 0 { 0 } else { TICK };
            let times = [
                base.saturating_sub(5 * NS),
                base,
                at_frame(base, 1, rate),
                at_frame(base, 511, rate),
                at_frame(base, 1024, rate) - 1,
                at_frame(base, 1024, rate),
                at_frame(base, 2000, rate),
                base + wrap(rate as usize, 1),
            ];
            let one: Vec<(bool, u64)> = times
                .iter()
                .flat_map(|&t| [(false, t), (true, t)])
                .collect();
            let mut queues: Vec<Vec<(bool, u64)>> = vec![vec![]];
            for len in 1..=3 {
                let mut next = Vec::new();
                for q in queues.iter().filter(|q| q.len() == len - 1) {
                    for &a in &one {
                        let mut q = q.clone();
                        q.push(a);
                        next.push(q);
                    }
                }
                queues.extend(next);
            }
            for actions in &queues {
                for visible in [false, true] {
                    for buf in [false, true] {
                        check(&Case {
                            actions: actions.clone(),
                            spare: 1,
                            visible,
                            ts,
                            rate: rate as usize,
                            buf,
                        });
                        runs += 1;
                    }
                }
            }
        }
    }
    // 16 actions: 1 + 16 + 16^2 + 16^3 queues
    assert_eq!(runs, 2 * 2 * 4369 * 4);
}

/// Null pointers: the Rust side does nothing (the C dereferences them).
#[test]
fn null_queue_or_visibility_does_nothing() {
    let mut visible = true;
    let mut buf = gains(FRAMES + 1);
    assert_eq!(
        unsafe {
            obs_scene_audio_actions_apply(
                std::ptr::null_mut(),
                &mut visible,
                TICK,
                48000,
                buf.as_mut_ptr(),
            )
        },
        0
    );
    assert!(visible);
    let mut da = obs_scene_item_action_array_t {
        array: std::ptr::null_mut(),
        num: 0,
        capacity: 0,
    };
    assert_eq!(
        unsafe {
            obs_scene_audio_actions_apply(
                &mut da,
                std::ptr::null_mut(),
                TICK,
                48000,
                buf.as_mut_ptr(),
            )
        },
        0
    );
    assert!(buf.iter().all(|g| g.to_bits() == UNWRITTEN));
}

/// The `ts` 0 reference: where both fit, the oracle on the queue up to its
/// first action past the tick gives the gains of the direct oracle.
#[test]
fn ts_0_reference_agrees_with_the_direct_oracle() {
    let case = Case {
        actions: vec![
            (false, at_frame(0, 100, 48000)),
            (true, at_frame(0, 900, 48000)),
            (false, at_frame(0, 3000, 48000)),
            (true, at_frame(0, 5000, 48000)),
        ],
        spare: 0,
        visible: true,
        ts: 0,
        rate: 48000,
        buf: true,
    };
    let (_, direct) = run_oracle(&case, &case.actions, 5001);
    let (_, cut) = run_oracle(&case, &case.actions[..2], FRAMES + 1);
    assert_eq!(
        bits(&direct.unwrap()[..FRAMES]),
        bits(&cut.unwrap()[..FRAMES])
    );
}

fn rate() -> impl Strategy<Value = usize> {
    let mut edge: Vec<usize> = vec![
        0,
        1,
        7,
        999,
        8000,
        11025,
        44100,
        48000,
        192000,
        1_000_000_000,
    ];
    if !UDIV128 {
        edge.extend([1_000_000_007, u32::MAX as usize, 20_000_000_000, usize::MAX]);
    }
    prop_oneof![
        3 => 8000usize..=192000,
        1 => proptest::sample::select(edge),
    ]
}

fn tick() -> impl Strategy<Value = u64> {
    prop_oneof![
        2 => Just(0u64),
        1 => 1u64..1_000_000,
        3 => (TICK - 10 * NS)..(TICK + 10 * NS),
        1 => any::<u64>(),
        1 => (u64::MAX - 100 * NS)..=u64::MAX,
    ]
}

/// A time relative to the tick `ts` (or to 0 when ts is 0): on a frame of
/// this tick or the next two, a ns either side, before the tick, or
/// anywhere.
fn time(ts: u64, rate: usize) -> impl Strategy<Value = u64> {
    let r = (rate as u64).clamp(1, 1_000_000_000);
    prop_oneof![
        4 => (0u64..3 * FRAMES as u64, -1i64..=1).prop_map(move |(k, d)| {
            ts.saturating_add((k * NS).div_ceil(r)).saturating_add_signed(d)
        }),
        1 => Just(0u64),
        1 => Just(ts),
        1 => (0..10 * NS).prop_map(move |d| ts.saturating_sub(d)),
        1 => (u64::MAX - 10 * NS)..=u64::MAX,
        1 => any::<u64>(),
        1 => (1u64..4, 0u64..2000).prop_map(move |(k, d)| ts.saturating_add(wrap(rate, k)).saturating_add(d)),
    ]
}

/// The `k`th time past a tick where `since * rate` wraps a `u64`: there a
/// product taken in one step lands back in the tick, but `util_mul_div64`
/// does not (it divides first). 0 for rates of 0 and 1.
fn wrap(rate: usize, k: u64) -> u64 {
    let r = rate as u64;
    if r < 2 {
        return 0;
    }
    (u64::MAX / r).saturating_add(1).saturating_mul(k)
}

fn case() -> impl Strategy<Value = Case> {
    (
        tick(),
        rate(),
        any::<bool>(),
        any::<bool>(),
        0usize..4,
        prop_oneof![3 => 0usize..8, 1 => 0usize..40],
    )
        .prop_flat_map(|(ts, rate, visible, buf, spare, len)| {
            proptest::collection::vec((any::<bool>(), time(ts, rate)), len).prop_map(
                move |actions| Case {
                    actions,
                    spare,
                    visible,
                    ts,
                    rate,
                    buf,
                },
            )
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn random_queues_match_the_oracle(case in case()) {
        check(&case);
    }

    /// The window on its own, over any time and tick, at any rate but 0.
    #[test]
    fn window_matches_the_oracle(t in any::<u64>(), ts in tick(), rate in rate()) {
        if rate == 0 {
            prop_assert!(obs_scene_audio_action_due(t, ts, 0));
        } else {
            let o = unsafe { oracle_obs_scene_audio_action_due(t, ts, rate) };
            prop_assert_eq!(obs_scene_audio_action_due(t, ts, rate), o);
            let d = (FRAMES as u64 * NS) / rate as u64;
            if !UDIV128 || rate as u64 <= NS {
                // and right at the edge of the window
                for t in [ts.wrapping_add(d).wrapping_sub(1), ts.wrapping_add(d)] {
                    let o = unsafe { oracle_obs_scene_audio_action_due(t, ts, rate) };
                    prop_assert_eq!(obs_scene_audio_action_due(t, ts, rate), o);
                }
            }
        }
    }
}
