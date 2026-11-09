//! Tier 3 for `obs_source_seam::ffi::source_transition_video` against the
//! original `libobs/obs-source-transition-video.c.h`
//! (`obs_c_oracle::source_transition_video`, renamed `oracle_*`).
//!
//! Both sides run on the same graph, built afresh for each: a test
//! transition, a source that is not a transition and two children (zeroed
//! `struct obs_source` with a type, a name and both transition mutexes),
//! each with a reference count; the transition's slots, both transitioning
//! flags, two matrices, two texrenders and its context data are set through
//! the transition seam. The C calls the oracle's recording stand-ins for
//! the clock read, stop, stop handler and size helpers, the source calls,
//! the signal and every graphics call, on fake texrenders and textures
//! whose formats, textures and begin result the graph sets; the Rust shims
//! are run with [`Oracle`], whose `Calls` are those same stand-ins, and
//! reach the fields, both locks and the context data through the C
//! transition seam, as in the hybrid libobs. The draw callback is the
//! oracle's recording one on both sides. Every stand-in records whether the
//! transition's `transition_mutex` and `transition_tex_mutex` were held.
//!
//! After every call the two must agree on what it returned, the
//! transition's slots, flags and texrenders, which of its mutexes are held,
//! every reference count, every call the stand-ins logged in order (with
//! its arguments, the floats bit for bit, and the held mutexes) and the
//! slots oracle's trace of the reference calls and the stop's slot shift.
//! The inputs are every call on fixed graphs (each flag pair, a due or
//! early time, active or not, each slot empty or holding a child, a
//! texrender whose format fits or not and whose texture is there or not,
//! a texrender that does not begin, a zero size, a callback or none, the
//! texture lock free or held), and a proptest over random graphs and call
//! sequences. The `graphics.h` inline `gs_get_format_from_space()`, which
//! the production `Calls` runs as its Rust port, is compared with the C on
//! every space and on values outside the enum.
//!
//! Intentional differences from C:
//! - `obs_transition_video_get_color_space()` checks its argument with
//!   `transition_valid()`. On NULL the C crashes, so only the Rust side
//!   runs: it returns `GS_CS_SRGB` and calls nothing. On a source that is
//!   not a transition the C mixes that source's slots; the Rust side
//!   returns `GS_CS_SRGB` and calls nothing, checked against the C's
//!   outcome.
//! - `obs_transition_video_render_direct()` hands `transition_valid()` its
//!   own name. That only changes the debug line logged for NULL, which the
//!   oracle does not record: on NULL both sides return false and call
//!   nothing, and that is compared.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

use obs_c_oracle::parity::cases;
use obs_c_oracle::source_transition_size::{self as size, Matrix4 as OracleMatrix4};
use obs_c_oracle::source_transition_slots::{self as slots, graph_lock, seam};
use obs_c_oracle::source_transition_stop as stop;
use obs_c_oracle::source_transition_video::{self as c, HANDLES, kind};
use obs_source::transition_video::format_from_space;
use obs_source_seam::ffi::source_transition_video::{self as rs, Calls, Matrix4, Vec4};
use proptest::prelude::*;

/// `OBS_SOURCE_TYPE_INPUT`.
const INPUT: c_int = 0;
/// `OBS_SOURCE_TYPE_TRANSITION`.
const TRANSITION: c_int = 2;

/// `GS_CS_SRGB`, `GS_CS_SRGB_16F`, `GS_CS_709_EXTENDED`, `GS_CS_709_SCRGB`.
const SRGB: c_int = 0;
const SRGB_16F: c_int = 1;
const EXTENDED: c_int = 2;
const SCRGB: c_int = 3;

/// `GS_RGBA`, `GS_RGBA16F`.
const RGBA: c_int = 3;
const RGBA16F: c_int = 9;

/// The sources of a graph, in the order they are made.
const TR: usize = 0;
const NOT_TR: usize = 1;
const CH_A: usize = 2;
const CH_B: usize = 3;
const SOURCES: usize = 4;

const NAMES: [&CStr; SOURCES] = [c"video transition", c"an input", c"child a", c"child b"];

/// The fake texture the core's transparent texture is.
const TRANSPARENT: usize = 30;
/// The first fake texrender a create hands out.
const CREATED: usize = 20;

