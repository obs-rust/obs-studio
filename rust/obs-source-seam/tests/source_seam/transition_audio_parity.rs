//! Tier 3 for `obs_source_seam::ffi::source_transition` against the
//! original rest of `libobs/obs-source-transition.c.h`
//! (`obs_c_oracle::source_transition`, renamed `oracle_*`):
//! `obs_transition_audio_render()`, `obs_transition_init()` and
//! `obs_transition_free()`.
//!
//! Both sides run on the same graph, built afresh for each: a test
//! transition, a source that is not a transition and two children (zeroed
//! `struct obs_source` with a type, a name and both transition mutexes),
//! each with a reference count and an active-child link count, each child
//! with an audio timestamp, pending, duplicated and removed flags and an
//! audio block of its own, and the caller's mix as one block. The C calls
//! the oracle's recording stand-ins (`obs_source_get_ref()`,
//! `obs_source_release()`, `obs_source_removed()`,
//! `obs_source_get_audio_timestamp()`, `obs_source_get_audio_mix()`, the
//! texrender create and destroy, the graphics context, and through the
//! stop oracle's C the release, active-child remove, signals and the
//! type's `transition_stop`); the Rust shims are run with [`Oracle`], whose
//! `Calls` are those same stand-ins, and reach the fields, the lock and the
//! mutexes through the C transition seam, as in the hybrid libobs. The
//! type's mix callbacks are the oracle's, which record their calls, a hash
//! of the times (and the context data) they got and whether the lock was
//! held, and return a value of the time and the call count.
//!
//! After every call the two must agree on what it returned and wrote to
//! `ts_out`, the caller's whole mix bit for bit, the transition's slots,
//! active flags, transitioning flags, clock, alignment and texrenders,
//! which mutexes are held, every reference and link count, the callbacks'
//! records, and every call in order (with whether the lock was held at the
//! timestamp and mix reads). The inputs are the characterization test's
//! cases, every audio stop step, every place, a child in both slots, the
//! argument check on NULL and on a source that is not a transition, init
//! and free with each texrender missing, and a proptest over random graphs
//! (any slots and flags, modes off the enum, any manual value, clock and
//! timestamps, every child state) and requests (any mixer mask, 0 to 8
//! channels, sample rates from 1 Hz up).
//!
//! No intentional differences. Left out, as libobs never passes them and
//! the C has no defined behaviour for them: a sample rate of 0 (the C
//! divides by it), more than `MAX_AUDIO_CHANNELS` channels (the C reads
//! past `data[]`) and NULL mix callbacks (the C calls through them). A
//! failed mutex init can't be made on the real mutexes the seam makes, so
//! Tier 1 covers it.

use std::ffi::{c_char, c_int, c_void};
use std::ptr;

use obs_c_oracle::parity::cases;
use obs_c_oracle::source_transition as c;
use obs_c_oracle::source_transition_slots::{self as slots, graph_lock, seam};
use obs_c_oracle::source_transition_stop as st;
use obs_c_oracle::source_transition_switch as sw;
use obs_source::transition_audio::{INIT_ALIGNMENT, MAX_AUDIO_MIXES};
use obs_source_seam::ffi::source_transition::{
    self as rs, AUDIO_OUTPUT_FRAMES, AudioMix, Calls, GS_RGBA, GS_ZS_NONE, MAX_AUDIO_CHANNELS,
    TOTAL_AUDIO_FLOATS,
};
use obs_source_seam::ffi::source_transition_stop as rs_stop;
use proptest::prelude::*;

/// `OBS_SOURCE_TYPE_INPUT`.
const INPUT: c_int = 0;
/// `OBS_SOURCE_TYPE_TRANSITION`.
const TRANSITION: c_int = 2;

/// `OBS_TRANSITION_MODE_AUTO`, `OBS_TRANSITION_MODE_MANUAL`.
const AUTO: c_int = 0;
const MANUAL: c_int = 1;

/// The sources of a graph, in the order they are made.
const TR: usize = 0;
const NOT_TR: usize = 1;
const CH_A: usize = 2;
const CH_B: usize = 3;
const SOURCES: usize = 4;

const NAMES: [&std::ffi::CStr; SOURCES] =
    [c"audio transition", c"an input", c"child a", c"child b"];

