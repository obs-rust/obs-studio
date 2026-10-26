//! Tier 3 parity: the shims of `obs_output::ffi::encoder_gpu_gates` (and
//! the safe core under them) against the original
//! `libobs/obs-encoder-gpu-gates.c.h`, compiled as
//! `oracle_obs_encoder_gpu_gates_*` (obs-c-oracle,
//! oracle/encoder_gpu_gates.c).
//!
//! Every output is compared exactly, on all three sides (oracle, shim,
//! core): counts, counters, keys and timestamps as integers, the decisions
//! as bools.
//!
//! Runs: the two constants; the `int` gates over -100,000 to 100,000, the
//! 100,000 values above `i32::MIN` and below `i32::MAX`, and a stride
//! through all of `i32`; the divisor sum, wrap and skip over every pair of
//! counters and divisors below 256 and every pair of 20 special `u32`
//! values, and the sum over 0 to 1,000,000 and a stride through all `u32`;
//! the key over every count and encoder number below 64 with 16 special
//! keys and every pair of 9 special `usize` values; the timing entry and
//! the requeue timestamp over every pair of 24 special `u64` values; the
//! format over -100,000 to 100,000 and a stride through all `i32`; the
//! per-frame body of `gpu_encode_thread()` (as the C test's stand-in, with
//! the shared group, paired and pts gates of `obs-encoder-audio-start.c`
//! on the matching side) on the oracle and on the shims, over every list of
//! one to three encoders with divisors 0 to 3 and counters 0 to 3 through
//! eight frames with counts 1 to 3 (comparing every encode, key, pts,
//! counter and requeue), and over the skip reasons (failed, group, paired,
//! paused) of a middle encoder; and proptests (20,000 cases each) of every
//! decision over full-range values and of the stand-in over encoder lists
//! and frame sequences from any state.
//!
//! Intentional difference: `warm_next` and `count_next` at `i32::MIN`,
//! where the C's subtraction is undefined, wrap in the port. The thread
//! starts the wait at 1 and a count at 1 or more. Tier 1 pins it; here the
//! oracle is called only above `i32::MIN`, which excludes exactly that
//! input. Otherwise no input is excluded. `test_encoder_gpu_gates_steps`
//! runs the C test against the C; the real-libobs texture encoder tests run
//! against the hybrid libobs, where these shims replace the C.

use std::ffi::c_int;

use obs_c_oracle::encoder_audio_start::{
    oracle_obs_encoder_audio_start_group_ready, oracle_obs_encoder_audio_start_paired_late,
    oracle_obs_encoder_audio_start_video_pts,
};
use obs_c_oracle::encoder_gpu_gates::{
    CONSTANT_NAMES, oracle_encoder_gpu_gates_constant, oracle_obs_encoder_gpu_gates_count_next,
    oracle_obs_encoder_gpu_gates_divisor_inc, oracle_obs_encoder_gpu_gates_divisor_skip,
    oracle_obs_encoder_gpu_gates_divisor_wrap, oracle_obs_encoder_gpu_gates_next_key,
    oracle_obs_encoder_gpu_gates_p010, oracle_obs_encoder_gpu_gates_record_timing,
    oracle_obs_encoder_gpu_gates_requeue, oracle_obs_encoder_gpu_gates_requeue_ts,
    oracle_obs_encoder_gpu_gates_warm_next, oracle_obs_encoder_gpu_gates_warming,
};
use obs_output::encoder_gpu_gates as core;
use obs_output::ffi::encoder_audio_start::{
    obs_encoder_audio_start_group_ready, obs_encoder_audio_start_paired_late,
    obs_encoder_audio_start_video_pts,
};
use obs_output::ffi::encoder_gpu_gates::{
    obs_encoder_gpu_gates_count_next, obs_encoder_gpu_gates_divisor_inc,
    obs_encoder_gpu_gates_divisor_skip, obs_encoder_gpu_gates_divisor_wrap,
    obs_encoder_gpu_gates_next_key, obs_encoder_gpu_gates_p010,
    obs_encoder_gpu_gates_record_timing, obs_encoder_gpu_gates_requeue,
    obs_encoder_gpu_gates_requeue_ts, obs_encoder_gpu_gates_warm_next,
    obs_encoder_gpu_gates_warming,
};
use proptest::prelude::*;