/// The oracle's stand-ins, as the Rust shims' libobs calls.
enum Oracle {}

// SAFETY: each is the stand-in the C oracle calls under the same name.
unsafe impl Calls for Oracle {
    unsafe fn video_time(transition: *mut c_void) -> f32 {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_time(transition) }
    }
    unsafe fn stop(transition: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_stop(transition) }
    }
    unsafe fn handle_stop(transition: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_handle_stop(transition) }
    }
    unsafe fn cx(transition: *mut c_void) -> u32 {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_cx(transition) }
    }
    unsafe fn cy(transition: *mut c_void) -> u32 {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_cy(transition) }
    }
    unsafe fn active(source: *mut c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_active(source) }
    }
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_get_ref(source) }
    }
    unsafe fn release(source: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_release(source) }
    }
    unsafe fn dosignal(
        transition: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    ) {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_dosignal(transition, signal_obs, signal_source) }
    }
    unsafe fn color_space(
        source: *mut c_void,
        count: usize,
        preferred_spaces: *const c_int,
    ) -> c_int {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_color_space(source, count, preferred_spaces) }
    }
    unsafe fn video_render(source: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_render(source) }
    }
    unsafe fn transparent_texture() -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_transparent_texture() }
    }
    unsafe fn texrender_get_texture(texrender: *mut c_void) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_texrender_get_texture(texrender) }
    }
    unsafe fn texrender_get_format(texrender: *mut c_void) -> c_int {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_texrender_get_format(texrender) }
    }
    unsafe fn texrender_destroy(texrender: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_texrender_destroy(texrender) }
    }
    unsafe fn texrender_create(format: c_int, zsformat: c_int) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_texrender_create(format, zsformat) }
    }
    unsafe fn texrender_begin(texrender: *mut c_void, cx: u32, cy: u32, space: c_int) -> bool {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_texrender_begin(texrender, cx, cy, space) }
    }
    unsafe fn texrender_end(texrender: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_texrender_end(texrender) }
    }
    unsafe fn format_from_space(space: c_int) -> c_int {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_format_from_space(space) }
    }
    unsafe fn current_color_space() -> c_int {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_current_color_space() }
    }
    unsafe fn clear(clear_flags: u32, color: *const Vec4, depth: f32, stencil: u8) {
        // SAFETY: see the trait; `Vec4` is four floats.
        unsafe { c::oracle_video_clear(clear_flags, color.cast(), depth, stencil) }
    }
    unsafe fn ortho(left: f32, right: f32, top: f32, bottom: f32, znear: f32, zfar: f32) {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_ortho(left, right, top, bottom, znear, zfar) }
    }
    unsafe fn matrix_push() {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_matrix_push() }
    }
    unsafe fn matrix_mul(matrix: *const Matrix4) {
        // SAFETY: see the trait; `Matrix4` is sixteen floats.
        unsafe { c::oracle_video_matrix_mul(matrix.cast()) }
    }
    unsafe fn matrix_pop() {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_matrix_pop() }
    }
    unsafe fn blend_state_push() {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_blend_state_push() }
    }
    unsafe fn blend_function(src: c_int, dest: c_int) {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_blend_function(src, dest) }
    }
    unsafe fn blend_state_pop() {
        // SAFETY: see the trait.
        unsafe { c::oracle_video_blend_state_pop() }
    }
}

/// A fake texrender's set-up: its format and whether its texture is NULL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fake {
    format: c_int,
    no_texture: bool,
}

/// How a graph starts.
#[derive(Debug, Clone)]
struct Init {
    /// The transition's slots (a source of the graph or none); the input
    /// gets the same, for the C's colour space read on it.
    slots: [Option<usize>; 2],
    tv: bool,
    ta: bool,
    matrices: [[f32; 16]; 2],
    /// The fake texrender (0..4) each slot holds, or none.
    texrenders: [Option<usize>; 2],
    fakes: [Fake; 4],
    time: f32,
    cx: u32,
    cy: u32,
    active: bool,
    current_space: c_int,
    begins: bool,
    /// The colour space each source gives.
    spaces: [c_int; SOURCES],
    /// The test holds `transition_tex_mutex` around the call.
    tex_busy: bool,
}

