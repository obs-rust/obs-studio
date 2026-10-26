//! Tier 1: `obs_output::encoder_gpu_gates`, the per-frame and per-encoder
//! gates of the GPU encode thread.
//!
//! The `steps_*` cases port `test/cmocka/test_encoder_gpu_gates_steps.c` 1:1:
//! its 16 cases under the same names, in the same order, with the same values
//! and expectations, the "characterized, not endorsed" ones included. The C
//! calls the hidden `obs_encoder_gpu_gates_*` and `obs_encoder_audio_start_*`
//! functions; here the safe core is called directly. The C's stand-in (the
//! per-frame body of `gpu_encode_thread()` over stand-in encoders) is
//! [`pass`].
//!
//! The `edge_*` cases are new: every decision against a model of the
//! original C over edge values, every `i32` near the extremes and every
//! format, and proptests of every decision over full-range inputs and of
//! the stand-in over frame sequences against the origin/master code with
//! its expressions inline.

use obs_output::encoder_gpu_gates::{
    VIDEO_FORMAT_P010, WAIT_FRAMES, count_next, divisor_inc, divisor_skip, divisor_wrap,
    group_ready, next_key, p010, paired_late, record_timing, requeue, requeue_ts, video_pts,
    warm_next, warming,
};
use proptest::prelude::*;

const EDGE_U64: [u64; 7] = [
    0,
    1,
    2,
    0x7fff_ffff_ffff_ffff,
    0x8000_0000_0000_0000,
    u64::MAX - 1,
    u64::MAX,
];
const EDGE_U32: [u32; 8] = [0, 1, 2, 3, 0x7fff_ffff, 0x8000_0000, u32::MAX - 1, u32::MAX];
const EDGE_SIZE: [usize; 7] = [0, 1, 2, 3, 4, usize::MAX - 1, usize::MAX];
// i32::MIN is left out, as in the C: count - 1 there overflows int
const EDGE_INT: [i32; 9] = [i32::MIN + 1, -2, -1, 0, 1, 2, 3, i32::MAX - 1, i32::MAX];

/// `VIDEO_FORMAT_NV12` and `VIDEO_FORMAT_R10L` (`media-io/video-io.h`).
const VIDEO_FORMAT_NV12: i32 = 2;
const VIDEO_FORMAT_R10L: i32 = 25;

const FRAME_NS: u64 = 16_666_667;

// ------------------------------------------------------------------------
// The stand-in: the per-frame body of gpu_encode_thread(), as the C test's.

const MAX_CALLS: usize = 32;

#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct Paired {
    alive: bool,
    first_received: bool,
    first_raw_ts: u64,
}

#[derive(Clone, Default, Debug, PartialEq)]
struct Enc {
    failed: bool,
    /// The group's `start_timestamp`, when the encoder is in a group.
    group: Option<u64>,
    start_ts: u64,
    first_received: bool,
    paired: Vec<Paired>,
    paused: bool,
    frame_rate_divisor: u32,
    frame_rate_divisor_counter: u32,
    timebase_num: u32,
    cur_pts: i64,
    has_texture2: bool,
    fail_encode: bool,

    // what the stand-in encode saw
    calls: usize,
    lock_key: Vec<u64>,
    next_key: Vec<u64>,
    pts: Vec<i64>,
    frame_ts: Vec<u64>,
    texture2_calls: usize,
    timings: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    timestamp: u64,
    lock_key: u64,
    count: i32,
}

fn enc(divisor: u32) -> Enc {
    Enc {
        frame_rate_divisor: divisor,
        timebase_num: 1,
        first_received: true,
        ..Enc::default()
    }
}

fn frame(timestamp: u64) -> Frame {
    Frame {
        timestamp,
        lock_key: 0,
        count: 1,
    }
}

fn encode(e: &mut Enc, timestamp: u64, lock_key: u64, next_key: &mut u64, texture2: bool) -> bool {
    if e.calls < MAX_CALLS {
        e.lock_key.push(lock_key);
        e.next_key.push(*next_key);
        e.pts.push(e.cur_pts);
        e.frame_ts.push(timestamp);
    }
    e.calls += 1;
    if texture2 {
        e.texture2_calls += 1;
    }
    !e.fail_encode
}