const FRAME_NS: u64 = 16_666_667;

fn special_u64() -> Vec<u64> {
    let mut v = Vec::new();
    for base in [0u64, 1 << 32, 1 << 63, FRAME_NS] {
        for d in [-1i64, 0, 1] {
            v.push(base.wrapping_add_signed(d));
        }
    }
    for d in 0..6u64 {
        v.push(u64::MAX - d);
    }
    v.extend([
        2,
        3,
        2 * FRAME_NS,
        1_000_000_000,
        0xdead_beef_cafe_f00d,
        33_333_333,
    ]);
    v
}

fn special_u32() -> Vec<u32> {
    let mut v: Vec<u32> = (0..8).collect();
    v.extend([
        0x7fff_ffff,
        0x8000_0000,
        0x8000_0001,
        0xffff,
        0x1_0000,
        1001,
        60,
        u32::MAX - 3,
        u32::MAX - 2,
        u32::MAX - 1,
        u32::MAX,
        0xdead_beef,
    ]);
    v
}

const SPECIAL_USIZE: [usize; 9] = [0, 1, 2, 3, 4, 63, 64, usize::MAX - 1, usize::MAX];

/// The `int` values compared: `i32::MIN` left out (the C's `- 1` is
/// undefined there).
fn ints() -> impl Iterator<Item = i32> {
    (-100_000..=100_000)
        .chain(i32::MIN + 1..=i32::MIN + 100_000)
        .chain(i32::MAX - 100_000..=i32::MAX)
        .chain((1..=i32::MAX / 7919).map(|k| k * 7919))
        .chain((1..=i32::MAX / 7919).map(|k| -(k * 7919)))
}

#[test]
fn parity_constants() {
    let ours = [core::WAIT_FRAMES, core::VIDEO_FORMAT_P010];
    for (i, name) in CONSTANT_NAMES.iter().enumerate() {
        let c = unsafe { oracle_encoder_gpu_gates_constant(i) };
        assert_eq!(c, i64::from(ours[i]), "{name}");
    }
    assert_eq!(
        unsafe { oracle_encoder_gpu_gates_constant(CONSTANT_NAMES.len()) },
        -1
    );
}

#[test]
fn parity_int_gates() {
    for x in ints() {
        let c: c_int = x;
        let o = unsafe {
            (
                oracle_obs_encoder_gpu_gates_warming(c),
                oracle_obs_encoder_gpu_gates_warm_next(c),
                oracle_obs_encoder_gpu_gates_count_next(c),
                oracle_obs_encoder_gpu_gates_requeue(c),
            )
        };
        let s = (
            obs_encoder_gpu_gates_warming(c),
            obs_encoder_gpu_gates_warm_next(c),
            obs_encoder_gpu_gates_count_next(c),
            obs_encoder_gpu_gates_requeue(c),
        );
        let k = (
            core::warming(x),
            core::warm_next(x),
            core::count_next(x),
            core::requeue(x),
        );
        assert_eq!(o, s, "{x}");
        assert_eq!(s, k, "{x}");
    }
}

fn check_divisor(c: u32, d: u32) {
    let o = unsafe {
        (
            oracle_obs_encoder_gpu_gates_divisor_inc(c),
            oracle_obs_encoder_gpu_gates_divisor_wrap(c, d),
            oracle_obs_encoder_gpu_gates_divisor_skip(c),
        )
    };
    let s = (
        obs_encoder_gpu_gates_divisor_inc(c),
        obs_encoder_gpu_gates_divisor_wrap(c, d),
        obs_encoder_gpu_gates_divisor_skip(c),
    );
    let k = (
        core::divisor_inc(c),
        core::divisor_wrap(c, d),
        core::divisor_skip(c),
    );
    assert_eq!(o, s, "{c} {d}");
    assert_eq!(s, k, "{c} {d}");
}

