//! Tier 3 parity: the `coreaudio_pacing` core and its shims against the
//! original `coreaudio-pacing.c.h` (oracle/coreaudio_pacing.c).
//!
//! The unchanged C test `test_coreaudio_pacing_steps` compiles the C into
//! itself; `test_coreaudio_pacing_steps_rust` runs it against the shims, and
//! this file compares all three sides (oracle, shims, core) exactly: the
//! constant, every function over grids around every edge of every input, the
//! volume gate over every float near 1 and 1 +- `EPSILON`, a stand-in of the
//! monitor (`fill_buffer()`, `on_audio_playback()` after the resample and
//! `coreaudio_buffer_audio()`, asking the functions in the C's order) over
//! runs of audio pushes and buffer returns with and without failed enqueues,
//! and proptests.
//!
//! One intentional difference: `coreaudio_monitor_recover_failed_enqueue()`.
//! The C answers no (a buffer whose `AudioQueueEnqueueBuffer()` failed stays
//! out for good, so the queue, stopped by the failure, never pauses and is
//! never started again); the port answers yes (the buffer goes back on the
//! free list and the monitor takes the paused state with the full wait, so
//! the next audio starts the queue again). [`difference_is_only_recovery`]
//! pins it; every other comparison asks the oracle for everything else, and
//! the runs with failures give the oracle's stand-in the port's answer to
//! that one question, no more. No input is excluded: the C has no undefined
//! behaviour here (all its arithmetic is unsigned).

use obs_audio_monitoring::coreaudio_pacing as core;
use obs_audio_monitoring::ffi::coreaudio_pacing as shim;
use obs_c_oracle::coreaudio_pacing::{
    oracle_coreaudio_monitor_all_back, oracle_coreaudio_monitor_buffer_size,
    oracle_coreaudio_monitor_buffers, oracle_coreaudio_monitor_can_fill,
    oracle_coreaudio_monitor_needs_volume, oracle_coreaudio_monitor_recover_failed_enqueue,
    oracle_coreaudio_monitor_wait_over, oracle_coreaudio_monitor_wait_size,
};
use proptest::prelude::*;

const PTR: usize = size_of::<*const ()>();

// ---- the three sides, one function each ----

fn o_buffer_size(c: u32, r: u32) -> usize {
    // SAFETY: plain values; the oracle reads nothing else.
    unsafe { oracle_coreaudio_monitor_buffer_size(c, r) }
}
fn o_wait_size(b: usize) -> usize {
    // SAFETY: as above.
    unsafe { oracle_coreaudio_monitor_wait_size(b) }
}
fn o_can_fill(q: usize, b: usize) -> bool {
    // SAFETY: as above.
    unsafe { oracle_coreaudio_monitor_can_fill(q, b) }
}
fn o_needs_volume(v: f32) -> bool {
    // SAFETY: as above.
    unsafe { oracle_coreaudio_monitor_needs_volume(v) }
}
fn o_wait_over(q: usize, w: usize) -> bool {
    // SAFETY: as above.
    unsafe { oracle_coreaudio_monitor_wait_over(q, w) }
}
fn o_all_back(e: usize, p: usize) -> bool {
    // SAFETY: as above.
    unsafe { oracle_coreaudio_monitor_all_back(e, p) }
}
fn o_recover() -> bool {
    // SAFETY: no arguments.
    unsafe { oracle_coreaudio_monitor_recover_failed_enqueue() }
}

/// Every side agrees on one function of these arguments.
macro_rules! same {
    ($o:ident, $s:ident, $c:ident, $($a:expr),+) => {{
        let o = $o($($a),+);
        assert_eq!(shim::$s($($a),+), o, "shim {}{:?}", stringify!($s), ($($a),+));
        assert_eq!(core::$c($($a),+), o, "core {}{:?}", stringify!($c), ($($a),+));
    }};
}

#[test]
fn difference_is_only_recovery() {
    assert!(!o_recover());
    assert!(shim::coreaudio_monitor_recover_failed_enqueue());
    assert!(core::recover_failed_enqueue());
}

#[test]
fn constant_matches() {
    // SAFETY: no arguments.
    assert_eq!(unsafe { oracle_coreaudio_monitor_buffers() }, core::BUFFERS);
}

const U32_EDGES: [u32; 14] = [
    0,
    1,
    24,
    25,
    26,
    100,
    44100,
    48000,
    192_000,
    0x3fff_ffff,
    0x4000_0000,
    0x7fff_ffff,
    0x8000_0000,
    u32::MAX,
];