/// The source a public call is handed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Null,
    Tr,
    NotTr,
}

/// One call.
#[derive(Debug, Clone, Copy)]
enum Op {
    Render(Target, bool),
    /// With a callback or not, and a placeholder texture or NULL.
    Render2(Target, bool, bool),
    ColorSpace(Target),
    Direct(Target, c_int),
}

impl Op {
    /// The argument check that differs from C on purpose, if this is one.
    fn checks_argument(self) -> Option<Target> {
        match self {
            Self::ColorSpace(t) if t != Target::Tr => Some(t),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    C,
    Rust,
}

/// A graph's sources.
struct Graph {
    src: [*mut c_void; SOURCES],
}

impl Graph {
    /// Builds `init` on a fresh oracle graph.
    fn new(init: &Init) -> Self {
        let mut src = [ptr::null_mut(); SOURCES];
        // SAFETY: the oracle's constructors and the seam setters on its
        // live sources; the caller holds the graph lock.
        unsafe {
            slots::oracle_slots_reset();
            for (i, s) in src.iter_mut().enumerate() {
                let kind = if i == TR { TRANSITION } else { INPUT };
                *s = slots::oracle_slots_source_new(kind, NAMES[i].as_ptr());
                assert!(!s.is_null());
            }
            let tr = src[TR];
            c::oracle_video_reset(
                tr,
                init.time,
                init.cx,
                init.cy,
                init.active,
                init.current_space,
                init.begins,
                CREATED,
            );
            for owner in [TR, NOT_TR] {
                for (idx, slot) in init.slots.iter().enumerate() {
                    let Some(child) = *slot else { continue };
                    seam::obs_source_transition_seam_set_sources(src[owner], idx, src[child]);
                    let refs = slots::oracle_slots_refs(src[child]);
                    slots::oracle_slots_set_refs(src[child], refs + 1);
                }
            }
            seam::obs_source_transition_seam_set_transitioning_video(tr, init.tv);
            seam::obs_source_transition_seam_set_transitioning_audio(tr, init.ta);
            for (idx, m) in init.matrices.iter().enumerate() {
                let m = OracleMatrix4(*m);
                size::seam::obs_source_transition_seam_set_matrix(tr, idx, &raw const m);
            }
            for (idx, t) in init.texrenders.iter().enumerate() {
                let p = t.map_or(ptr::null_mut(), |n| c::oracle_video_texrender(n));
                seam::obs_source_transition_seam_set_texrender(tr, idx, p);
            }
            for (n, f) in init.fakes.iter().enumerate() {
                c::oracle_video_set_texrender(n, f.format, f.no_texture);
            }
            for (i, space) in init.spaces.iter().enumerate() {
                c::oracle_video_set_space(src[i], *space);
            }
            // context.data = the transition itself
            stop::oracle_stop_plugin(tr);
            c::oracle_video_set_transparent(c::oracle_video_texture(TRANSPARENT));
        }
        Self { src }
    }

    fn id(&self, p: *const c_void) -> Option<usize> {
        (!p.is_null()).then(|| {
            self.src
                .iter()
                .position(|s| s.cast_const() == p)
                .unwrap_or(99)
        })
    }

    fn target(&self, t: Target) -> *mut c_void {
        match t {
            Target::Null => ptr::null_mut(),
            Target::Tr => self.src[TR],
            Target::NotTr => self.src[NOT_TR],
        }
    }
}

/// A pointer an event names, as what it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Who {
    Null,
    Source(usize),
    Texrender(usize),
    Texture(usize),
    Other,
}

fn who(g: &Graph, p: *const c_void) -> Who {
    if p.is_null() {
        return Who::Null;
    }
    if let Some(i) = g.id(p).filter(|i| *i < SOURCES) {
        return Who::Source(i);
    }
    // SAFETY: the oracle's fake handles, compared by address only.
    unsafe {
        for n in 0..HANDLES {
            if p == c::oracle_video_texrender(n).cast_const() {
                return Who::Texrender(n);
            }
            if p == c::oracle_video_texture(n).cast_const() {
                return Who::Texture(n);
            }
        }
    }
    Who::Other
}

/// An event, with its pointers resolved and its names read.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Logged {
    kind: c_int,
    p: [Who; 3],
    name: [Option<Vec<u8>>; 2],
    i: [c_int; 4],
    f: [u32; 16],
    held: c_int,
}