const T0: u64 = 1_000_000_000;
/// 512 frames at 48 kHz.
const HALF_BLOCK_NS: u64 = 10_666_667;
/// 1100 frames at 48 kHz.
const LATE_NS: u64 = 22_916_667;

/// The oracle's stand-ins, as the Rust shims' libobs calls.
enum Oracle {}

// SAFETY: each is the stand-in the C oracle calls under the same name.
unsafe impl rs_stop::Calls for Oracle {
    unsafe fn video_time() -> u64 {
        // SAFETY: see the trait.
        unsafe { st::oracle_stop_video_time() }
    }
    unsafe fn dosignal(
        transition: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    ) {
        // SAFETY: see the trait.
        unsafe { sw::oracle_switch_dosignal(transition, signal_obs, signal_source) }
    }
    unsafe fn remove_active_child(parent: *mut c_void, child: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { slots::oracle_slots_remove_active_child(parent, child) }
    }
    unsafe fn release(source: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { slots::oracle_slots_release(source) }
    }
}

// SAFETY: each is the stand-in the C oracle calls under the same name.
unsafe impl Calls for Oracle {
    unsafe fn removed(source: *mut c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { c::oracle_audio_removed(source) }
    }
    unsafe fn audio_timestamp(source: *mut c_void) -> u64 {
        // SAFETY: see the trait.
        unsafe { c::oracle_audio_timestamp(source) }
    }
    unsafe fn audio_mix(source: *mut c_void, audio: *mut AudioMix) {
        // SAFETY: see the trait.
        unsafe { c::oracle_audio_get_mix(source, audio.cast()) }
    }
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { slots::oracle_slots_get_ref(source) }
    }
    unsafe fn graphics() -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { c::oracle_audio_graphics() }
    }
    unsafe fn texrender_create() -> *mut c_void {
        // SAFETY: see the trait; the shim's own format and depth.
        unsafe { c::oracle_audio_texrender_create(GS_RGBA, GS_ZS_NONE) }
    }
    unsafe fn texrender_destroy(texrender: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_audio_texrender_destroy(texrender) }
    }
    unsafe fn enter_context(graphics: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_audio_enter_context(graphics) }
    }
    unsafe fn leave_context() {
        // SAFETY: see the trait.
        unsafe { c::oracle_audio_leave_context() }
    }
}

/// One slot of the transition: which source it holds and whether it is
/// active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    source: Option<usize>,
    active: bool,
}

/// A child's audio state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Audio {
    ts: u64,
    pending: bool,
    duplicated: bool,
    removed: bool,
    /// References of its own: 0 for a child being destroyed.
    refs: i64,
}

/// How a graph starts.
#[derive(Debug, Clone)]
struct Init {
    slots: [Slot; 2],
    mode: c_int,
    manual_val: f32,
    start_time: u64,
    duration: u64,
    video: bool,
    audio: bool,
    children: [Audio; 2],
    /// The type has a `transition_stop` (and the context data is the
    /// transition).
    plugin: bool,
    /// The ids the texrender creates hand out (0 for NULL).
    creates: [c_int; 2],
    /// Each mix callback call rewrites the clock, audio flag and context
    /// data, as another thread may after the lock.
    clobber: bool,
}

/// The source a render is handed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Null,
    Tr,
    NotTr,
}

/// What a render asks for.
#[derive(Debug, Clone, Copy)]
struct Req {
    mixers: u32,
    channels: usize,
    sample_rate: usize,
}

/// One call.
#[derive(Debug, Clone, Copy)]
enum Op {
    Render(Target, Req),
    /// `obs_transition_init()` on the transition, its mutexes destroyed
    /// first.
    Init,
    /// `obs_transition_free()`, its mutexes made again after.
    Free,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    C,
    Rust,
}

/// A graph's sources and the caller's mix.
struct Graph {
    src: [*mut c_void; SOURCES],
    out: *mut f32,
    mix: AudioMix,
}

