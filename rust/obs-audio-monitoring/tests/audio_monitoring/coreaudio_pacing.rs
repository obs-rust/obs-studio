//! Tier 1: `obs_audio_monitoring::coreaudio_pacing`, the CoreAudio monitor's
//! queue pacing.
//!
//! The `steps_*` cases port `test/cmocka/test_coreaudio_pacing_steps.c` 1:1:
//! its 12 shared cases and the 3 of its `COREAUDIO_PACING_TEST_RUST` build
//! (the port's intentional difference), under the same names, in the same
//! order, with the same values and expectations. The C test's other three
//! cases pin the C's failed enqueue, which the port changes: they are
//! `edge_c_*` here, against the model of the C below. The C calls the hidden
//! `coreaudio_monitor_*` functions; here the safe core is called directly.
//! The C's stand-in monitor is [`Sim`].
//!
//! The `edge_*` cases are new: each function against a model of the
//! origin/master C (its expressions inline) over edge values, and proptests
//! over full-range inputs and over runs of pushes and returns, the stand-in
//! on the core against the stand-in on the model.

use obs_audio_monitoring::coreaudio_pacing::{
    BUFFERS, all_back, buffer_size, can_fill, needs_volume, recover_failed_enqueue, wait_over,
    wait_size,
};
use proptest::prelude::*;

const PTR: usize = size_of::<*const ()>();

// ---- models of the origin/master C, expressions inline ----

/// `channels * sizeof(float) * info->samples_per_sec / 100 * 3`
fn c_buffer_size(channels: u32, rate: u32) -> usize {
    (channels as usize)
        .wrapping_mul(4)
        .wrapping_mul(rate as usize)
        / 100
        * 3
}

/// `monitor->buffer_size * 3`
fn c_wait_size(buffer_size: usize) -> usize {
    buffer_size.wrapping_mul(3)
}

/// `!(monitor->new_data.size < monitor->buffer_size)`
fn c_can_fill(queued: usize, buffer_size: usize) -> bool {
    queued >= buffer_size
}

/// `!close_float(vol, 1.0f, EPSILON)`, negated as the C does: NaN is never
/// close.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn c_needs_volume(vol: f32) -> bool {
    !((vol - 1.0).abs() <= 1e-4)
}

/// `monitor->new_data.size >= monitor->wait_size`
fn c_wait_over(queued: usize, wait_size: usize) -> bool {
    queued >= wait_size
}

/// `monitor->empty_buffers.size == sizeof(buf) * 3`
fn c_all_back(empty_bytes: usize, ptr_size: usize) -> bool {
    empty_bytes == ptr_size.wrapping_mul(3)
}

/// The decisions a stand-in asks: the core's, or the C's.
#[derive(Clone, Copy)]
struct Rules {
    buffer_size: fn(u32, u32) -> usize,
    wait_size: fn(usize) -> usize,
    can_fill: fn(usize, usize) -> bool,
    wait_over: fn(usize, usize) -> bool,
    all_back: fn(usize, usize) -> bool,
    recover: fn() -> bool,
}

const CORE: Rules = Rules {
    buffer_size,
    wait_size,
    can_fill,
    wait_over,
    all_back,
    recover: recover_failed_enqueue,
};

const C: Rules = Rules {
    buffer_size: c_buffer_size,
    wait_size: c_wait_size,
    can_fill: c_can_fill,
    wait_over: c_wait_over,
    all_back: c_all_back,
    recover: || false,
};

// ---- the C test's stand-in monitor ----

/// The C test's `struct sim`: the monitor as `coreaudio-output.c` drives it,
/// with buffer ids for the AudioQueue buffers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Sim {
    buffer_size: usize,
    wait_size: usize,
    queued: usize,
    paused: bool,
    free_ids: Vec<usize>,
    queue_ids: Vec<usize>,
    /// The next enqueues to fail.
    fail_enqueues: u32,
    enqueues: u32,
    failed: u32,
    starts: u32,
    pauses: u32,
    stops: u32,
}