/// +-3 around every edge of a size: the small counts, the pointer
/// multiples, the buffer sizes and the top of `usize`.
fn sizes() -> Vec<usize> {
    let mut v = Vec::new();
    let bases = [
        0,
        4,
        8,
        12,
        16,
        24,
        32,
        5292,
        11520,
        15876,
        34560,
        usize::MAX / 3,
        usize::MAX / 2,
        usize::MAX,
    ];
    for b in bases {
        for d in 0..=3 {
            v.push(b.wrapping_add(d));
            v.push(b.wrapping_sub(d));
        }
    }
    v.sort_unstable();
    v.dedup();
    v
}

#[test]
fn sizes_match() {
    for c in U32_EDGES {
        for r in U32_EDGES {
            for dc in 0..=2u32 {
                for dr in 0..=2u32 {
                    let (c, r) = (c.wrapping_add(dc), r.wrapping_add(dr));
                    same!(
                        o_buffer_size,
                        coreaudio_monitor_buffer_size,
                        buffer_size,
                        c,
                        r
                    );
                }
            }
        }
    }
    for b in sizes() {
        same!(o_wait_size, coreaudio_monitor_wait_size, wait_size, b);
    }
}

#[test]
fn tests_match() {
    let s = sizes();
    for &a in &s {
        for &b in &s {
            same!(o_can_fill, coreaudio_monitor_can_fill, can_fill, a, b);
            same!(o_wait_over, coreaudio_monitor_wait_over, wait_over, a, b);
            same!(o_all_back, coreaudio_monitor_all_back, all_back, a, b);
        }
    }
    for e in 0..=64 {
        for p in 0..=16 {
            same!(o_all_back, coreaudio_monitor_all_back, all_back, e, p);
        }
    }
}

#[test]
fn volume_matches() {
    // every float within 4096 ULPs of 1 and of 1 +- EPSILON
    for centre in [1.0f32, 1.0 + 1e-4, 1.0 - 1e-4] {
        let bits = centre.to_bits();
        for d in -4096i32..=4096 {
            let v = f32::from_bits(bits.wrapping_add_signed(d));
            same!(
                o_needs_volume,
                coreaudio_monitor_needs_volume,
                needs_volume,
                v
            );
        }
    }
    for v in [
        0.0,
        -0.0,
        -1.0,
        2.0,
        f32::MIN_POSITIVE,
        f32::MAX,
        f32::MIN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        -f32::NAN,
        f32::from_bits(0x7f80_0001),
        f32::from_bits(0xffc0_1234),
    ] {
        same!(
            o_needs_volume,
            coreaudio_monitor_needs_volume,
            needs_volume,
            v
        );
    }
}

// ---- the stand-in monitor ----

/// The decisions a stand-in asks.
#[derive(Clone, Copy)]
struct Rules {
    buffer_size: fn(u32, u32) -> usize,
    wait_size: fn(usize) -> usize,
    can_fill: fn(usize, usize) -> bool,
    wait_over: fn(usize, usize) -> bool,
    all_back: fn(usize, usize) -> bool,
    recover: fn() -> bool,
}

const ORACLE: Rules = Rules {
    buffer_size: o_buffer_size,
    wait_size: o_wait_size,
    can_fill: o_can_fill,
    wait_over: o_wait_over,
    all_back: o_all_back,
    recover: o_recover,
};

/// The oracle with the port's one intentional difference.
const ORACLE_RECOVERING: Rules = Rules {
    recover: || true,
    ..ORACLE
};

const SHIM: Rules = Rules {
    buffer_size: |c, r| shim::coreaudio_monitor_buffer_size(c, r),
    wait_size: |b| shim::coreaudio_monitor_wait_size(b),
    can_fill: |q, b| shim::coreaudio_monitor_can_fill(q, b),
    wait_over: |q, w| shim::coreaudio_monitor_wait_over(q, w),
    all_back: |e, p| shim::coreaudio_monitor_all_back(e, p),
    recover: || shim::coreaudio_monitor_recover_failed_enqueue(),
};

const CORE: Rules = Rules {
    buffer_size: core::buffer_size,
    wait_size: core::wait_size,
    can_fill: core::can_fill,
    wait_over: core::wait_over,
    all_back: core::all_back,
    recover: core::recover_failed_enqueue,
};

/// What the AudioQueue saw: an enqueue (buffer id, failed), a start, a
/// pause, a stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    Enqueue(usize, bool),
    Start,
    Pause,
    Stop,
}

