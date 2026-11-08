//! Tier 3 for `obs_core::ffi::audio_callback` against the original
//! `libobs/obs-audio.c.h` (`obs_c_oracle::audio_callback`, renamed
//! `oracle_audio_callback`, on the C oracles of its decisions).
//!
//! Each case builds the same world twice (a stand-in core with its views,
//! mixes, sources, buffering and task queue) and runs the same ticks on
//! both: the Rust shim on one, through the core-audio seam and the
//! oracle's recording stand-ins as its `Calls`, and the C on the other.
//! After every tick the two must agree on the result and `*out_ts`, and on
//! the world: the log (each libobs call in order with its arguments and
//! the locks it saw held, each log line with its values, each task run),
//! the core's buffering fields, buffered timestamps, render order and root
//! nodes, every source's reference count, flags, timestamp, input (sizes
//! and bytes) and output buffers, every mutex being free, and the mixes bit
//! for bit.
//!
//! One intentional difference, which no single-threaded run can see: the
//! Rust reads a root source's `audio_pending` under its `audio_buf_mutex`,
//! where the C reads it just before taking the lock. Nothing is excluded
//! for it; Tier 1's `pending_is_read_under_the_buffer_lock` checks it.
//!
//! The oracle's core is one global, so the cases run one at a time under
//! [`LOCK`].

use std::ffi::{c_char, c_int, c_void};
use std::sync::Mutex;

use obs_c_oracle::audio_callback::{self as c, CoreInit, SourceInit};
use obs_core::audio_render::{SOURCE_AUDIO, SOURCE_COMPOSITE, TYPE_INPUT};
use obs_core::ffi::audio_callback::{self as rs, Blog, Calls, EnumProc};
use obs_core::ffi::audio_mix::audio_output_data;
use proptest::prelude::*;

static LOCK: Mutex<()> = Mutex::new(());

const RATE: u32 = 48_000;
const START: u64 = 10_000_000_000;
/// One tick of 1024 frames at 48 kHz.
const TICK: u64 = 21_333_333;
const TYPE_FILTER: u32 = 1;
const TYPE_TRANSITION: u32 = 2;
const TYPE_SCENE: u32 = 3;
const SOURCE_VIDEO: u32 = 1;

/// The oracle's stand-ins, as the shim's libobs calls.
enum Stand {}

// SAFETY: each method is the stand-in of the libobs function it names,
// with the same signature.
unsafe impl Calls for Stand {
    unsafe fn sample_rate(audio: *const c_void) -> u32 {
        // SAFETY: the world's own output pointer.
        unsafe { c::oracle_tick_sample_rate(audio) }
    }
    unsafe fn channels(audio: *const c_void) -> usize {
        // SAFETY: as above.
        unsafe { c::oracle_tick_channels(audio) }
    }
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: a source of the world.
        unsafe { c::oracle_tick_get_ref(source) }
    }
    unsafe fn release(source: *mut c_void) {
        // SAFETY: as above.
        unsafe { c::oracle_tick_release(source) }
    }
    unsafe fn get_name(source: *const c_void) -> *const c_char {
        // SAFETY: as above.
        unsafe { c::oracle_tick_get_name(source) }
    }
    unsafe fn removed(source: *const c_void) -> bool {
        // SAFETY: as above.
        unsafe { c::oracle_tick_removed(source) }
    }
    unsafe fn active(source: *const c_void) -> bool {
        // SAFETY: as above.
        unsafe { c::oracle_tick_active(source) }
    }
    unsafe fn audio_render(
        source: *mut c_void,
        mixers: u32,
        channels: usize,
        sample_rate: usize,
        size: usize,
    ) {
        // SAFETY: as above.
        unsafe { c::oracle_tick_audio_render(source, mixers, channels, sample_rate, size) }
    }
    unsafe fn enum_active_tree(source: *mut c_void, callback: EnumProc, param: *mut c_void) {
        // SAFETY: as above, with the shim's callback and its parameter.
        unsafe { c::oracle_tick_enum_active_tree(source, callback, param) }
    }
    fn blog() -> Blog {
        c::oracle_tick_blog
    }
}

/// One source of a world.
#[derive(Debug, Clone)]
struct Src {
    init: SourceInit,
    tree: Vec<c_int>,
}