impl Graph {
    /// Builds `init` on a fresh oracle graph.
    fn new(init: &Init) -> Self {
        let mut src = [ptr::null_mut(); SOURCES];
        // SAFETY: the oracle's constructors and the seam setters on its
        // live sources; the caller holds the graph lock.
        unsafe {
            slots::oracle_slots_reset();
            sw::oracle_switch_reset();
            for (i, s) in src.iter_mut().enumerate() {
                let kind = if i == TR { TRANSITION } else { INPUT };
                *s = slots::oracle_slots_source_new(kind, NAMES[i].as_ptr());
                assert!(!s.is_null());
            }
            let tr = src[TR];
            c::oracle_audio_reset(tr, init.creates[0], init.creates[1]);
            c::oracle_audio_clobber(init.clobber);
            st::oracle_stop_reset(tr, 0);
            for (i, a) in init.children.iter().enumerate() {
                let child = src[CH_A + i];
                let seed = u32::try_from(i).unwrap() + 1;
                c::oracle_audio_child(child, a.ts, a.pending, a.duplicated, a.removed, seed);
                slots::oracle_slots_set_refs(child, a.refs);
            }
            for (idx, slot) in init.slots.iter().enumerate() {
                let Some(child) = slot.source else { continue };
                seam::obs_source_transition_seam_set_sources(tr, idx, src[child]);
                seam::obs_source_transition_seam_set_source_active(tr, idx, slot.active);
                let refs = slots::oracle_slots_refs(src[child]);
                slots::oracle_slots_set_refs(src[child], refs + 1);
                if slot.active {
                    let links = slots::oracle_slots_link(tr, src[child]);
                    slots::oracle_slots_set_link(tr, src[child], links + 1);
                }
            }
            seam::obs_source_transition_seam_set_mode(tr, init.mode);
            seam::obs_source_transition_seam_set_manual_val(tr, init.manual_val);
            seam::obs_source_transition_seam_set_start_time(tr, init.start_time);
            seam::obs_source_transition_seam_set_duration(tr, init.duration);
            seam::obs_source_transition_seam_set_transitioning_video(tr, init.video);
            seam::obs_source_transition_seam_set_transitioning_audio(tr, init.audio);
            if init.plugin {
                st::oracle_stop_plugin(tr);
                st::oracle_stop_plugin(src[NOT_TR]);
            }
            let out = c::oracle_audio_block_new(99);
            assert!(!out.is_null());
            let mut mix = AudioMix::empty();
            for (m, output) in mix.output.iter_mut().enumerate() {
                for (ch, data) in output.data.iter_mut().enumerate() {
                    *data = out.add((m * MAX_AUDIO_CHANNELS + ch) * AUDIO_OUTPUT_FRAMES);
                }
            }
            Self { src, out, mix }
        }
    }

    fn id(&self, p: *mut c_void) -> Option<usize> {
        (!p.is_null()).then(|| self.src.iter().position(|s| *s == p).unwrap_or(99))
    }

    fn target(&self, t: Target) -> *mut c_void {
        match t {
            Target::Null => ptr::null_mut(),
            Target::Tr => self.src[TR],
            Target::NotTr => self.src[NOT_TR],
        }
    }
}

/// What a graph looks like after a call.
#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    /// What a render returned and left in `ts_out`.
    result: Option<(bool, u64)>,
    out: Vec<u32>,
    slots: [(Option<usize>, bool); 2],
    flags: (bool, bool),
    clock: (c_int, u32, u64, u64),
    alignment: u32,
    texrenders: [c_int; 2],
    held: [c_int; SOURCES],
    refs: [i64; SOURCES],
    links: [i64; SOURCES],
    /// (calls, hash, calls with the lock held) of each mix callback.
    mixes: [(i64, u64, i64); 2],
    /// (kind, a, b, result) of every traced call.
    trace: Vec<(c_int, Option<usize>, Option<usize>, c_int)>,
}

fn texrender_id(p: *mut c_void) -> c_int {
    // SAFETY: the oracle's getters.
    unsafe {
        (0..=2)
            .find(|&id| c::oracle_audio_texrender(id) == p)
            .unwrap_or(99)
    }
}