/// `struct audio_monitor`'s pacing state, the AudioQueue and the calls made.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Monitor {
    buffer_size: usize,
    wait_size: usize,
    queued: usize,
    paused: bool,
    free: Vec<usize>,
    queue: Vec<usize>,
    calls: Vec<Call>,
}

impl Monitor {
    fn new(r: Rules, channels: u32, rate: u32) -> Self {
        let buffer_size = (r.buffer_size)(channels, rate);
        Self {
            buffer_size,
            wait_size: (r.wait_size)(buffer_size),
            free: (0..core::BUFFERS).collect(),
            ..Self::default()
        }
    }

    /// `fill_buffer()`; `fail` decides each enqueue.
    fn fill(&mut self, r: Rules, fail: &mut dyn FnMut() -> bool) -> bool {
        if !(r.can_fill)(self.queued, self.buffer_size) {
            return false;
        }
        let id = self.free.remove(0);
        self.queued -= self.buffer_size;
        let failed = fail();
        self.calls.push(Call::Enqueue(id, failed));
        if failed {
            self.calls.push(Call::Stop);
            if (r.recover)() {
                self.free.push(id);
                self.paused = true;
                self.wait_size = (r.wait_size)(self.buffer_size);
            }
        } else {
            self.queue.push(id);
        }
        true
    }

    fn fill_loop(&mut self, r: Rules, fail: &mut dyn FnMut() -> bool) {
        while !self.free.is_empty() {
            if !self.fill(r, fail) {
                break;
            }
        }
    }

    /// `on_audio_playback()` after the resample.
    fn play(&mut self, r: Rules, bytes: usize, fail: &mut dyn FnMut() -> bool) {
        self.queued += bytes;
        if (r.wait_over)(self.queued, self.wait_size) {
            self.wait_size = 0;
            self.fill_loop(r, fail);
            if self.paused {
                self.calls.push(Call::Start);
                self.paused = false;
            }
        }
    }

    /// `coreaudio_buffer_audio()` for queued buffer `at`.
    fn back(&mut self, r: Rules, at: usize, fail: &mut dyn FnMut() -> bool) {
        let id = self.queue.remove(at);
        self.free.push(id);
        self.fill_loop(r, fail);
        if (r.all_back)(self.free.len() * PTR, PTR) {
            self.paused = true;
            self.wait_size = (r.wait_size)(self.buffer_size);
            self.calls.push(Call::Pause);
        }
    }

    /// `on_audio_pause()`: the queued audio is dropped.
    fn drop_audio(&mut self) {
        self.queued = 0;
    }
}

#[derive(Clone, Debug)]
enum Op {
    Play(usize),
    Back(usize),
    Drop,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        6 => (1usize..60_000).prop_map(Op::Play),
        5 => any::<usize>().prop_map(Op::Back),
        1 => Just(Op::Drop),
    ]
}

/// The monitor after each step; enqueue `n` fails when `fails[n % len]`.
fn run(r: Rules, channels: u32, rate: u32, ops: &[Op], fails: &[bool]) -> Vec<Monitor> {
    let mut m = Monitor::new(r, channels, rate);
    let mut n = 0usize;
    let mut fail = || {
        let f = !fails.is_empty() && fails[n % fails.len()];
        n += 1;
        f
    };
    let mut trace = vec![m.clone()];
    for o in ops {
        match *o {
            Op::Play(b) => m.play(r, b, &mut fail),
            Op::Back(at) => {
                if m.queue.is_empty() {
                    continue;
                }
                let at = at % m.queue.len();
                m.back(r, at, &mut fail);
            }
            Op::Drop => m.drop_audio(),
        }
        trace.push(m.clone());
    }
    trace
}

const RATES: [u32; 6] = [8000, 22050, 44100, 44101, 48000, 192_000];