/// One pass of the thread over the frame, on the core; true: the frame goes
/// back to the front of the queue (with its new timestamp), false: to the
/// available queue.
fn pass(encs: &mut [Enc], tf: &mut Frame, interval: u64) -> bool {
    let timestamp = tf.timestamp;
    let mut lock_key = tf.lock_key;
    let mut key = tf.lock_key;
    let mut lock_count = 0usize;
    let num = encs.len();

    for encoder in encs.iter_mut() {
        if encoder.failed {
            continue;
        }
        if let Some(group_start) = encoder.group
            && encoder.start_ts == 0
            && !group_ready(group_start, timestamp)
        {
            continue;
        }
        if !encoder.first_received && !encoder.paired.is_empty() {
            let mut wait_for_audio = false;
            for p in &encoder.paired {
                if wait_for_audio {
                    break;
                }
                if !p.alive {
                    continue;
                }
                if !p.first_received || paired_late(p.first_raw_ts, timestamp) {
                    wait_for_audio = true;
                }
            }
            if wait_for_audio {
                continue;
            }
        }
        if encoder.paused {
            continue;
        }

        let skip = encoder.frame_rate_divisor_counter;
        encoder.frame_rate_divisor_counter = divisor_inc(skip);
        if divisor_wrap(
            encoder.frame_rate_divisor_counter,
            encoder.frame_rate_divisor,
        ) {
            encoder.frame_rate_divisor_counter = 0;
        }
        if divisor_skip(skip) {
            continue;
        }

        if encoder.start_ts == 0 {
            encoder.start_ts = timestamp;
        }

        lock_count += 1;
        key = next_key(lock_count, num, key);

        let texture2 = encoder.has_texture2;
        let success = encode(encoder, timestamp, lock_key, &mut key, texture2);

        if record_timing(tf.timestamp) {
            encoder.timings += 1;
        }
        // send_off_encoder_packet(): a failed encode stops the encoder
        if !success {
            encoder.failed = true;
        }

        lock_key = key;

        encoder.cur_pts = video_pts(
            encoder.cur_pts,
            encoder.timebase_num,
            encoder.frame_rate_divisor,
        );
    }

    tf.lock_key = key;
    tf.count = count_next(tf.count);
    if requeue(tf.count) {
        tf.timestamp = requeue_ts(tf.timestamp, interval);
        return true;
    }
    false
}

// ------------------------------------------------------------------------
// test_encoder_gpu_gates_steps.c, 1:1

#[test]
fn steps_constants() {
    // NUM_ENCODE_TEXTURE_FRAMES_TO_WAIT: one frame is skipped
    assert_eq!(WAIT_FRAMES, 1);
    assert_eq!(VIDEO_FORMAT_P010, 18);
}

#[test]
fn steps_warmup() {
    for &x in &EDGE_INT {
        assert_eq!(warming(x), x != 0, "{x}");
        assert_eq!(warm_next(x), x - 1, "{x}");
    }

    // the thread skips frames while warming and counts down: exactly
    // WAIT_FRAMES of them
    let mut wait_frames = WAIT_FRAMES;
    let mut skipped = 0;
    for _ in 0..10 {
        if warming(wait_frames) {
            wait_frames = warm_next(wait_frames);
            skipped += 1;
        }
    }
    assert_eq!(skipped, 1);
    assert_eq!(wait_frames, 0);
}

#[test]
fn steps_divisor_gates() {
    for &a in &EDGE_U32 {
        // a u32 sum: u32::MAX wraps to 0
        assert_eq!(divisor_inc(a), a.wrapping_add(1), "{a}");
        assert_eq!(divisor_skip(a), a != 0, "{a}");
        for &b in &EDGE_U32 {
            assert_eq!(divisor_wrap(a, b), a == b, "{a} {b}");
        }
    }
    assert_eq!(divisor_inc(u32::MAX), 0);
}