impl Sim {
    /// `sim_init()`
    fn new(rules: Rules, channels: u32, rate: u32) -> Self {
        let buffer_size = (rules.buffer_size)(channels, rate);
        Self {
            buffer_size,
            wait_size: (rules.wait_size)(buffer_size),
            free_ids: (0..BUFFERS).collect(),
            ..Self::default()
        }
    }

    /// `fill_buffer()`
    fn fill(&mut self, rules: Rules) -> bool {
        if !(rules.can_fill)(self.queued, self.buffer_size) {
            return false;
        }
        let id = self.free_ids.remove(0);
        self.queued -= self.buffer_size;
        self.enqueues += 1;

        if self.fail_enqueues > 0 {
            self.fail_enqueues -= 1;
            self.failed += 1;
            self.stops += 1;
            if (rules.recover)() {
                assert!(self.free_ids.len() < BUFFERS);
                self.free_ids.push(id);
                self.paused = true;
                self.wait_size = (rules.wait_size)(self.buffer_size);
            }
        } else {
            self.queue_ids.push(id);
        }
        true
    }

    fn fill_loop(&mut self, rules: Rules) {
        while !self.free_ids.is_empty() {
            if !self.fill(rules) {
                break;
            }
        }
    }

    /// `on_audio_playback()`, after the resample: `bytes` of audio pushed.
    fn play(&mut self, rules: Rules, bytes: usize) {
        self.queued += bytes;
        if (rules.wait_over)(self.queued, self.wait_size) {
            self.wait_size = 0;
            self.fill_loop(rules);
            if self.paused {
                self.starts += 1;
                self.paused = false;
            }
        }
    }

    /// `coreaudio_buffer_audio()`: the oldest queued buffer comes back.
    fn back(&mut self, rules: Rules) {
        let id = self.queue_ids.remove(0);
        self.free_ids.push(id);
        self.fill_loop(rules);
        if (rules.all_back)(self.free_ids.len() * PTR, PTR) {
            self.paused = true;
            self.wait_size = (rules.wait_size)(self.buffer_size);
            self.pauses += 1;
        }
    }
}

// ---- test_coreaudio_pacing_steps.c, 1:1 ----

#[test]
fn steps_the_constants() {
    assert_eq!(BUFFERS, 3);
}

#[test]
fn steps_the_buffer_is_30_ms_of_float_audio() {
    assert_eq!(buffer_size(1, 44100), 5292);
    assert_eq!(buffer_size(2, 44100), 10584);
    assert_eq!(buffer_size(8, 44100), 42336);
    assert_eq!(buffer_size(1, 48000), 5760);
    assert_eq!(buffer_size(2, 48000), 11520);
    assert_eq!(buffer_size(8, 48000), 46080);
    assert_eq!(buffer_size(6, 96000), 69120);
    assert_eq!(buffer_size(2, 192_000), 46080);
}

#[test]
fn steps_the_buffer_size_divides_after_multiplying() {
    assert_eq!(buffer_size(1, 44101), 5292);
    assert_eq!(buffer_size(1, 22051), 2646);
    assert_eq!(buffer_size(3, 22051), 7938);
    assert_eq!(buffer_size(1, 25), 3);
    assert_eq!(buffer_size(1, 26), 3);
    assert_eq!(buffer_size(1, 50), 6);
    assert_eq!(buffer_size(3, 25), 9);

    // Characterized, not endorsed.
    assert_eq!(buffer_size(1, 24), 0);
    assert_eq!(buffer_size(1, 0), 0);
    assert_eq!(buffer_size(0, 48000), 0);

    if usize::BITS == 64 {
        assert_eq!(
            buffer_size(u32::MAX, u32::MAX) as u64,
            0x07ae_147a_a3d7_0a3c
        );
        assert_eq!(buffer_size(8, u32::MAX) as u64, 0xf5c2_8f5a);
    }
}