#[test]
fn runs_match_on_fixed_patterns() {
    // Pushes of every size around a buffer, and returns in turn, with
    // failures never, on every third enqueue, on every one, and on every
    // other one.
    let patterns: [&[bool]; 4] = [&[], &[false, false, true], &[true], &[true, false]];
    for channels in [1, 2, 8] {
        for rate in RATES {
            let buf = core::buffer_size(channels, rate);
            let mut ops = Vec::new();
            for k in [
                1,
                buf / 2,
                buf - 1,
                buf,
                buf + 1,
                3 * buf - 1,
                3 * buf,
                7 * buf,
            ] {
                ops.push(Op::Play(k));
                ops.push(Op::Back(0));
                ops.push(Op::Back(1));
            }
            for _ in 0..4 {
                ops.push(Op::Back(0));
            }
            ops.push(Op::Drop);
            ops.push(Op::Play(3 * buf));
            for fails in patterns {
                let s = run(SHIM, channels, rate, &ops, fails);
                assert_eq!(s, run(CORE, channels, rate, &ops, fails));
                if fails.is_empty() {
                    assert_eq!(
                        s,
                        run(ORACLE, channels, rate, &ops, fails),
                        "{channels} at {rate}"
                    );
                } else {
                    assert_eq!(
                        s,
                        run(ORACLE_RECOVERING, channels, rate, &ops, fails),
                        "{channels} at {rate}, fails {fails:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_c_loses_a_buffer_and_the_port_does_not() {
    // One failed enqueue, then the buffers come back and audio follows.
    let mut ops = vec![Op::Play(34560)];
    for _ in 0..3 {
        ops.push(Op::Back(0));
    }
    ops.push(Op::Play(34560));
    let fails = [true, false, false, false, false, false, false];
    let last = |t: Vec<Monitor>| t.last().cloned().unwrap_or_default();
    let o = last(run(ORACLE, 2, 48000, &ops, &fails));
    let s = last(run(SHIM, 2, 48000, &ops, &fails));
    let starts = |m: &Monitor| m.calls.iter().filter(|c| **c == Call::Start).count();
    let pauses = |m: &Monitor| m.calls.iter().filter(|c| **c == Call::Pause).count();

    // The C: two buffers in play, no pause, no start after the stop.
    assert_eq!(o.free.len() + o.queue.len(), 2);
    assert_eq!(pauses(&o), 0);
    assert_eq!(starts(&o), 0);
    // The port: all three, a start straight after the failure, and again
    // after the pause once every buffer was back.
    assert_eq!(s.queue.len(), 3);
    assert_eq!(starts(&s), 2);
    assert_eq!(pauses(&s), 1);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn prop_sizes(c in any::<u32>(), r in any::<u32>(), b in any::<usize>()) {
        prop_assert_eq!(shim::coreaudio_monitor_buffer_size(c, r), o_buffer_size(c, r));
        prop_assert_eq!(core::buffer_size(c, r), o_buffer_size(c, r));
        prop_assert_eq!(shim::coreaudio_monitor_wait_size(b), o_wait_size(b));
        prop_assert_eq!(core::wait_size(b), o_wait_size(b));
    }

    #[test]
    fn prop_tests(a in any::<usize>(), b in any::<usize>(), e in 0usize..128, p in 0usize..32) {
        prop_assert_eq!(shim::coreaudio_monitor_can_fill(a, b), o_can_fill(a, b));
        prop_assert_eq!(core::can_fill(a, b), o_can_fill(a, b));
        prop_assert_eq!(shim::coreaudio_monitor_wait_over(a, b), o_wait_over(a, b));
        prop_assert_eq!(core::wait_over(a, b), o_wait_over(a, b));
        prop_assert_eq!(shim::coreaudio_monitor_all_back(a, b), o_all_back(a, b));
        prop_assert_eq!(core::all_back(e, p), o_all_back(e, p));
        prop_assert_eq!(shim::coreaudio_monitor_all_back(e, p), o_all_back(e, p));
    }

    #[test]
    fn prop_volume(bits in any::<u32>()) {
        let v = f32::from_bits(bits);
        prop_assert_eq!(shim::coreaudio_monitor_needs_volume(v), o_needs_volume(v));
        prop_assert_eq!(core::needs_volume(v), o_needs_volume(v));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(2000)))]

    /// Runs of pushes, returns in any order and dropped audio: without
    /// failed enqueues all three sides step alike; with them, the shims and
    /// the core step as the oracle given the port's answer to the failed
    /// enqueue, and keep every buffer.
    #[test]
    fn prop_runs(
        channels in 1u32..=8,
        rate in prop_oneof![
            proptest::sample::select(RATES.to_vec()),
            8000u32..=192_000,
        ],
        ops in proptest::collection::vec(op(), 0..160),
        fails in proptest::collection::vec(proptest::bool::weighted(0.2), 0..8),
    ) {
        let s = run(SHIM, channels, rate, &ops, &fails);
        prop_assert_eq!(&s, &run(CORE, channels, rate, &ops, &fails));
        if fails.iter().any(|f| *f) {
            prop_assert_eq!(&s, &run(ORACLE_RECOVERING, channels, rate, &ops, &fails));
            for m in &s {
                prop_assert_eq!(m.free.len() + m.queue.len(), core::BUFFERS);
            }
        } else {
            prop_assert_eq!(&s, &run(ORACLE, channels, rate, &ops, &fails));
        }
    }
}