fn snapshot(g: &Graph, result: Option<(bool, u64)>) -> Snapshot {
    let tr = g.src[TR];
    // SAFETY: the oracle's getters on its live sources and block.
    unsafe {
        let slot = |idx| {
            (
                g.id(seam::obs_source_transition_seam_get_sources(tr, idx)),
                seam::obs_source_transition_seam_get_source_active(tr, idx),
            )
        };
        let mut trace = Vec::new();
        for i in 0..slots::oracle_slots_traced() {
            let (mut kind, mut a, mut b, mut r) = (0, ptr::null_mut(), ptr::null_mut(), 0);
            slots::oracle_slots_event(i, &raw mut kind, &raw mut a, &raw mut b, &raw mut r);
            trace.push((kind, g.id(a), g.id(b), r));
        }
        let mix = |which| {
            let (mut calls, mut hash, mut held) = (0, 0, 0);
            c::oracle_audio_mix_record(which, &raw mut calls, &raw mut hash, &raw mut held);
            (calls, hash, held)
        };
        let out = std::slice::from_raw_parts(g.out, TOTAL_AUDIO_FLOATS)
            .iter()
            .map(|f| f.to_bits())
            .collect();
        Snapshot {
            result,
            out,
            slots: [slot(0), slot(1)],
            flags: (
                seam::obs_source_transition_seam_get_transitioning_video(tr),
                seam::obs_source_transition_seam_get_transitioning_audio(tr),
            ),
            clock: (
                seam::obs_source_transition_seam_get_mode(tr),
                seam::obs_source_transition_seam_get_manual_val(tr).to_bits(),
                seam::obs_source_transition_seam_get_start_time(tr),
                seam::obs_source_transition_seam_get_duration(tr),
            ),
            alignment: c::oracle_audio_alignment(tr),
            texrenders: [0, 1].map(|idx| texrender_id(seam_texrender(tr, idx))),
            held: g.src.map(|s| st::oracle_stop_held(s)),
            refs: g.src.map(|s| slots::oracle_slots_refs(s)),
            links: g.src.map(|s| slots::oracle_slots_link(tr, s)),
            mixes: [mix(0), mix(1)],
            trace,
        }
    }
}

unsafe extern "C" {
    fn obs_source_transition_seam_get_texrender(
        transition: *const c_void,
        idx: usize,
    ) -> *mut c_void;
}

fn seam_texrender(tr: *mut c_void, idx: usize) -> *mut c_void {
    // SAFETY: the seam's getter on a live transition.
    unsafe { obs_source_transition_seam_get_texrender(tr, idx) }
}

/// Runs `op` on `g` through `side`.
fn run(g: &mut Graph, side: Side, op: Op) -> Option<(bool, u64)> {
    let tr = g.src[TR];
    let mix_a = Some(c::oracle_audio_mix_a as unsafe extern "C" fn(*mut c_void, f32) -> f32);
    let mix_b = Some(c::oracle_audio_mix_b as unsafe extern "C" fn(*mut c_void, f32) -> f32);
    // SAFETY: the graph's live sources (or NULL, which the C side never
    // gets), its mix and block, and the mutexes made where the C needs them.
    unsafe {
        match op {
            Op::Render(target, req) => {
                let mut ts = 0xDEAD_BEEF_u64;
                let audio = &raw mut g.mix;
                let ok = match side {
                    Side::C => c::oracle_obs_transition_audio_render(
                        g.target(target),
                        &raw mut ts,
                        audio.cast(),
                        req.mixers,
                        req.channels,
                        req.sample_rate,
                        mix_a,
                        mix_b,
                    ),
                    Side::Rust => rs::audio_render::<Oracle>(
                        g.target(target),
                        &raw mut ts,
                        audio,
                        req.mixers,
                        req.channels,
                        req.sample_rate,
                        mix_a,
                        mix_b,
                    ),
                };
                Some((ok, ts))
            }
            Op::Init => {
                c::oracle_audio_mutexes(tr, false);
                let ok = match side {
                    Side::C => c::oracle_obs_transition_init(tr),
                    Side::Rust => rs::init::<Oracle>(tr),
                };
                Some((ok, 0))
            }
            Op::Free => {
                match side {
                    Side::C => c::oracle_obs_transition_free(tr),
                    Side::Rust => rs::free::<Oracle>(tr),
                }
                c::oracle_audio_mutexes(tr, true);
                None
            }
        }
    }
}