#[test]
fn parity_divisor() {
    for c in 0..256u32 {
        for d in 0..256u32 {
            check_divisor(c, d);
        }
    }
    let sp = special_u32();
    for &c in &sp {
        for &d in &sp {
            check_divisor(c, d);
        }
    }
    for c in (0..=1_000_000u32).chain((0..=u32::MAX / 65_521).map(|k| k * 65_521)) {
        check_divisor(c, c);
    }
}

fn check_key(count: usize, num: usize, key: u64) {
    let o = unsafe { oracle_obs_encoder_gpu_gates_next_key(count, num, key) };
    let s = obs_encoder_gpu_gates_next_key(count, num, key);
    assert_eq!(o, s, "{count} {num} {key}");
    assert_eq!(s, core::next_key(count, num, key), "{count} {num} {key}");
}

#[test]
fn parity_next_key() {
    let keys: Vec<u64> = special_u64().into_iter().take(16).collect();
    for count in 0..64usize {
        for num in 0..64usize {
            for &key in &keys {
                check_key(count, num, key);
            }
        }
    }
    for &count in &SPECIAL_USIZE {
        for &num in &SPECIAL_USIZE {
            for key in special_u64() {
                check_key(count, num, key);
            }
        }
    }
}

#[test]
fn parity_timing_and_requeue_ts() {
    let sp = special_u64();
    for &a in &sp {
        let o = unsafe { oracle_obs_encoder_gpu_gates_record_timing(a) };
        assert_eq!(o, obs_encoder_gpu_gates_record_timing(a), "{a}");
        assert_eq!(o, core::record_timing(a), "{a}");
        for &b in &sp {
            let o = unsafe { oracle_obs_encoder_gpu_gates_requeue_ts(a, b) };
            assert_eq!(o, obs_encoder_gpu_gates_requeue_ts(a, b), "{a} {b}");
            assert_eq!(o, core::requeue_ts(a, b), "{a} {b}");
        }
    }
}

#[test]
fn parity_format() {
    for f in (-100_000..=100_000)
        .chain([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX])
        .chain((i32::MIN / 7919..=i32::MAX / 7919).map(|k| k * 7919))
    {
        let o = unsafe { oracle_obs_encoder_gpu_gates_p010(f) };
        assert_eq!(o, obs_encoder_gpu_gates_p010(f), "{f}");
        assert_eq!(o, core::p010(f), "{f}");
    }
}

// ------------------------------------------------------------------------
// The stand-in on both sides

/// The gates one side asks.
struct Gates {
    group_ready: fn(u64, u64) -> bool,
    paired_late: fn(u64, u64) -> bool,
    video_pts: fn(i64, u32, u32) -> i64,
    divisor_inc: fn(u32) -> u32,
    divisor_wrap: fn(u32, u32) -> bool,
    divisor_skip: fn(u32) -> bool,
    next_key: fn(usize, usize, u64) -> u64,
    record_timing: fn(u64) -> bool,
    count_next: fn(c_int) -> c_int,
    requeue: fn(c_int) -> bool,
    requeue_ts: fn(u64, u64) -> u64,
}

const ORACLE: Gates = Gates {
    group_ready: |a, b| unsafe { oracle_obs_encoder_audio_start_group_ready(a, b) },
    paired_late: |a, b| unsafe { oracle_obs_encoder_audio_start_paired_late(a, b) },
    video_pts: |a, b, c| unsafe { oracle_obs_encoder_audio_start_video_pts(a, b, c) },
    divisor_inc: |a| unsafe { oracle_obs_encoder_gpu_gates_divisor_inc(a) },
    divisor_wrap: |a, b| unsafe { oracle_obs_encoder_gpu_gates_divisor_wrap(a, b) },
    divisor_skip: |a| unsafe { oracle_obs_encoder_gpu_gates_divisor_skip(a) },
    next_key: |a, b, c| unsafe { oracle_obs_encoder_gpu_gates_next_key(a, b, c) },
    record_timing: |a| unsafe { oracle_obs_encoder_gpu_gates_record_timing(a) },
    count_next: |a| unsafe { oracle_obs_encoder_gpu_gates_count_next(a) },
    requeue: |a| unsafe { oracle_obs_encoder_gpu_gates_requeue(a) },
    requeue_ts: |a, b| unsafe { oracle_obs_encoder_gpu_gates_requeue_ts(a, b) },
};