#[test]
fn steps_the_wait_is_every_buffer() {
    assert_eq!(wait_size(0), 0);
    assert_eq!(wait_size(1), 3);
    assert_eq!(wait_size(5292), 15876);
    assert_eq!(wait_size(11520), 34560);
    assert_eq!(wait_size(usize::MAX), usize::MAX - 2);
}

#[test]
fn steps_a_buffer_fills_from_a_whole_buffer() {
    assert!(!can_fill(0, 1));
    assert!(!can_fill(11519, 11520));
    assert!(can_fill(11520, 11520));
    assert!(can_fill(11521, 11520));
    assert!(can_fill(usize::MAX, 11520));
    assert!(!can_fill(usize::MAX - 1, usize::MAX));
    assert!(can_fill(usize::MAX, usize::MAX));
    // Characterized, not endorsed.
    assert!(can_fill(0, 0));
}

#[test]
fn steps_the_volume_step_runs_unless_the_volume_is_1() {
    assert!(!needs_volume(1.0));
    assert!(!needs_volume(1.000_09));
    assert!(!needs_volume(0.999_91));
    assert!(!needs_volume(1.000_099_9));
    assert!(!needs_volume(0.999_900_1));
    assert!(needs_volume(1.0001));
    assert!(needs_volume(0.9999));
    assert!(needs_volume(0.0));
    assert!(needs_volume(0.5));
    assert!(needs_volume(2.0));
    assert!(needs_volume(-1.0));
    assert!(needs_volume(f32::INFINITY));
    assert!(needs_volume(f32::NEG_INFINITY));
    assert!(needs_volume(f32::NAN));
}

#[test]
fn steps_the_wait_ends_at_the_wait_size() {
    assert!(!wait_over(34559, 34560));
    assert!(wait_over(34560, 34560));
    assert!(wait_over(34561, 34560));
    assert!(wait_over(0, 0));
    assert!(wait_over(1, 0));
    assert!(!wait_over(usize::MAX - 1, usize::MAX));
    assert!(wait_over(usize::MAX, usize::MAX));
}

#[test]
fn steps_the_queue_pauses_at_exactly_three_buffers() {
    assert!(!all_back(0, 8));
    assert!(!all_back(8, 8));
    assert!(!all_back(16, 8));
    assert!(all_back(24, 8));
    assert!(!all_back(32, 8));
    assert!(!all_back(23, 8));
    assert!(!all_back(25, 8));
    assert!(all_back(12, 4));
    assert!(!all_back(24, 4));
    assert!(!all_back(8, 4));
    assert!(all_back(3 * PTR, PTR));
    assert!(all_back(usize::MAX - 2, usize::MAX));
    assert!(all_back(0, 0));
}

#[test]
fn steps_the_monitor_waits_for_three_buffers_then_plays() {
    let r = CORE;
    let mut s = Sim::new(r, 2, 48000);
    assert_eq!(s.buffer_size, 11520);
    assert_eq!(s.wait_size, 34560);

    s.play(r, 11520);
    s.play(r, 11520);
    assert_eq!(s.enqueues, 0);
    assert_eq!(s.wait_size, 34560);

    s.play(r, 11519);
    assert_eq!(s.enqueues, 0);

    s.play(r, 1);
    assert_eq!(s.enqueues, 3);
    assert_eq!(s.queue_ids.len(), 3);
    assert_eq!(s.free_ids.len(), 0);
    assert_eq!(s.queued, 0);
    assert_eq!(s.wait_size, 0);
    assert_eq!(s.starts, 0);
    assert!(!s.paused);

    s.play(r, 11519);
    s.back(r);
    assert_eq!(s.enqueues, 3);
    assert_eq!(s.free_ids.len(), 1);
    s.play(r, 1);
    assert_eq!(s.enqueues, 4);
    assert_eq!(s.free_ids.len(), 0);
    s.play(r, 11520 * 2 + 7);
    s.back(r);
    s.back(r);
    assert_eq!(s.enqueues, 6);
    assert_eq!(s.queued, 7);
    assert_eq!(s.pauses, 0);
}