/// A world: what both sides start from.
#[derive(Debug, Clone)]
struct Spec {
    channels: usize,
    core: CoreInit,
    sources: Vec<Src>,
    audio_list: Vec<c_int>,
    /// Each view's set channels.
    views: Vec<Vec<(u32, c_int)>>,
    /// Each mix: its view (-1 for none) and `mix_audio`.
    mixes: Vec<(c_int, bool)>,
    dedup: c_int,
    /// Each queued task: its id and chain.
    tasks: Vec<(c_int, c_int)>,
}

/// One tick: its mixers, and the audio fed to each source after it.
#[derive(Debug, Clone)]
struct Step {
    mixers: u32,
    feeds: Vec<(c_int, u32, u64)>,
}

struct World(*mut c_void);

impl World {
    fn new(spec: &Spec) -> Self {
        // SAFETY: plain values; the indexes are the world's own.
        unsafe {
            let w = c::oracle_tick_new(RATE, spec.channels, &raw const spec.core);
            for s in &spec.sources {
                c::oracle_tick_add_source(w, &raw const s.init);
            }
            for (i, s) in spec.sources.iter().enumerate() {
                let i = c_int::try_from(i).unwrap();
                c::oracle_tick_set_tree(w, i, s.tree.as_ptr(), s.tree.len());
            }
            c::oracle_tick_link_audio(w, spec.audio_list.as_ptr(), spec.audio_list.len());
            for channels in &spec.views {
                let v = c::oracle_tick_add_view(w);
                for &(ch, s) in channels {
                    c::oracle_tick_set_channel(w, v, ch, s);
                }
            }
            for &(view, mix_audio) in &spec.mixes {
                c::oracle_tick_add_mix(w, view, mix_audio);
            }
            c::oracle_tick_set_dedup(w, spec.dedup);
            for &(id, chain) in &spec.tasks {
                c::oracle_tick_queue_task(w, id, chain);
            }
            Self(w)
        }
    }

    fn state(&self) -> String {
        let mut out = vec![0u8; 262_144];
        // SAFETY: a live world and a buffer of `out.len()` bytes.
        let len = unsafe { c::oracle_tick_state(self.0, out.as_mut_ptr().cast(), out.len()) };
        out.truncate(len);
        String::from_utf8(out).unwrap()
    }