const SHIMS: Gates = Gates {
    group_ready: |a, b| obs_encoder_audio_start_group_ready(a, b),
    paired_late: |a, b| obs_encoder_audio_start_paired_late(a, b),
    video_pts: |a, b, c| obs_encoder_audio_start_video_pts(a, b, c),
    divisor_inc: |a| obs_encoder_gpu_gates_divisor_inc(a),
    divisor_wrap: |a, b| obs_encoder_gpu_gates_divisor_wrap(a, b),
    divisor_skip: |a| obs_encoder_gpu_gates_divisor_skip(a),
    next_key: |a, b, c| obs_encoder_gpu_gates_next_key(a, b, c),
    record_timing: |a| obs_encoder_gpu_gates_record_timing(a),
    count_next: |a| obs_encoder_gpu_gates_count_next(a),
    requeue: |a| obs_encoder_gpu_gates_requeue(a),
    requeue_ts: |a, b| obs_encoder_gpu_gates_requeue_ts(a, b),
};

#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct Paired {
    alive: bool,
    first_received: bool,
    first_raw_ts: u64,
}

#[derive(Clone, Default, Debug, PartialEq)]
struct Enc {
    failed: bool,
    group: Option<u64>,
    start_ts: u64,
    first_received: bool,
    paired: Vec<Paired>,
    paused: bool,
    frame_rate_divisor: u32,
    frame_rate_divisor_counter: u32,
    timebase_num: u32,
    cur_pts: i64,
    fail_encode: bool,
    /// Every encode: the frame, the key in, the next key in and the pts.
    calls: Vec<(u64, u64, u64, i64)>,
    timings: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    timestamp: u64,
    lock_key: u64,
    count: c_int,
}

/// One pass of `gpu_encode_thread()` over the frame on `g`'s gates, as the
/// C test's stand-in.
fn pass(g: &Gates, encs: &mut [Enc], tf: &mut Frame, interval: u64) -> bool {
    let timestamp = tf.timestamp;
    let mut lock_key = tf.lock_key;
    let mut key = tf.lock_key;
    let mut lock_count = 0usize;
    let num = encs.len();

    for e in encs.iter_mut() {
        if e.failed {
            continue;
        }
        if let Some(group_start) = e.group
            && e.start_ts == 0
            && !(g.group_ready)(group_start, timestamp)
        {
            continue;
        }
        if !e.first_received && !e.paired.is_empty() {
            let mut wait_for_audio = false;
            for p in &e.paired {
                if wait_for_audio {
                    break;
                }
                if !p.alive {
                    continue;
                }
                if !p.first_received || (g.paired_late)(p.first_raw_ts, timestamp) {
                    wait_for_audio = true;
                }
            }
            if wait_for_audio {
                continue;
            }
        }
        if e.paused {
            continue;
        }
        let skip = e.frame_rate_divisor_counter;
        e.frame_rate_divisor_counter = (g.divisor_inc)(skip);
        if (g.divisor_wrap)(e.frame_rate_divisor_counter, e.frame_rate_divisor) {
            e.frame_rate_divisor_counter = 0;
        }
        if (g.divisor_skip)(skip) {
            continue;
        }
        if e.start_ts == 0 {
            e.start_ts = timestamp;
        }
        lock_count += 1;
        key = (g.next_key)(lock_count, num, key);
        e.calls.push((timestamp, lock_key, key, e.cur_pts));
        // an encoder may move the key it hands on
        if e.timebase_num % 5 == 3 {
            key = key.wrapping_mul(3);
        }
        let success = !e.fail_encode;
        if (g.record_timing)(tf.timestamp) {
            e.timings += 1;
        }
        if !success {
            e.failed = true;
        }
        lock_key = key;
        e.cur_pts = (g.video_pts)(e.cur_pts, e.timebase_num, e.frame_rate_divisor);
    }

    tf.lock_key = key;
    tf.count = (g.count_next)(tf.count);
    if (g.requeue)(tf.count) {
        tf.timestamp = (g.requeue_ts)(tf.timestamp, interval);
        return true;
    }
    false
}