/// What a graph looks like after a call.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    result: Option<i64>,
    slots: [Option<usize>; 2],
    flags: (bool, bool),
    texrenders: [Who; 2],
    /// Whether `transition_mutex` and `transition_tex_mutex` are held.
    held: (bool, bool),
    refs: [i64; SOURCES],
    log: Vec<Logged>,
    /// (kind, a, b, result) of the slots oracle's trace.
    trace: Vec<(c_int, Option<usize>, Option<usize>, c_int)>,
}

fn snapshot(g: &Graph, result: Option<i64>) -> Snapshot {
    let tr = g.src[TR];
    // SAFETY: the oracle's getters on its live sources and its log.
    unsafe {
        let held = |try_lock: unsafe extern "C" fn(*mut c_void) -> c_int,
                    unlock: unsafe extern "C" fn(*mut c_void) -> c_int| {
            if try_lock(tr) == 0 {
                unlock(tr);
                false
            } else {
                true
            }
        };
        let log = (0..c::oracle_video_logged())
            .map(|i| {
                let e = *c::oracle_video_event(i);
                let name = e
                    .name
                    .map(|n| (!n.is_null()).then(|| CStr::from_ptr(n).to_bytes().to_vec()));
                Logged {
                    kind: e.kind,
                    p: e.p.map(|p| who(g, p)),
                    name,
                    i: e.i,
                    f: e.f,
                    held: e.held,
                }
            })
            .collect();
        let trace = (0..slots::oracle_slots_traced())
            .map(|i| {
                let (mut k, mut a, mut b, mut r) = (0, ptr::null_mut(), ptr::null_mut(), 0);
                slots::oracle_slots_event(i, &mut k, &mut a, &mut b, &mut r);
                (k, g.id(a), g.id(b), r)
            })
            .collect();
        Snapshot {
            result,
            slots: [0, 1].map(|idx| g.id(seam::obs_source_transition_seam_get_sources(tr, idx))),
            flags: (
                seam::obs_source_transition_seam_get_transitioning_video(tr),
                seam::obs_source_transition_seam_get_transitioning_audio(tr),
            ),
            texrenders: [0, 1]
                .map(|idx| who(g, seam::obs_source_transition_seam_get_texrender(tr, idx))),
            held: (
                held(
                    seam::obs_source_transition_seam_trylock,
                    seam::obs_source_transition_seam_unlock,
                ),
                held(
                    seam::obs_source_transition_seam_trylock_textures,
                    seam::obs_source_transition_seam_unlock_textures,
                ),
            ),
            refs: g.src.map(|s| slots::oracle_slots_refs(s)),
            log,
            trace,
        }
    }
}

/// The callback a call is handed.
fn callback(on: bool) -> Option<c::RenderCallback> {
    on.then_some(c::oracle_video_callback as c::RenderCallback)
}

fn run(g: &Graph, side: Side, op: Op) -> Option<i64> {
    // SAFETY: the graph's live sources (or NULL where a public call takes
    // it, which the C's colour space read never gets).
    unsafe {
        let placeholder = |on: bool| {
            if on {
                c::oracle_video_texture(TRANSPARENT - 1)
            } else {
                ptr::null_mut()
            }
        };
        match (side, op) {
            (Side::C, Op::Render(t, cb)) => {
                c::oracle_obs_transition_video_render(g.target(t), callback(cb));
                None
            }
            (Side::Rust, Op::Render(t, cb)) => {
                rs::render::<Oracle>(g.target(t), callback(cb));
                None
            }
            (Side::C, Op::Render2(t, cb, p)) => {
                c::oracle_obs_transition_video_render2(g.target(t), callback(cb), placeholder(p));
                None
            }
            (Side::Rust, Op::Render2(t, cb, p)) => {
                rs::render2::<Oracle>(g.target(t), callback(cb), placeholder(p));
                None
            }
            (Side::C, Op::ColorSpace(t)) => Some(i64::from(
                c::oracle_obs_transition_video_get_color_space(g.target(t)),
            )),
            (Side::Rust, Op::ColorSpace(t)) => {
                Some(i64::from(rs::get_color_space::<Oracle>(g.target(t))))
            }
            (Side::C, Op::Direct(t, target)) => Some(i64::from(
                c::oracle_obs_transition_video_render_direct(g.target(t), target),
            )),
            (Side::Rust, Op::Direct(t, target)) => {
                Some(i64::from(rs::render_direct::<Oracle>(g.target(t), target)))
            }
        }
    }
}