#[test]
fn steps_next_key() {
    for &count in &EDGE_SIZE {
        for &num in &EDGE_SIZE {
            for &key in &EDGE_U64 {
                let want = if count == num { 0 } else { key.wrapping_add(1) };
                assert_eq!(next_key(count, num, key), want, "{count} {num} {key}");
            }
        }
    }
    // the last encoder hands on 0, any other the next key; the key wraps
    assert_eq!(next_key(3, 3, 41), 0);
    assert_eq!(next_key(2, 3, 41), 42);
    assert_eq!(next_key(1, 3, u64::MAX), 0);
    // characterized, not endorsed: lock_count past the number of encoders
    // never happens (it counts the encoders that encode), and hands on the
    // next key
    assert_eq!(next_key(4, 3, 41), 42);
}

#[test]
fn steps_record_timing() {
    for &ts in &EDGE_U64 {
        assert_eq!(record_timing(ts), ts != 0, "{ts}");
    }
}

#[test]
fn steps_requeue_gates() {
    for &c in &EDGE_INT {
        assert_eq!(count_next(c), c - 1, "{c}");
        assert_eq!(requeue(c), c != 0, "{c}");
    }
    for &a in &EDGE_U64 {
        for &b in &EDGE_U64 {
            assert_eq!(requeue_ts(a, b), a.wrapping_add(b), "{a} {b}");
        }
    }
    // a u64 sum: near u64::MAX it wraps
    assert_eq!(requeue_ts(u64::MAX - 5, 16_666_667), 16_666_661);
}

#[test]
fn steps_texture_format() {
    for f in 0..=VIDEO_FORMAT_R10L {
        assert_eq!(p010(f), f == VIDEO_FORMAT_P010, "{f}");
    }
    // every other value, NV12 included, gives NV12 textures
    assert!(!p010(VIDEO_FORMAT_NV12));
    assert!(!p010(-1));
    assert!(!p010(i32::MAX));
    assert!(!p010(i32::MIN));
}

#[test]
fn steps_shared_gates() {
    assert!(group_ready(16_666_667, 16_666_667));
    assert!(!group_ready(16_666_667, 33_333_334));
    assert!(paired_late(16_666_668, 16_666_667));
    assert!(!paired_late(16_666_667, 16_666_667));
    // the pts step: timebase_num times the divisor, a u32 product
    assert_eq!(video_pts(10, 1, 2), 12);
    assert_eq!(video_pts(0, 0x10000, 0x10000), 0);
}

/// Two encoders with the same divisor, started on the same frame, encode
/// on the same frames: the first frame and then every divisor-th one. That
/// is why the counter is explicit.
#[test]
fn steps_e2e_divisors_in_step() {
    for d in 1..=3u32 {
        let mut encs = [enc(d), enc(d)];
        for f in 0..12u64 {
            let mut tf = frame((f + 1) * FRAME_NS);
            assert!(!pass(&mut encs, &mut tf, FRAME_NS));
        }
        for e in &encs {
            assert_eq!(e.calls, 12 / usize::try_from(d).unwrap());
            for c in 0..e.calls {
                let c64 = u64::try_from(c).unwrap();
                assert_eq!(e.frame_ts[c], (c64 * u64::from(d) + 1) * FRAME_NS);
                // the pts steps by timebase_num times the divisor
                assert_eq!(e.pts[c], i64::try_from(c64 * u64::from(d)).unwrap());
            }
            assert_eq!(e.start_ts, FRAME_NS);
            assert_eq!(e.frame_rate_divisor_counter, 12 % d);
        }
    }

    // an encoder whose counter is one frame in is out of step: it encodes
    // when the other skips
    let mut encs = [enc(2), enc(2)];
    encs[1].frame_rate_divisor_counter = 1;
    for f in 0..4u64 {
        let mut tf = frame((f + 1) * FRAME_NS);
        pass(&mut encs, &mut tf, FRAME_NS);
    }
    assert_eq!(encs[0].calls, 2);
    assert_eq!(encs[1].calls, 2);
    assert_eq!(encs[0].frame_ts[0], FRAME_NS);
    assert_eq!(encs[1].frame_ts[0], 2 * FRAME_NS);

    // characterized, not endorsed: a divisor of 0 (the setter refuses it)
    // encodes the first frame and then never again until the u32 counter
    // wraps
    let mut zero = [enc(0)];
    for f in 0..5u64 {
        let mut tf = frame((f + 1) * FRAME_NS);
        pass(&mut zero, &mut tf, FRAME_NS);
    }
    assert_eq!(zero[0].calls, 1);
    assert_eq!(zero[0].frame_rate_divisor_counter, 5);
    zero[0].frame_rate_divisor_counter = u32::MAX;
    let mut tf = frame(6 * FRAME_NS);
    pass(&mut zero, &mut tf, FRAME_NS);
    assert_eq!(zero[0].calls, 1);
    assert_eq!(zero[0].frame_rate_divisor_counter, 0);
    tf.timestamp = 7 * FRAME_NS;
    tf.count = 1;
    pass(&mut zero, &mut tf, FRAME_NS);
    assert_eq!(zero[0].calls, 2);
}