/// Runs `ops` on a fresh graph through `side`, a snapshot after each.
fn trace_side(init: &Init, ops: &[Op], side: Side) -> Vec<Snapshot> {
    let mut g = Graph::new(init);
    let mut out = Vec::new();
    for &op in ops {
        if side == Side::C && matches!(op, Op::Render(Target::Null, _)) {
            // The C's transition_valid() logs and returns false: both sides
            // run it, the C one here with the same NULL.
        }
        let r = run(&mut g, side, op);
        out.push(snapshot(&g, r));
    }
    out
}

/// Checks `ops` on both sides: equal after every call.
fn check(init: &Init, ops: &[Op]) {
    let _graph = graph_lock();
    let c_side = trace_side(init, ops, Side::C);
    let r_side = trace_side(init, ops, Side::Rust);
    for (i, op) in ops.iter().enumerate() {
        let (cs, rs_) = (&c_side[i], &r_side[i]);
        if cs != rs_ {
            let first = cs.out.iter().zip(&rs_.out).position(|(a, b)| a != b);
            let mut cs = cs.clone();
            let mut rs_ = rs_.clone();
            cs.out.clear();
            rs_.out.clear();
            panic!(
                "call {i} {op:?} of {ops:?} on {init:?}: first sample apart {first:?}\nC:    {cs:?}\nRust: {rs_:?}"
            );
        }
    }
}

fn slot(source: Option<usize>, active: bool) -> Slot {
    Slot { source, active }
}

fn child(ts: u64) -> Audio {
    Audio {
        ts,
        pending: false,
        duplicated: false,
        removed: false,
        refs: 1,
    }
}

/// An idle auto transition holding child A, active; both children valid
/// at T0.
fn idle() -> Init {
    Init {
        slots: [slot(Some(CH_A), true), slot(None, false)],
        mode: AUTO,
        manual_val: 0.0,
        start_time: 0,
        duration: 0,
        video: false,
        audio: false,
        children: [child(T0), child(T0)],
        plugin: true,
        creates: [1, 2],
        clobber: false,
    }
}

/// Transitioning from A to B in manual mode at `t`.
fn transitioning(t: f32) -> Init {
    Init {
        slots: [slot(Some(CH_A), true), slot(Some(CH_B), true)],
        mode: MANUAL,
        manual_val: t,
        video: true,
        audio: true,
        ..idle()
    }
}

fn req(mixers: u32, channels: usize) -> Req {
    Req {
        mixers,
        channels,
        sample_rate: 48_000,
    }
}

fn render(mixers: u32, channels: usize) -> Op {
    Op::Render(Target::Tr, req(mixers, channels))
}

#[test]
fn layout_and_constants_match_the_headers() {
    // SAFETY: the oracle's layout and constant getters.
    unsafe {
        assert_eq!(c::oracle_audio_layout(0), size_of::<AudioMix>());
        assert_eq!(c::oracle_audio_layout(1), align_of::<AudioMix>());
        assert_eq!(
            c::oracle_audio_layout(2),
            std::mem::offset_of!(AudioMix, output) + size_of::<rs::AudioOutputData>()
        );
        assert_eq!(c::oracle_audio_layout(3), size_of::<rs::AudioOutputData>());
        assert_eq!(c::oracle_audio_layout(4), align_of::<rs::AudioOutputData>());
        assert_eq!(c::oracle_audio_layout(5), size_of::<*mut f32>());
        assert_eq!(c::oracle_audio_constant(0), GS_RGBA);
        assert_eq!(c::oracle_audio_constant(1), GS_ZS_NONE);
        assert_eq!(
            c::oracle_audio_constant(2),
            c_int::try_from(INIT_ALIGNMENT).unwrap()
        );
        assert_eq!(
            usize::try_from(c::oracle_audio_constant(3)).unwrap(),
            MAX_AUDIO_MIXES
        );
        assert_eq!(
            usize::try_from(c::oracle_audio_constant(4)).unwrap(),
            MAX_AUDIO_CHANNELS
        );
        assert_eq!(
            usize::try_from(c::oracle_audio_constant(5)).unwrap(),
            AUDIO_OUTPUT_FRAMES
        );
        assert_eq!(
            usize::try_from(c::oracle_audio_constant(6)).unwrap(),
            TOTAL_AUDIO_FLOATS * size_of::<f32>()
        );
    }
}