#[test]
fn steps_the_queue_pauses_when_every_buffer_is_back() {
    let r = CORE;
    let mut s = Sim::new(r, 1, 44100);
    s.play(r, s.wait_size);
    assert_eq!(s.queue_ids.len(), 3);

    s.back(r);
    s.back(r);
    assert_eq!(s.free_ids.len(), 2);
    assert_eq!(s.pauses, 0);
    assert!(!s.paused);
    assert_eq!(s.wait_size, 0);

    s.back(r);
    assert_eq!(s.free_ids.len(), 3);
    assert_eq!(s.pauses, 1);
    assert!(s.paused);
    assert_eq!(s.wait_size, 15876);

    s.play(r, 15875);
    assert_eq!(s.starts, 0);
    assert!(s.paused);
    s.play(r, 1);
    assert_eq!(s.starts, 1);
    assert!(!s.paused);
    assert_eq!(s.wait_size, 0);
    assert_eq!(s.queue_ids.len(), 3);
}

#[test]
fn steps_the_wait_clears_without_a_pause() {
    let r = CORE;
    let mut s = Sim::new(r, 1, 48000);
    s.play(r, s.wait_size + 5);
    assert_eq!(s.wait_size, 0);
    assert_eq!(s.starts, 0);
    assert_eq!(s.queued, 5);

    s.back(r);
    s.play(r, 1);
    assert_eq!(s.wait_size, 0);
    assert_eq!(s.free_ids.len(), 1);
    assert_eq!(s.starts, 0);
}

#[test]
fn steps_a_refill_on_return_can_keep_the_queue_from_pausing() {
    let r = CORE;
    let mut s = Sim::new(r, 2, 44100);
    s.play(r, s.wait_size + s.buffer_size * 2);
    assert_eq!(s.queued, s.buffer_size * 2);

    for _ in 0..2 {
        s.back(r);
        assert_eq!(s.free_ids.len(), 0);
    }
    assert_eq!(s.enqueues, 5);
    for _ in 0..3 {
        s.back(r);
    }
    assert_eq!(s.pauses, 1);
    assert!(s.paused);
}

#[test]
fn steps_the_port_recovers_a_failed_enqueue() {
    assert!(recover_failed_enqueue());
}

#[test]
fn steps_the_port_restarts_the_queue_after_a_failed_enqueue() {
    let r = CORE;
    let mut s = Sim::new(r, 2, 48000);
    s.fail_enqueues = 1;
    s.play(r, s.wait_size);

    assert_eq!(s.failed, 1);
    assert_eq!(s.stops, 1);
    assert_eq!(s.enqueues, 3);
    assert_eq!(s.queue_ids.len(), 2);
    assert_eq!(s.free_ids, vec![0]);
    assert_eq!(s.starts, 1);
    assert!(!s.paused);
    assert_eq!(s.wait_size, 34560);

    s.back(r);
    s.back(r);
    assert_eq!(s.free_ids.len(), 3);
    assert_eq!(s.pauses, 1);
    assert!(s.paused);

    s.play(r, s.wait_size);
    assert_eq!(s.queue_ids.len(), 3);
    assert_eq!(s.starts, 2);
    assert!(!s.paused);
}

#[test]
fn steps_the_port_waits_after_a_failed_refill() {
    let r = CORE;
    let mut s = Sim::new(r, 1, 48000);
    s.play(r, s.wait_size + s.buffer_size);

    s.fail_enqueues = 1;
    s.back(r);
    assert_eq!(s.failed, 1);
    assert_eq!(s.free_ids.len(), 1);
    assert!(s.paused);
    assert_eq!(s.wait_size, 17280);
    assert_eq!(s.pauses, 0);

    s.play(r, 17279);
    assert_eq!(s.starts, 0);
    s.play(r, 1);
    assert_eq!(s.starts, 1);
    assert_eq!(s.queue_ids.len(), 3);
    assert_eq!(s.wait_size, 0);
}