fn run_both(encs: &[Enc], frames: &[Frame], interval: u64) {
    let mut a = encs.to_vec();
    let mut b = encs.to_vec();
    for f in frames {
        let (mut fa, mut fb) = (*f, *f);
        for _ in 0..4 {
            let ra = pass(&ORACLE, &mut a, &mut fa, interval);
            let rb = pass(&SHIMS, &mut b, &mut fb, interval);
            assert_eq!(ra, rb, "{encs:?} {frames:?}");
            assert_eq!(fa, fb, "{encs:?} {frames:?}");
            if !ra {
                break;
            }
        }
    }
    assert_eq!(a, b, "{encs:?} {frames:?}");
}

fn enc(divisor: u32, counter: u32) -> Enc {
    Enc {
        first_received: true,
        frame_rate_divisor: divisor,
        frame_rate_divisor_counter: counter,
        timebase_num: 1,
        ..Enc::default()
    }
}

#[test]
fn parity_stand_in_lists() {
    let frames: Vec<Frame> = (0..8u64)
        .map(|f| Frame {
            timestamp: (f + 1) * FRAME_NS,
            lock_key: f,
            count: c_int::try_from(f % 3 + 1).unwrap(),
        })
        .collect();
    let mut lists = 0;
    for n in 1..=3usize {
        let combos = 16usize.pow(u32::try_from(n).unwrap());
        for code in 0..combos {
            let encs: Vec<Enc> = (0..n)
                .map(|i| {
                    let c = (code >> (4 * i)) & 15;
                    enc(
                        u32::try_from(c & 3).unwrap(),
                        u32::try_from(c >> 2).unwrap(),
                    )
                })
                .collect();
            run_both(&encs, &frames, FRAME_NS);
            run_both(&encs, &frames, u64::MAX - 1);
            lists += 1;
        }
    }
    assert_eq!(lists, 16 + 256 + 4096);
}

#[test]
fn parity_stand_in_skips() {
    let frames: Vec<Frame> = (0..6u64)
        .map(|f| Frame {
            timestamp: (f + 1) * FRAME_NS,
            lock_key: 5,
            count: 1,
        })
        .collect();
    for why in 0..6 {
        let mut encs = vec![enc(1, 0), enc(1, 0), enc(1, 0)];
        match why {
            0 => encs[1].failed = true,
            1 => encs[1].group = Some(3 * FRAME_NS),
            2 => {
                encs[1].first_received = false;
                encs[1].paired = vec![
                    Paired::default(),
                    Paired {
                        alive: true,
                        first_received: true,
                        first_raw_ts: 4 * FRAME_NS,
                    },
                ];
            }
            3 => encs[1].paused = true,
            4 => encs[1].fail_encode = true,
            _ => encs[1].timebase_num = 3,
        }
        run_both(&encs, &frames, FRAME_NS);
    }
}