#[test]
fn init_and_free_match() {
    for creates in [[1, 2], [0, 2], [1, 0], [0, 0]] {
        let init = Init { creates, ..idle() };
        check(&init, &[Op::Init, render(0x1, 1), Op::Free]);
        check(&init, &[Op::Free]);
        check(&init, &[Op::Init, Op::Init]);
    }
}

#[test]
fn the_argument_check_matches() {
    for target in [Target::Null, Target::NotTr] {
        check(&transitioning(0.5), &[Op::Render(target, req(0x3F, 8))]);
    }
}

/// The characterization test's cases (test_source_transition_audio.c).
#[test]
fn characterized_cases_match() {
    // idle_copies_a
    check(&idle(), &[render(0x1, 1), render(0x3F, 8)]);
    // no_timestamp_writes_nothing, zero_timestamp_is_none
    for a in [
        Audio {
            pending: true,
            ..child(T0)
        },
        Audio {
            removed: true,
            ..child(T0)
        },
        child(0),
    ] {
        let init = Init {
            children: [a, child(T0)],
            ..idle()
        };
        check(&init, &[render(0x3F, 2)]);
    }
    check(
        &Init {
            slots: [slot(None, false); 2],
            ..idle()
        },
        &[render(0x3F, 2)],
    );
    // transitioning_mixes_both, no_mixers_mix_nothing
    check(
        &transitioning(0.5),
        &[render(0x5, 2), render(0, 2), render(0x3F, 8)],
    );
    // pending_and_duplicated_are_skipped
    let pending = Audio {
        pending: true,
        ..child(T0 - 1000)
    };
    let duplicated = Audio {
        duplicated: true,
        ..child(T0 - HALF_BLOCK_NS)
    };
    check(
        &Init {
            children: [pending, child(T0)],
            ..transitioning(0.5)
        },
        &[render(0x1, 1)],
    );
    check(
        &Init {
            children: [child(T0), duplicated],
            ..transitioning(0.5)
        },
        &[render(0x1, 1)],
    );
    // place_follows_the_timestamp
    for b in [T0 + HALF_BLOCK_NS, T0 + LATE_NS, 0] {
        check(
            &Init {
                children: [child(T0), child(b)],
                ..transitioning(0.5)
            },
            &[render(0x1, 1)],
        );
    }
}

/// The characterization test's clock and stop cases
/// (test_source_transition_audio.c).
#[test]
fn characterized_stop_cases_match() {
    // auto_mode_times_each_sample
    check(
        &Init {
            mode: AUTO,
            start_time: T0,
            duration: 4 * HALF_BLOCK_NS,
            children: [child(T0), child(T0 + HALF_BLOCK_NS)],
            ..transitioning(0.0)
        },
        &[render(0x1, 1), render(0x3, 2)],
    );
    // audio_end_stops, audio_end_with_video_running
    for video in [false, true] {
        check(
            &Init {
                video,
                children: [child(T0), child(T0 + 1000)],
                ..transitioning(1.0)
            },
            &[render(0x1, 1), render(0x1, 1)],
        );
    }
    // no_timestamp_stop
    for video in [true, false] {
        let p = Audio {
            pending: true,
            ..child(T0)
        };
        check(
            &Init {
                video,
                children: [p, p],
                ..transitioning(0.5)
            },
            &[render(0x1, 1)],
        );
    }
    // removed_child_after_stop
    for (t, video) in [(0.5, true), (1.0, false)] {
        check(
            &Init {
                video,
                children: [
                    child(T0),
                    Audio {
                        removed: true,
                        ..child(T0)
                    },
                ],
                ..transitioning(t)
            },
            &[render(0x1, 1)],
        );
    }
}

#[test]
fn more_cases_match() {
    // The same child in both slots; a child being destroyed; no plugin
    // (no context data, no stop callback); no stop without the audio
    // flag; an inactive slot A at the stop.
    let mut both = transitioning(0.5);
    both.slots[1].source = Some(CH_A);
    check(&both, &[render(0x3F, 2)]);
    let mut dying = transitioning(0.5);
    dying.children[1].refs = 0;
    check(&dying, &[render(0x1, 2)]);
    for (t, video, audio) in [(1.0, false, true), (1.0, false, false), (2.0, true, true)] {
        let init = Init {
            plugin: false,
            video,
            audio,
            ..transitioning(t)
        };
        check(&init, &[render(0x1, 1), render(0x1, 1)]);
    }
    let mut inactive = transitioning(1.0);
    inactive.video = false;
    inactive.slots[0].active = false;
    check(&inactive, &[render(0x1, 1)]);
}