/// Every encoder takes the key the one before handed on; the last hands on
/// 0, which the frame keeps.
#[test]
fn steps_e2e_keys_all_encode() {
    let mut encs = [enc(1), enc(1), enc(1)];
    let mut tf = Frame {
        timestamp: FRAME_NS,
        lock_key: 5,
        count: 1,
    };
    assert!(!pass(&mut encs, &mut tf, FRAME_NS));
    assert_eq!((encs[0].lock_key[0], encs[0].next_key[0]), (5, 6));
    assert_eq!((encs[1].lock_key[0], encs[1].next_key[0]), (6, 7));
    assert_eq!((encs[2].lock_key[0], encs[2].next_key[0]), (7, 0));
    assert_eq!(tf.lock_key, 0);

    // one encoder: key 0 in, 0 handed on
    let mut one = [enc(1)];
    let mut tf1 = frame(FRAME_NS);
    pass(&mut one, &mut tf1, FRAME_NS);
    assert_eq!((one[0].lock_key[0], one[0].next_key[0]), (0, 0));
    assert_eq!(tf1.lock_key, 0);

    // the key wraps
    let mut two = [enc(1), enc(1)];
    let mut tf2 = Frame {
        timestamp: FRAME_NS,
        lock_key: u64::MAX,
        count: 1,
    };
    pass(&mut two, &mut tf2, FRAME_NS);
    assert_eq!((two[0].lock_key[0], two[0].next_key[0]), (u64::MAX, 0));
    assert_eq!((two[1].lock_key[0], two[1].next_key[0]), (0, 0));
}

/// Characterized, not endorsed (upstream): when an encoder in the list
/// skips the frame (divisor, failed, waiting or paused), lock_count never
/// reaches the number of encoders, so the last key handed on is not 0, and
/// the frame keeps it.
#[test]
fn steps_e2e_keys_one_skips() {
    for why in 0..4 {
        let mut encs = [enc(1), enc(1), enc(1)];
        match why {
            0 => {
                encs[1].frame_rate_divisor = 2;
                encs[1].frame_rate_divisor_counter = 1;
            }
            1 => encs[1].failed = true,
            2 => encs[1].paused = true,
            _ => {
                encs[1].first_received = false;
                encs[1].paired = vec![Paired {
                    alive: true,
                    ..Paired::default()
                }];
            }
        }
        let mut tf = Frame {
            timestamp: FRAME_NS,
            lock_key: 5,
            count: 1,
        };
        pass(&mut encs, &mut tf, FRAME_NS);
        assert_eq!(encs[1].calls, 0, "{why}");
        assert_eq!((encs[0].lock_key[0], encs[0].next_key[0]), (5, 6), "{why}");
        assert_eq!((encs[2].lock_key[0], encs[2].next_key[0]), (6, 7), "{why}");
        assert_eq!(tf.lock_key, 7, "{why}");
    }
}

/// An encoder in a group that has not started waits until the frame is the
/// group's start; once started it no longer asks.
#[test]
fn steps_e2e_group_start() {
    let mut e = [enc(1)];
    e[0].group = Some(3 * FRAME_NS);
    for f in 1..=5u64 {
        let mut tf = frame(f * FRAME_NS);
        pass(&mut e, &mut tf, FRAME_NS);
    }
    assert_eq!(e[0].calls, 3);
    assert_eq!(e[0].frame_ts[0], 3 * FRAME_NS);
    assert_eq!(e[0].start_ts, 3 * FRAME_NS);

    // a frame past the group's start never matches
    let mut late = [enc(1)];
    late[0].group = Some(3 * FRAME_NS);
    let mut tf = frame(4 * FRAME_NS);
    pass(&mut late, &mut tf, FRAME_NS);
    assert_eq!(late[0].calls, 0);
}