fn any_enc() -> impl Strategy<Value = Enc> {
    (
        (
            prop::bool::weighted(0.1),
            prop::option::of(prop_oneof![
                Just(FRAME_NS),
                Just(2 * FRAME_NS),
                any::<u64>()
            ]),
            prop_oneof![Just(0u64), Just(FRAME_NS), any::<u64>()],
            any::<bool>(),
            prop::collection::vec(
                (
                    any::<bool>(),
                    any::<bool>(),
                    prop_oneof![Just(0u64), Just(FRAME_NS), Just(3 * FRAME_NS), any::<u64>()],
                ),
                0..4,
            ),
            prop::bool::weighted(0.1),
        ),
        (
            prop_oneof![0..=4u32, any::<u32>()],
            prop_oneof![0..=4u32, any::<u32>()],
            prop_oneof![Just(1u32), Just(1001u32), any::<u32>()],
            any::<i64>(),
            prop::bool::weighted(0.1),
        ),
    )
        .prop_map(
            |(
                (failed, group, start_ts, first_received, paired, paused),
                (
                    frame_rate_divisor,
                    frame_rate_divisor_counter,
                    timebase_num,
                    cur_pts,
                    fail_encode,
                ),
            )| Enc {
                failed,
                group,
                start_ts,
                first_received,
                paired: paired
                    .into_iter()
                    .map(|(alive, first_received, first_raw_ts)| Paired {
                        alive,
                        first_received,
                        first_raw_ts,
                    })
                    .collect(),
                paused,
                frame_rate_divisor,
                frame_rate_divisor_counter,
                timebase_num,
                cur_pts,
                fail_encode,
                ..Enc::default()
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn parity_decisions_full_range(a in any::<u64>(), b in any::<u64>(), c in any::<u32>(),
                                   d in any::<u32>(), n in any::<usize>(), m in any::<usize>(),
                                   i in (i32::MIN + 1)..=i32::MAX) {
        unsafe {
            prop_assert_eq!(oracle_obs_encoder_gpu_gates_warming(i), obs_encoder_gpu_gates_warming(i));
            prop_assert_eq!(oracle_obs_encoder_gpu_gates_warm_next(i), obs_encoder_gpu_gates_warm_next(i));
            prop_assert_eq!(oracle_obs_encoder_gpu_gates_count_next(i), obs_encoder_gpu_gates_count_next(i));
            prop_assert_eq!(oracle_obs_encoder_gpu_gates_requeue(i), obs_encoder_gpu_gates_requeue(i));
            prop_assert_eq!(oracle_obs_encoder_gpu_gates_p010(i), obs_encoder_gpu_gates_p010(i));
            prop_assert_eq!(oracle_obs_encoder_gpu_gates_divisor_inc(c), obs_encoder_gpu_gates_divisor_inc(c));
            prop_assert_eq!(oracle_obs_encoder_gpu_gates_divisor_wrap(c, d), obs_encoder_gpu_gates_divisor_wrap(c, d));
            prop_assert_eq!(oracle_obs_encoder_gpu_gates_divisor_skip(c), obs_encoder_gpu_gates_divisor_skip(c));
            prop_assert_eq!(oracle_obs_encoder_gpu_gates_next_key(n, m, a), obs_encoder_gpu_gates_next_key(n, m, a));
            prop_assert_eq!(oracle_obs_encoder_gpu_gates_next_key(n, n, a), obs_encoder_gpu_gates_next_key(n, n, a));
            prop_assert_eq!(oracle_obs_encoder_gpu_gates_record_timing(a), obs_encoder_gpu_gates_record_timing(a));
            prop_assert_eq!(oracle_obs_encoder_gpu_gates_requeue_ts(a, b), obs_encoder_gpu_gates_requeue_ts(a, b));
        }
        prop_assert_eq!(core::warm_next(i), obs_encoder_gpu_gates_warm_next(i));
        prop_assert_eq!(core::next_key(n, m, a), obs_encoder_gpu_gates_next_key(n, m, a));
        prop_assert_eq!(core::requeue_ts(a, b), obs_encoder_gpu_gates_requeue_ts(a, b));
    }

    #[test]
    fn parity_stand_in_any(encs in prop::collection::vec(any_enc(), 0..5),
                           frames in prop::collection::vec(
                               (prop_oneof![Just(0u64), Just(FRAME_NS), Just(u64::MAX - 3), any::<u64>()],
                                any::<u64>(), 1..=4i32), 1..8),
                           interval in prop_oneof![Just(FRAME_NS), Just(u64::MAX), any::<u64>()]) {
        let frames: Vec<Frame> = frames
            .into_iter()
            .map(|(timestamp, lock_key, count)| Frame { timestamp, lock_key, count })
            .collect();
        run_both(&encs, &frames, interval);
    }
}