    fn feed(&self, feeds: &[(c_int, u32, u64)]) {
        for &(s, frames, ts) in feeds {
            // SAFETY: a live world and one of its sources.
            unsafe { c::oracle_tick_feed(self.0, s, frames, ts) };
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        // SAFETY: a live world, freed once.
        unsafe { c::oracle_tick_free(self.0) };
    }
}

/// Runs `steps` on both sides of `spec` and compares after each.
fn check(spec: &Spec, steps: &[Step]) {
    let _guard = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let rust = World::new(spec);
    let other = World::new(spec);
    for (i, step) in steps.iter().enumerate() {
        let start = START + u64::try_from(i).unwrap() * TICK;
        let end = start + TICK;

        let mut rust_ts = 0;
        // SAFETY: the world is the core the seam and stand-ins see, and its
        // mixes are MAX_AUDIO_MIXES cleared `struct audio_output_data`.
        let rust_result = unsafe {
            let mixes = c::oracle_tick_clear_mixes(rust.0).cast::<audio_output_data>();
            c::oracle_tick_use(rust.0);
            rs::audio_callback_with::<Stand>(start, end, &raw mut rust_ts, step.mixers, mixes)
        };
        let mut c_ts = 0;
        // SAFETY: a live world.
        let c_result =
            unsafe { c::oracle_tick_run_c(other.0, start, end, step.mixers, &raw mut c_ts) };

        assert_eq!((rust_result, rust_ts), (c_result, c_ts), "tick {i} result");
        assert_eq!(rust.state(), other.state(), "tick {i}");
        rust.feed(&step.feeds);
        other.feed(&step.feeds);
    }
}

fn input() -> SourceInit {
    SourceInit {
        kind: TYPE_INPUT,
        output_flags: SOURCE_AUDIO,
        active: true,
        audio_ts: START,
        frames: 1024,
        out_mixes: 0x3f,
        audio_mixers: 0x3f,
        volume: 1.0,
        ..SourceInit::default()
    }
}

fn scene() -> SourceInit {
    SourceInit {
        kind: TYPE_SCENE,
        output_flags: SOURCE_VIDEO | SOURCE_AUDIO | SOURCE_COMPOSITE,
        audio_render: true,
        frames: 0,
        ..input()
    }
}

fn src(init: SourceInit, tree: &[c_int]) -> Src {
    Src {
        init,
        tree: tree.to_vec(),
    }
}

fn spec(sources: Vec<Src>) -> Spec {
    Spec {
        channels: 2,
        core: CoreInit {
            max_buffering_ticks: 45,
            ..CoreInit::default()
        },
        sources,
        audio_list: vec![],
        views: vec![],
        mixes: vec![],
        dedup: -1,
        tasks: vec![],
    }
}

fn steady(n: usize, sources: &[c_int]) -> Vec<Step> {
    (0..n)
        .map(|_| Step {
            mixers: 0x3f,
            feeds: sources.iter().map(|&s| (s, 1024, START)).collect(),
        })
        .collect()
}

#[test]
fn a_scene_with_two_inputs_matches() {
    let mut s = spec(vec![
        src(scene(), &[1, 2]),
        src(input(), &[]),
        src(input(), &[]),
    ]);
    s.audio_list = vec![1, 2];
    s.views = vec![vec![(0, 0)]];
    s.mixes = vec![(0, true), (0, true), (-1, true)];
    check(&s, &steady(3, &[1, 2]));
}

#[test]
fn a_shared_input_is_a_root_and_matches() {
    let mut s = spec(vec![
        src(scene(), &[2]),
        src(scene(), &[2]),
        src(input(), &[]),
    ]);
    s.audio_list = vec![2];
    s.views = vec![vec![(0, 0), (1, 1)]];
    s.mixes = vec![(0, true)];
    check(&s, &steady(2, &[2]));
}

#[test]
fn late_and_short_sources_buffer_and_log_alike() {
    let mut late = input();
    late.audio_ts = START - 3 * TICK;
    let mut short = input();
    short.frames = 100;
    let mut s = spec(vec![src(late, &[]), src(short, &[]), src(input(), &[])]);
    s.audio_list = vec![0, 1, 2];
    s.core.max_buffering_ticks = 2;
    check(&s, &steady(4, &[0, 1, 2]));
}

#[test]
fn going_backwards_at_max_buffering_matches() {
    let mut back = input();
    back.audio_ts = START - TICK / 2;
    back.frames = 4096;
    let mut render = scene();
    render.audio_ts = START - TICK;
    let mut s = spec(vec![src(back, &[]), src(render, &[])]);
    s.audio_list = vec![0, 1];
    s.core.max_buffering_ticks = 3;
    s.core.total_buffering_ticks = 3;
    check(&s, &steady(3, &[0]));
}

#[test]
fn fixed_buffering_matches() {
    let mut s = spec(vec![src(input(), &[])]);
    s.audio_list = vec![0];
    s.core.fixed_buffer = true;
    s.core.max_buffering_ticks = 3;
    check(&s, &steady(5, &[0]));
}

#[test]
fn the_monitoring_dedup_matches() {
    let mut dedup = input();
    dedup.audio_mixers = 0b0110;
    let mut monitored = input();
    monitored.monitoring = true;
    monitored.audio_mixers = 0b0011;
    let mut s = spec(vec![src(dedup, &[]), src(monitored, &[])]);
    s.audio_list = vec![0, 1];
    s.views = vec![vec![(0, 0), (1, 1)]];
    s.mixes = vec![(0, true)];
    s.dedup = 0;
    check(&s, &steady(2, &[0, 1]));
}

#[test]
fn tasks_and_wait_ticks_match() {
    let mut s = spec(vec![src(input(), &[])]);
    s.audio_list = vec![0];
    s.core.buffering_wait_ticks = 1;
    s.tasks = vec![(1, 2), (2, 0)];
    check(&s, &steady(3, &[0]));
}

fn kind() -> impl Strategy<Value = (u32, u32, bool)> {
    prop_oneof![
        Just((TYPE_INPUT, SOURCE_AUDIO, false)),
        Just((TYPE_INPUT, SOURCE_VIDEO, false)),
        Just((TYPE_INPUT, SOURCE_AUDIO | SOURCE_COMPOSITE, true)),
        Just((TYPE_FILTER, SOURCE_AUDIO, false)),
        Just((TYPE_TRANSITION, SOURCE_AUDIO | SOURCE_COMPOSITE, true)),
        Just((
            TYPE_SCENE,
            SOURCE_VIDEO | SOURCE_AUDIO | SOURCE_COMPOSITE,
            true
        )),
    ]
}

fn stamp() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0),
        Just(START),
        Just(START - 1),
        Just(START + 1),
        (1u64..6).prop_map(|k| START - k * TICK),
        (1u64..TICK).prop_map(|d| START - d),
        (1u64..TICK).prop_map(|d| START + d),
        Just(START + TICK),
    ]
}