/// The video waits while a paired audio encoder has not received audio or
/// its first audio is after the frame; a dead weak reference is skipped.
#[test]
fn steps_e2e_paired_wait() {
    let mut e = [enc(1)];
    e[0].first_received = false;
    e[0].paired = vec![
        Paired::default(),
        Paired {
            alive: true,
            first_received: true,
            first_raw_ts: 2 * FRAME_NS,
        },
        Paired {
            alive: true,
            first_received: true,
            first_raw_ts: 3 * FRAME_NS,
        },
    ];
    for f in 1..=4u64 {
        let mut tf = frame(f * FRAME_NS);
        pass(&mut e, &mut tf, FRAME_NS);
    }
    // frames 1 and 2 wait (paired[2] starts at 3), 3 and 4 encode
    assert_eq!(e[0].calls, 2);
    assert_eq!(e[0].frame_ts[0], 3 * FRAME_NS);

    // no audio yet on a live paired encoder: the video waits
    let mut w = [enc(1)];
    w[0].first_received = false;
    w[0].paired = vec![Paired {
        alive: true,
        ..Paired::default()
    }];
    let mut tf = frame(FRAME_NS);
    pass(&mut w, &mut tf, FRAME_NS);
    assert_eq!(w[0].calls, 0);

    // only dead paired encoders: nothing to wait for
    let mut d = [enc(1)];
    d[0].first_received = false;
    d[0].paired = vec![Paired::default(), Paired::default()];
    let mut tf2 = frame(FRAME_NS);
    pass(&mut d, &mut tf2, FRAME_NS);
    assert_eq!(d[0].calls, 1);
}

/// Characterized, not endorsed: the GPU path steps cur_pts after every
/// encode, failed or not; receive_video() steps it only after a successful
/// one. The failure stops the encoder either way, so the step is taken
/// once.
#[test]
fn steps_e2e_pts_after_failed_encode() {
    let mut e = [enc(2)];
    e[0].timebase_num = 1001;
    e[0].cur_pts = 7;
    e[0].fail_encode = true;
    for f in 1..=4u64 {
        let mut tf = frame(f * FRAME_NS);
        pass(&mut e, &mut tf, FRAME_NS);
    }
    assert_eq!(e[0].calls, 1);
    assert_eq!(e[0].pts[0], 7);
    assert!(e[0].failed);
    assert_eq!(e[0].cur_pts, 7 + 1001 * 2);
}

/// The timing entry is recorded only for a frame with a timestamp.
#[test]
fn steps_e2e_timing() {
    let mut e = [enc(1)];
    let mut tf = frame(0);
    pass(&mut e, &mut tf, FRAME_NS);
    assert_eq!(e[0].calls, 1);
    assert_eq!(e[0].timings, 0);
    tf.timestamp = FRAME_NS;
    tf.count = 1;
    pass(&mut e, &mut tf, FRAME_NS);
    assert_eq!(e[0].calls, 2);
    assert_eq!(e[0].timings, 1);
}

/// A frame with a count of n is encoded n times, one interval apart, then
/// goes to the available queue.
#[test]
fn steps_e2e_requeue() {
    for count in 1..=3i32 {
        let mut e = [enc(1)];
        let mut tf = Frame {
            timestamp: 10 * FRAME_NS,
            lock_key: 0,
            count,
        };
        let mut passes = 1;
        while pass(&mut e, &mut tf, FRAME_NS) {
            passes += 1;
        }
        let n = u64::try_from(count).unwrap();
        assert_eq!(passes, count);
        assert_eq!(tf.count, 0);
        assert_eq!(tf.timestamp, (10 + n - 1) * FRAME_NS);
        assert_eq!(e[0].calls, usize::try_from(count).unwrap());
        for c in 0..e[0].calls {
            assert_eq!(
                e[0].frame_ts[c],
                (10 + u64::try_from(c).unwrap()) * FRAME_NS
            );
        }
    }

    // near u64::MAX the timestamp wraps
    let mut e = [enc(1)];
    let mut tf = Frame {
        timestamp: u64::MAX - 5,
        lock_key: 0,
        count: 3,
    };
    assert!(pass(&mut e, &mut tf, FRAME_NS));
    assert_eq!(tf.timestamp, FRAME_NS - 6);
    assert_eq!(tf.count, 2);
    assert!(pass(&mut e, &mut tf, FRAME_NS));
    assert_eq!(tf.timestamp, 2 * FRAME_NS - 6);
    assert!(!pass(&mut e, &mut tf, FRAME_NS));
    assert_eq!(tf.timestamp, 2 * FRAME_NS - 6);

    // with no encoder the frame still counts down and requeues
    let mut none = Frame {
        timestamp: FRAME_NS,
        lock_key: 9,
        count: 2,
    };
    assert!(pass(&mut [], &mut none, FRAME_NS));
    assert_eq!(none.timestamp, 2 * FRAME_NS);
    assert_eq!(none.lock_key, 9);
    assert!(!pass(&mut [], &mut none, FRAME_NS));
}