// ---- the C's failed enqueue, which the port changes ----

#[test]
fn edge_c_keeps_a_failed_buffer_out() {
    assert!(!(C.recover)());
    assert_ne!((C.recover)(), recover_failed_enqueue());
}

#[test]
fn edge_c_never_restarts_after_a_failed_enqueue() {
    let r = C;
    let mut s = Sim::new(r, 2, 48000);
    s.fail_enqueues = 1;
    s.play(r, s.wait_size);
    assert_eq!(s.failed, 1);
    assert_eq!(s.stops, 1);
    assert_eq!(s.queue_ids.len(), 2);
    assert_eq!(s.free_ids.len(), 0);
    assert_eq!(s.starts, 0);
    assert!(!s.paused);
    assert_eq!(s.wait_size, 0);

    s.back(r);
    s.back(r);
    assert_eq!(s.free_ids.len(), 2);
    assert_eq!(s.pauses, 0);
    for _ in 0..10 {
        s.play(r, s.buffer_size * 3);
    }
    assert_eq!(s.starts, 0);
    assert_eq!(s.pauses, 0);
}

#[test]
fn edge_c_keeps_filling_after_a_failed_enqueue() {
    let r = C;
    let mut s = Sim::new(r, 1, 48000);
    s.fail_enqueues = 2;
    s.play(r, s.wait_size);
    assert_eq!(s.enqueues, 3);
    assert_eq!(s.failed, 2);
    assert_eq!(s.stops, 2);
    assert_eq!(s.queue_ids.len(), 1);
    assert_eq!(s.free_ids.len(), 0);
    assert_eq!(s.queued, 0);
}

// ---- new: each function against the C model ----

const U32_EDGES: [u32; 18] = [
    0,
    1,
    2,
    3,
    7,
    8,
    24,
    25,
    26,
    99,
    100,
    101,
    44100,
    44101,
    48000,
    0x7fff_ffff,
    0x8000_0000,
    u32::MAX,
];

fn usize_edges() -> Vec<usize> {
    let mut v = Vec::new();
    for base in [
        0,
        3,
        4,
        8,
        12,
        16,
        24,
        32,
        5292,
        11520,
        34560,
        usize::MAX / 3,
        usize::MAX / 2,
        usize::MAX,
    ] {
        for d in 0..=2 {
            v.push(base.wrapping_add(d));
            v.push(base.wrapping_sub(d));
        }
    }
    v.sort_unstable();
    v.dedup();
    v
}

#[test]
fn edge_sizes_against_c() {
    for c in U32_EDGES {
        for r in U32_EDGES {
            assert_eq!(
                buffer_size(c, r),
                c_buffer_size(c, r),
                "{c} channels at {r}"
            );
        }
    }
    for b in usize_edges() {
        assert_eq!(wait_size(b), c_wait_size(b), "{b}");
    }
}

#[test]
fn edge_tests_against_c() {
    let edges = usize_edges();
    for &a in &edges {
        for &b in &edges {
            assert_eq!(can_fill(a, b), c_can_fill(a, b), "can_fill({a}, {b})");
            assert_eq!(wait_over(a, b), c_wait_over(a, b), "wait_over({a}, {b})");
            assert_eq!(all_back(a, b), c_all_back(a, b), "all_back({a}, {b})");
        }
    }
}

#[test]
fn edge_volume_against_c() {
    let one = 1.0_f32;
    let mut vols = vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        2.0,
        1e-4,
        f32::MIN_POSITIVE,
        f32::MAX,
        f32::MIN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ];
    // every float within a few ULPs of 1 +- EPSILON, and of 1
    for centre in [one, one + 1e-4, one - 1e-4] {
        let mut up = centre;
        let mut down = centre;
        for _ in 0..8 {
            vols.push(up);
            vols.push(down);
            up = f32::from_bits(up.to_bits() + 1);
            down = f32::from_bits(down.to_bits() - 1);
        }
    }
    for v in vols {
        assert_eq!(needs_volume(v), c_needs_volume(v), "{v:e}");
    }
}