fn frames() -> impl Strategy<Value = u32> {
    prop_oneof![Just(0), Just(1024), Just(2048), 0u32..4096]
}

fn source(count: usize) -> impl Strategy<Value = Src> {
    let n = c_int::try_from(count).unwrap();
    (
        kind(),
        (any::<bool>(), any::<bool>(), any::<bool>(), any::<bool>()),
        (any::<bool>(), any::<bool>(), any::<bool>(), any::<bool>()),
        (stamp(), frames(), 0u32..64),
        (
            prop_oneof![Just(1.0f32), Just(0.0), Just(0.5)],
            prop::bool::weighted(0.9),
            prop::bool::weighted(0.1),
            prop::bool::weighted(0.1),
        ),
        prop::collection::vec(0..n, 0..4),
    )
        .prop_map(
            |(
                (kind, output_flags, audio_render),
                (pending, pending_stop, timing_set, duplicated),
                (muted, monitoring, full_out, all_mixers),
                (audio_ts, frames, audio_mixers),
                (volume, active, removed, dead),
                tree,
            )| Src {
                init: SourceInit {
                    kind,
                    output_flags,
                    audio_render,
                    removed,
                    active,
                    dead,
                    pending,
                    pending_stop,
                    timing_set,
                    duplicated,
                    muted,
                    monitoring,
                    audio_ts,
                    frames,
                    // libobs allocates a source's output buffers all at
                    // once (every mix and channel) or not at all, and the mix
                    // reads every one once the first is set
                    out_mixes: if full_out { 0x3f } else { 0 },
                    audio_mixers: if all_mixers { 0x3f } else { audio_mixers },
                    volume,
                },
                tree,
            },
        )
}

fn world() -> impl Strategy<Value = (Spec, Vec<Step>)> {
    (1usize..=7, prop_oneof![Just(1usize), Just(2), Just(6)]).prop_flat_map(|(count, channels)| {
        let n = c_int::try_from(count).unwrap();
        (
            prop::collection::vec(source(count), count),
            prop::sample::subsequence((0..n).collect::<Vec<_>>(), 0..=count),
            prop::collection::vec(prop::collection::vec((0u32..4, -1..n), 0..4), 0..3),
            prop::collection::vec((-1i32..3, any::<bool>()), 0..4),
            (
                prop_oneof![Just(-1), 0..n],
                0u64..3,
                0i32..6,
                0i32..8,
                prop::bool::weighted(0.2),
            ),
            prop::collection::vec((0i32..50, 0i32..3), 0..3),
            prop::collection::vec(
                (
                    prop_oneof![Just(0x3fu32), 0u32..64],
                    prop::collection::vec((0..n, frames(), stamp()), 0..4),
                ),
                1..5,
            ),
        )
            .prop_map(
                move |(sources, audio_list, views, mixes, core, tasks, steps)| {
                    let views_len = c_int::try_from(views.len()).unwrap();
                    let (dedup, wait, total, max, fixed) = core;
                    let spec = Spec {
                        channels,
                        core: CoreInit {
                            buffered_ts: 0,
                            buffering_wait_ticks: wait,
                            total_buffering_ticks: total.min(max),
                            max_buffering_ticks: max,
                            fixed_buffer: fixed,
                        },
                        sources,
                        audio_list,
                        views,
                        // a mix on a view that doesn't exist has none
                        mixes: mixes
                            .into_iter()
                            .map(|(v, a)| (if v < views_len { v } else { -1 }, a))
                            .collect(),
                        dedup,
                        tasks,
                    };
                    let steps = steps
                        .into_iter()
                        .map(|(mixers, feeds)| Step { mixers, feeds })
                        .collect();
                    (spec, steps)
                },
            )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn any_world_matches_the_c((spec, steps) in world()) {
        check(&spec, &steps);
    }
}