/// Runs `ops` on a fresh graph through `side`, a snapshot after each.
fn trace_side(init: &Init, ops: &[Op], side: Side) -> Vec<Snapshot> {
    let g = Graph::new(init);
    let tr = g.src[TR];
    let mut out = Vec::new();
    for &op in ops {
        if side == Side::C && op.checks_argument() == Some(Target::Null) {
            // The C crashes on NULL: nothing to compare, the Rust side is
            // checked against the graph as it stood.
            out.push(snapshot(&g, None));
            continue;
        }
        // SAFETY: the graph's transition; held around the call, as by
        // another thread.
        if init.tex_busy {
            assert_eq!(
                unsafe { seam::obs_source_transition_seam_trylock_textures(tr) },
                0
            );
        }
        let r = run(&g, side, op);
        if init.tex_busy {
            // SAFETY: as above.
            assert_eq!(
                unsafe { seam::obs_source_transition_seam_unlock_textures(tr) },
                0
            );
        }
        out.push(snapshot(&g, r));
    }
    out
}

/// Checks `ops` on both sides: equal after every call, but for the colour
/// space read's argument check, which leaves the Rust side as it stood and
/// returns sRGB, and which the C side is checked against on its own.
fn check(init: &Init, ops: &[Op]) {
    let _graph = graph_lock();
    check_locked(init, ops);
}

/// [`check`], with the graph lock held.
fn check_locked(init: &Init, ops: &[Op]) {
    let c_side = trace_side(init, ops, Side::C);
    let r_side = trace_side(init, ops, Side::Rust);
    let mut before = {
        let g = Graph::new(init);
        snapshot(&g, None)
    };
    for (i, &op) in ops.iter().enumerate() {
        let (cs, rs_) = (&c_side[i], &r_side[i]);
        let ctx = || format!("call {i} {op:?} of {ops:?} on {init:?}");
        match op.checks_argument() {
            None => assert_eq!(cs, rs_, "{}", ctx()),
            Some(target) => {
                let mut want = before.clone();
                want.result = Some(i64::from(SRGB));
                assert_eq!(rs_, &want, "{}", ctx());
                if target == Target::NotTr {
                    // The C mixes the input's slots: one colour space read
                    // per child, and a mixed space.
                    let new = &cs.log[before.log.len()..];
                    let reads = init.slots.iter().filter(|s| s.is_some()).count();
                    assert_eq!(new.len(), reads, "{}", ctx());
                    assert!(new.iter().all(|e| e.kind == kind::COLOR_SPACE), "{}", ctx());
                    let mut mixed = SRGB;
                    for s in init.slots.iter().flatten() {
                        mixed = obs_source::transition_clock::mix_spaces(mixed, init.spaces[*s]);
                    }
                    assert_eq!(cs.result, Some(i64::from(mixed)), "{}", ctx());
                }
                // The rest of the sequence runs on the C's graph as it now
                // is: the read changed nothing else there.
                let mut rest = cs.clone();
                rest.log.truncate(before.log.len());
                rest.result = want.result;
                assert_eq!(rest, want, "{}", ctx());
            }
        }
        before = rs_.clone();
        if op.checks_argument() == Some(Target::NotTr) {
            // The two logs part here; compare the rest from fresh graphs.
            for &later in &ops[i + 1..] {
                check_locked(init, &[later]);
            }
            return;
        }
    }
}

fn matrix(seed: f32) -> [f32; 16] {
    let mut m = [0.0; 16];
    for (i, v) in m.iter_mut().enumerate() {
        *v = seed + f32::from(u8::try_from(i).unwrap_or(0)) * 0.5;
    }
    m
}