// ------------------------------------------------------------------------
// Edge cases against models of the original C

/// The int edges the C test leaves out: i32::MIN, where the C's `- 1`
/// overflows (undefined), wraps here (intentional difference).
#[test]
fn edge_int_extremes() {
    assert_eq!(warm_next(i32::MIN), i32::MAX);
    assert_eq!(count_next(i32::MIN), i32::MAX);
    assert!(warming(i32::MIN));
    assert!(requeue(i32::MIN));
    for x in (i32::MIN..=i32::MIN + 1000)
        .chain(-1000..=1000)
        .chain(i32::MAX - 1000..=i32::MAX)
    {
        assert_eq!(warming(x), x != 0);
        assert_eq!(requeue(x), x != 0);
        assert_eq!(warm_next(x), x.wrapping_sub(1));
        assert_eq!(count_next(x), x.wrapping_sub(1));
    }
}

#[test]
fn edge_divisor_small() {
    // every counter and divisor below 64, and the counter sequence of
    // every divisor up to 64 over 200 frames against the C's expressions
    for c in 0..64u32 {
        for d in 0..64u32 {
            assert_eq!(divisor_wrap(c, d), c == d);
        }
    }
    for d in 0..64u32 {
        let (mut ours, mut c_counter) = (0u32, 0u32);
        for _ in 0..200 {
            // origin/master: skip = counter++; if (counter == divisor)
            // counter = 0; if (skip) continue;
            let c_skip = c_counter;
            c_counter = c_counter.wrapping_add(1);
            if c_counter == d {
                c_counter = 0;
            }

            let skip = ours;
            ours = divisor_inc(skip);
            if divisor_wrap(ours, d) {
                ours = 0;
            }
            assert_eq!(divisor_skip(skip), c_skip != 0);
            assert_eq!(ours, c_counter);
        }
    }
}

#[test]
fn edge_next_key_small() {
    for count in 0..16usize {
        for num in 0..16usize {
            for key in [0u64, 1, 7, u64::MAX - 1, u64::MAX] {
                // origin/master: if (++lock_count == encoders.num) next_key
                // = 0; else next_key++;
                let want = if count == num { 0 } else { key.wrapping_add(1) };
                assert_eq!(next_key(count, num, key), want);
            }
        }
    }
}

#[test]
fn edge_every_format() {
    for f in -1000..=1000 {
        assert_eq!(p010(f), f == 18, "{f}");
    }
}

// ------------------------------------------------------------------------
// The stand-in against origin/master's expressions inline