/// The mix reads only the state copied under the lock (#3499), and the
/// context data once per mix and channel: a callback that rewrites the
/// clock, the audio flag and the context data on every call changes
/// nothing of the render (but the next one).
#[test]
fn writes_after_the_lock_match() {
    for (mode, start_time) in [(MANUAL, 0), (AUTO, T0 - HALF_BLOCK_NS)] {
        let init = Init {
            mode,
            start_time,
            duration: 4 * HALF_BLOCK_NS,
            clobber: true,
            children: [child(T0), child(T0 + HALF_BLOCK_NS)],
            ..transitioning(0.5)
        };
        check(&init, &[render(0x5, 2), render(0x5, 2)]);
    }
}

fn audio_strategy() -> impl Strategy<Value = Audio> {
    (
        prop_oneof![
            Just(0u64),
            Just(T0),
            Just(T0 + HALF_BLOCK_NS),
            Just(T0 + LATE_NS),
            Just(T0 - HALF_BLOCK_NS),
            (T0 - 30_000_000..T0 + 30_000_000),
            any::<u64>(),
        ],
        prop::bool::weighted(0.15),
        prop::bool::weighted(0.15),
        prop::bool::weighted(0.15),
        prop_oneof![Just(1i64), Just(0)],
    )
        .prop_map(|(ts, pending, duplicated, removed, refs)| Audio {
            ts,
            pending,
            duplicated,
            removed,
            refs,
        })
}

fn slot_strategy() -> impl Strategy<Value = Slot> {
    (
        prop_oneof![Just(None), Just(Some(CH_A)), Just(Some(CH_B))],
        any::<bool>(),
    )
        .prop_map(|(source, active)| Slot { source, active })
}

fn init_strategy() -> impl Strategy<Value = Init> {
    (
        [slot_strategy(), slot_strategy()],
        prop_oneof![Just(AUTO), Just(MANUAL), Just(2), Just(-1)],
        prop_oneof![
            Just(0.0f32),
            Just(0.5),
            Just(1.0),
            Just(1.5),
            Just(f32::NAN),
            -1.0f32..2.0
        ],
        prop_oneof![
            Just(0u64),
            Just(T0),
            Just(T0 - 1),
            (T0 - 40_000_000..T0 + 40_000_000)
        ],
        prop_oneof![
            Just(0u64),
            Just(1),
            Just(4 * HALF_BLOCK_NS),
            1u64..100_000_000
        ],
        any::<bool>(),
        any::<bool>(),
        [audio_strategy(), audio_strategy()],
        (
            any::<bool>(),
            [0..=2i32, 0..=2i32],
            prop::bool::weighted(0.25),
        ),
    )
        .prop_map(
            |(
                slots,
                mode,
                manual_val,
                start_time,
                duration,
                video,
                audio,
                children,
                (plugin, creates, clobber),
            )| Init {
                slots,
                mode,
                manual_val,
                start_time,
                duration,
                video,
                audio,
                children,
                plugin,
                creates,
                clobber,
            },
        )
}

fn op_strategy() -> impl Strategy<Value = Op> {
    let req = (
        prop_oneof![Just(0x3Fu32), Just(0x1), any::<u32>()],
        0..=MAX_AUDIO_CHANNELS,
        prop_oneof![
            Just(48_000usize),
            Just(44_100),
            Just(96_000),
            1usize..200_000
        ],
    )
        .prop_map(|(mixers, channels, sample_rate)| Req {
            mixers,
            channels,
            sample_rate,
        });
    prop_oneof![
        8 => req.clone().prop_map(|r| Op::Render(Target::Tr, r)),
        1 => req.prop_map(|r| Op::Render(Target::NotTr, r)),
        1 => Just(Op::Init),
        1 => Just(Op::Free),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(256)))]

    #[test]
    fn random_graphs_match(
        init in init_strategy(),
        ops in prop::collection::vec(op_strategy(), 1..4),
    ) {
        check(&init, &ops);
    }
}