/// One step of a run: audio pushed, or the oldest buffer back, with the
/// enqueues it makes failing or not.
#[derive(Clone, Debug)]
enum Op {
    Play(usize, u32),
    Back(u32),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0usize..40_000, 0u32..3).prop_map(|(b, f)| Op::Play(b, f)),
        (0u32..3).prop_map(Op::Back),
    ]
}

fn run(rules: Rules, channels: u32, rate: u32, ops: &[Op], failures: bool) -> Vec<Sim> {
    let mut s = Sim::new(rules, channels, rate);
    let mut trace = vec![s.clone()];
    for o in ops {
        match *o {
            Op::Play(bytes, fail) => {
                s.fail_enqueues = if failures && fail == 0 { 1 } else { 0 };
                s.play(rules, bytes);
            }
            Op::Back(fail) => {
                if s.queue_ids.is_empty() {
                    continue;
                }
                s.fail_enqueues = if failures && fail == 0 { 1 } else { 0 };
                s.back(rules);
            }
        }
        s.fail_enqueues = 0;
        trace.push(s.clone());
    }
    trace
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(4096)))]

    #[test]
    fn edge_prop_sizes(c in any::<u32>(), r in any::<u32>(), b in any::<usize>()) {
        prop_assert_eq!(buffer_size(c, r), c_buffer_size(c, r));
        prop_assert_eq!(wait_size(b), c_wait_size(b));
    }

    #[test]
    fn edge_prop_tests(a in any::<usize>(), b in any::<usize>(), small in 0usize..64, p in 0usize..16) {
        prop_assert_eq!(can_fill(a, b), c_can_fill(a, b));
        prop_assert_eq!(wait_over(a, b), c_wait_over(a, b));
        prop_assert_eq!(all_back(a, b), c_all_back(a, b));
        prop_assert_eq!(all_back(small, p), c_all_back(small, p));
    }

    #[test]
    fn edge_prop_volume(bits in any::<u32>(), near in -2000i32..2000) {
        let v = f32::from_bits(bits);
        prop_assert_eq!(needs_volume(v), c_needs_volume(v));
        let w = f32::from_bits(1.0f32.to_bits().wrapping_add_signed(near));
        prop_assert_eq!(needs_volume(w), c_needs_volume(w));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    /// Without failed enqueues, the stand-in on the core and on the C step
    /// for step alike.
    #[test]
    fn edge_prop_runs_match_c(
        channels in 1u32..=8,
        rate in prop_oneof![Just(44100u32), Just(48000), 8000u32..=192_000],
        ops in proptest::collection::vec(op(), 0..120),
    ) {
        prop_assert_eq!(run(CORE, channels, rate, &ops, false), run(C, channels, rate, &ops, false));
    }

    /// With failed enqueues the two differ (the intentional difference),
    /// but the port never loses a buffer: every buffer is always either
    /// free or queued, and after a failure the queue is started again once
    /// three buffers' worth of audio arrives.
    #[test]
    fn edge_prop_runs_keep_every_buffer(
        channels in 1u32..=8,
        rate in prop_oneof![Just(44100u32), Just(48000), 8000u32..=192_000],
        ops in proptest::collection::vec(op(), 0..120),
    ) {
        let trace = run(CORE, channels, rate, &ops, true);
        for s in &trace {
            prop_assert_eq!(s.free_ids.len() + s.queue_ids.len(), BUFFERS);
            let mut ids: Vec<usize> = s.free_ids.iter().chain(&s.queue_ids).copied().collect();
            ids.sort_unstable();
            prop_assert_eq!(ids, vec![0, 1, 2]);
        }
        let mut s = trace.last().cloned().unwrap_or_default();
        let three = s.buffer_size * BUFFERS;
        s.play(CORE, three);
        prop_assert!(!s.paused);
        prop_assert_eq!(s.queue_ids.len(), BUFFERS);
    }
}