/// The pass with origin/master's expressions written out, as the C had
/// them before the move.
fn pass_inline(encs: &mut [Enc], tf: &mut Frame, interval: u64) -> bool {
    let timestamp = tf.timestamp;
    let mut lock_key = tf.lock_key;
    let mut key = tf.lock_key;
    let mut lock_count = 0usize;
    let num = encs.len();

    for encoder in encs.iter_mut() {
        if encoder.failed {
            continue;
        }
        if let Some(group_start) = encoder.group
            && encoder.start_ts == 0
            && group_start != timestamp
        {
            continue;
        }
        if !encoder.first_received && !encoder.paired.is_empty() {
            let mut wait_for_audio = false;
            for p in &encoder.paired {
                if wait_for_audio {
                    break;
                }
                if !p.alive {
                    continue;
                }
                if !p.first_received || p.first_raw_ts > timestamp {
                    wait_for_audio = true;
                }
            }
            if wait_for_audio {
                continue;
            }
        }
        if encoder.paused {
            continue;
        }
        let skip = encoder.frame_rate_divisor_counter;
        encoder.frame_rate_divisor_counter = encoder.frame_rate_divisor_counter.wrapping_add(1);
        if encoder.frame_rate_divisor_counter == encoder.frame_rate_divisor {
            encoder.frame_rate_divisor_counter = 0;
        }
        if skip != 0 {
            continue;
        }
        if encoder.start_ts == 0 {
            encoder.start_ts = timestamp;
        }
        lock_count += 1;
        if lock_count == num {
            key = 0;
        } else {
            key = key.wrapping_add(1);
        }
        let texture2 = encoder.has_texture2;
        let success = encode(encoder, timestamp, lock_key, &mut key, texture2);
        if tf.timestamp != 0 {
            encoder.timings += 1;
        }
        if !success {
            encoder.failed = true;
        }
        lock_key = key;
        encoder.cur_pts = encoder.cur_pts.wrapping_add(i64::from(
            encoder
                .timebase_num
                .wrapping_mul(encoder.frame_rate_divisor),
        ));
    }

    tf.lock_key = key;
    tf.count = tf.count.wrapping_sub(1);
    if tf.count != 0 {
        tf.timestamp = tf.timestamp.wrapping_add(interval);
        return true;
    }
    false
}

fn any_enc() -> impl Strategy<Value = Enc> {
    (
        (
            any::<bool>(),
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
            prop_oneof![1..=4u32, Just(0u32), any::<u32>()],
            prop_oneof![0..=4u32, any::<u32>()],
            prop_oneof![Just(1u32), Just(1001u32), any::<u32>()],
            any::<i64>(),
            any::<bool>(),
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
                    has_texture2,
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
                has_texture2,
                fail_encode,
                ..Enc::default()
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn edge_decisions_full_range(a in any::<u64>(), b in any::<u64>(), c32 in any::<u32>(),
                                 d32 in any::<u32>(), n in any::<usize>(), m in any::<usize>(),
                                 i in any::<i32>()) {
        prop_assert_eq!(warming(i), i != 0);
        prop_assert_eq!(warm_next(i), i.wrapping_sub(1));
        prop_assert_eq!(divisor_inc(c32), c32.wrapping_add(1));
        prop_assert_eq!(divisor_wrap(c32, d32), c32 == d32);
        prop_assert_eq!(divisor_wrap(c32, c32), true);
        prop_assert_eq!(divisor_skip(c32), c32 != 0);
        prop_assert_eq!(next_key(n, m, a), if n == m { 0 } else { a.wrapping_add(1) });
        prop_assert_eq!(next_key(n, n, a), 0);
        prop_assert_eq!(record_timing(a), a != 0);
        prop_assert_eq!(count_next(i), i.wrapping_sub(1));
        prop_assert_eq!(requeue(i), i != 0);
        prop_assert_eq!(requeue_ts(a, b), a.wrapping_add(b));
        prop_assert_eq!(p010(i), i == 18);
    }

    #[test]
    fn edge_stand_in_against_inline(encs in prop::collection::vec(any_enc(), 0..5),
                                    frames in prop::collection::vec(
                                        (prop_oneof![Just(0u64), Just(FRAME_NS), Just(u64::MAX - 3), any::<u64>()],
                                         any::<u64>(), -1..=4i32), 1..8),
                                    interval in prop_oneof![Just(FRAME_NS), Just(u64::MAX), any::<u64>()]) {
        let mut ours = encs.clone();
        let mut theirs = encs;
        for (timestamp, lock_key, count) in frames {
            let mut a = Frame { timestamp, lock_key, count };
            let mut b = a;
            // at most a few passes per frame
            for _ in 0..4 {
                let ra = pass(&mut ours, &mut a, interval);
                let rb = pass_inline(&mut theirs, &mut b, interval);
                prop_assert_eq!(ra, rb);
                prop_assert_eq!(a, b);
                if !ra {
                    break;
                }
            }
        }
        prop_assert_eq!(ours, theirs);
    }
}