/// A graph mid-transition: both slots full, both flags set, both
/// texrenders there with the sRGB format, everything else ordinary.
fn base() -> Init {
    Init {
        slots: [Some(CH_A), Some(CH_B)],
        tv: true,
        ta: true,
        matrices: [matrix(1.0), matrix(-3.25)],
        texrenders: [Some(0), Some(1)],
        fakes: [Fake {
            format: RGBA,
            no_texture: false,
        }; 4],
        time: 0.5,
        cx: 1920,
        cy: 1080,
        active: true,
        current_space: SRGB,
        begins: true,
        spaces: [SRGB; SOURCES],
        tex_busy: false,
    }
}

const ALL_OPS: [Op; 12] = [
    Op::Render(Target::Tr, true),
    Op::Render(Target::Tr, false),
    Op::Render2(Target::Tr, true, true),
    Op::Render2(Target::Tr, true, false),
    Op::Render2(Target::Tr, false, true),
    Op::ColorSpace(Target::Tr),
    Op::Direct(Target::Tr, 0),
    Op::Direct(Target::Tr, 1),
    Op::Direct(Target::Tr, 2),
    Op::Direct(Target::Tr, -1),
    Op::Render(Target::NotTr, true),
    Op::Direct(Target::NotTr, 1),
];

#[test]
fn every_call_on_every_flag_time_and_active_state() {
    for tv in [false, true] {
        for ta in [false, true] {
            for time in [0.5, 1.0, 1.5, f32::NAN] {
                for active in [false, true] {
                    for tex_busy in [false, true] {
                        let init = Init {
                            tv,
                            ta,
                            time,
                            active,
                            tex_busy,
                            ..base()
                        };
                        for op in ALL_OPS {
                            check(&init, &[op]);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_slot_state_and_the_same_child_twice() {
    let states = [None, Some(CH_A), Some(CH_B)];
    for a in states {
        for b in states {
            for time in [0.5, 1.0] {
                for ta in [false, true] {
                    let init = Init {
                        slots: [a, b],
                        time,
                        ta,
                        ..base()
                    };
                    for op in ALL_OPS {
                        check(&init, &[op]);
                    }
                }
            }
        }
    }
}

#[test]
fn the_texrenders_formats_textures_and_begin() {
    for texrenders in [
        [Some(0), Some(1)],
        [None, Some(1)],
        [Some(2), None],
        [None, None],
    ] {
        for (space, current) in [(SRGB, SRGB), (EXTENDED, SRGB_16F), (SCRGB, SRGB), (7, -1)] {
            for no_texture in [false, true] {
                for begins in [false, true] {
                    let mut init = Init {
                        texrenders,
                        begins,
                        current_space: current,
                        ..base()
                    };
                    init.spaces[TR] = space;
                    init.fakes[0].no_texture = no_texture;
                    init.fakes[2].format = RGBA16F;
                    for op in [
                        Op::Render(Target::Tr, true),
                        Op::Render2(Target::Tr, true, false),
                        Op::Render2(Target::Tr, true, true),
                    ] {
                        check(&init, &[op]);
                    }
                }
            }
        }
    }
}

#[test]
fn the_size_and_the_colour_space_mix() {
    for (cx, cy) in [(0, 0), (0, 1080), (1920, 0), (1, 1), (u32::MAX, 3)] {
        let init = Init { cx, cy, ..base() };
        check(&init, &[Op::Render(Target::Tr, true)]);
    }
    let spaces = [SRGB, SRGB_16F, EXTENDED, SCRGB, 9, -4];
    for a in spaces {
        for b in spaces {
            let mut init = base();
            init.spaces[CH_A] = a;
            init.spaces[CH_B] = b;
            check(&init, &[Op::ColorSpace(Target::Tr)]);
        }
    }
}

#[test]
fn a_frame_sequence_through_the_stop() {
    let init = Init {
        ta: false,
        time: 1.0,
        ..base()
    };
    check(
        &init,
        &[
            Op::Render(Target::Tr, true),
            Op::Render(Target::Tr, true),
            Op::Direct(Target::Tr, 1),
            Op::ColorSpace(Target::Tr),
        ],
    );
    let init = Init {
        active: false,
        time: 2.0,
        ..base()
    };
    check(
        &init,
        &[
            Op::Direct(Target::Tr, 1),
            Op::Direct(Target::Tr, 1),
            Op::Render2(Target::Tr, true, true),
        ],
    );
}

#[test]
fn the_argument_checks() {
    // R118-1: the colour space read on NULL and on a source that is not a
    // transition; R118-2: the direct render on NULL (its log name is not
    // recorded) and the renders, which already checked.
    for slots in [[Some(CH_A), Some(CH_B)], [None, Some(CH_B)], [None, None]] {
        let mut init = Init { slots, ..base() };
        init.spaces[CH_A] = EXTENDED;
        init.spaces[CH_B] = SRGB_16F;
        for op in [
            Op::ColorSpace(Target::Null),
            Op::ColorSpace(Target::NotTr),
            Op::Direct(Target::Null, 1),
            Op::Render(Target::Null, true),
            Op::Render2(Target::Null, true, true),
            Op::Direct(Target::NotTr, 0),
            Op::Render2(Target::NotTr, true, true),
        ] {
            check(&init, &[op]);
        }
        check(
            &init,
            &[
                Op::ColorSpace(Target::Null),
                Op::Render(Target::Tr, true),
                Op::ColorSpace(Target::NotTr),
                Op::Direct(Target::Tr, 1),
            ],
        );
    }
}

#[test]
fn the_format_from_space_inline() {
    for space in (-8..16).chain([i32::MIN, i32::MAX]) {
        // SAFETY: the inline instantiated, on any int.
        let c = unsafe { c::oracle_gs_get_format_from_space(space) };
        assert_eq!(format_from_space(space), c, "{space}");
    }
}

fn fake_strategy() -> impl Strategy<Value = Fake> {
    (
        prop_oneof![Just(RGBA), Just(RGBA16F), Just(0), Just(5)],
        any::<bool>(),
    )
        .prop_map(|(format, no_texture)| Fake { format, no_texture })
}

fn init_strategy() -> impl Strategy<Value = Init> {
    let child = prop_oneof![Just(None), Just(Some(CH_A)), Just(Some(CH_B))];
    let tr = prop_oneof![Just(None), (0usize..4).prop_map(Some)];
    let space = prop_oneof![
        Just(SRGB),
        Just(SRGB_16F),
        Just(EXTENDED),
        Just(SCRGB),
        -2i32..6
    ];
    let small = prop_oneof![Just(0u32), 1u32..4, Just(1920), any::<u32>()];
    (
        (
            [child.clone(), child],
            any::<bool>(),
            any::<bool>(),
            [tr.clone(), tr],
        ),
        (
            [
                fake_strategy(),
                fake_strategy(),
                fake_strategy(),
                fake_strategy(),
            ],
            prop_oneof![Just(1.0f32), Just(f32::NAN), -1.0f32..2.0],
            small.clone(),
            small,
        ),
        (
            any::<bool>(),
            space.clone(),
            any::<bool>(),
            [space.clone(), space.clone(), space.clone(), space],
            any::<bool>(),
            (-1.0e3f32..1.0e3, -1.0e3f32..1.0e3),
        ),
    )
        .prop_map(
            |(
                (slots, tv, ta, texrenders),
                (fakes, time, cx, cy),
                (active, current_space, begins, spaces, tex_busy, (m0, m1)),
            )| Init {
                slots,
                tv,
                ta,
                matrices: [matrix(m0), matrix(m1)],
                texrenders,
                fakes,
                time,
                cx,
                cy,
                active,
                current_space,
                begins,
                spaces,
                tex_busy,
            },
        )
}

fn op_strategy() -> impl Strategy<Value = Op> {
    let target = prop_oneof![
        8 => Just(Target::Tr),
        1 => Just(Target::NotTr),
        1 => Just(Target::Null),
    ];
    prop_oneof![
        (target.clone(), any::<bool>()).prop_map(|(t, cb)| Op::Render(t, cb)),
        (target.clone(), any::<bool>(), any::<bool>()).prop_map(|(t, cb, p)| Op::Render2(t, cb, p)),
        target.clone().prop_map(Op::ColorSpace),
        (target, -2i32..4).prop_map(|(t, target)| Op::Direct(t, target)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(256)))]

    #[test]
    fn random_graphs_and_calls(
        init in init_strategy(),
        ops in proptest::collection::vec(op_strategy(), 1..4),
    ) {
        check(&init, &ops);
    }
}
