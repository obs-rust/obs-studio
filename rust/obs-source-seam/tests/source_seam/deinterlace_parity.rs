//! Tier 3 for `obs_source_seam::ffi::source_deinterlace` against the
//! original `libobs/obs-source-deinterlace.c.h`
//! (`obs_c_oracle::source_deinterlace`, renamed `oracle_*`).
//!
//! Each side gets its own copy of the same world: a test source (a zeroed
//! `struct obs_source` with a default `async_mutex`), its queue of test
//! frames (each with a timestamp, a `prev_frame` flag and one reference),
//! a current and a previous frame in their slots or none, and every field
//! the deinterlace reads: the clock fields, the texture fields (fake
//! textures and texrenders), the effect, the mode and the field order. The
//! core's frame interval, video time and empty effect slots are the
//! oracle's. The C reads the fields and takes `async_mutex` directly and
//! calls the oracle's recording stand-ins; the Rust shims are run with
//! [`Oracle`], whose `Calls` are those same stand-ins, and reach the fields
//! and the lock through the C deinterlace seam, as in the hybrid libobs.
//! The selection, pacing and draw decisions are the Rust ones on both
//! sides, as in the hybrid libobs. Every stand-in records whether the
//! source's `async_mutex` was held and how deep the graphics context was.
//!
//! Each world runs a sequence of calls: the frame take (under
//! `async_mutex`, as `async_tick()` holds it), the texture update, the
//! render, and the mode and field-order setters (on the source, or on
//! NULL). After every call the two sides must agree on every field, the
//! queue and each frame's `prev_frame` and references (frames by their
//! index in the world), the core's effect slots, and every call the
//! stand-ins logged, in order, with its arguments (the floats bit for bit),
//! the lock state and the graphics depth. The inputs are fixed worlds for
//! each path, and a proptest over random worlds: queues of 0 to 40 frames
//! (past the C's 30-frame stack copy) whose timestamps step by nothing, a
//! field, a little, past `MAX_TS_VAR`, or backwards, from a base anywhere
//! in `u64`; buffered and unbuffered; every clock field; any mode, in the
//! enum or not; and every answer of the stand-ins.
//!
//! No intentional differences from C.

use std::ffi::{CStr, c_char, c_int, c_long, c_void};
use std::ptr;

use obs_c_oracle::parity::cases;
use obs_c_oracle::source_deinterlace::{self as c, Answers, HANDLES, State};
use obs_source_seam::ffi::source_deinterlace::{self as rs, Calls, Vec2};
use proptest::prelude::*;

const MS: u64 = 1_000_000;
const FIELD: u64 = 16_666_667;
const T: u64 = 1_000_000_000_000;

/// The oracle's stand-ins, as the Rust shims' libobs calls.
enum Oracle {}

// SAFETY: each is the stand-in the C oracle calls under the same name.
unsafe impl Calls for Oracle {
    unsafe fn remove_async_frame(source: *mut c_void, frame: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_remove_async_frame(source, frame) }
    }
    unsafe fn set_async_texture_size(source: *mut c_void, frame: *const c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_set_async_texture_size(source, frame) }
    }
    unsafe fn update_async_textures(
        source: *mut c_void,
        frame: *const c_void,
        textures: *mut *mut c_void,
        texrender: *mut c_void,
    ) -> bool {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_update_async_textures(source, frame, textures, texrender) }
    }
    unsafe fn release_frame(source: *mut c_void, frame: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_release_frame(source, frame) }
    }
    unsafe fn set_deinterlace_texture_size(source: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_set_deinterlace_texture_size(source) }
    }
    unsafe fn load_effect(effect: *mut *mut c_void, file: *const c_char) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_load_effect(effect, file) }
    }
    unsafe fn enter_graphics() {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_enter_graphics() }
    }
    unsafe fn leave_graphics() {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_leave_graphics() }
    }
    unsafe fn sdr_white_level() -> f32 {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_sdr_white_level() }
    }
    unsafe fn effect_get_param_by_name(effect: *const c_void, name: *const c_char) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_get_param_by_name(effect, name) }
    }
    unsafe fn texrender_get_texture(texrender: *const c_void) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_texrender_get_texture(texrender) }
    }
    unsafe fn get_linear_srgb() -> bool {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_get_linear_srgb() }
    }
    unsafe fn get_color_space() -> c_int {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_get_color_space() }
    }
    unsafe fn framebuffer_srgb_enabled() -> bool {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_framebuffer_srgb_enabled() }
    }
    unsafe fn enable_framebuffer_srgb(enable: bool) {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_enable_framebuffer_srgb(enable) }
    }
    unsafe fn effect_set_texture_srgb(param: *mut c_void, texture: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_set_texture_srgb(param, texture) }
    }
    unsafe fn effect_set_texture(param: *mut c_void, texture: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_set_texture(param, texture) }
    }
    unsafe fn effect_set_float(param: *mut c_void, value: f32) {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_set_float(param, value) }
    }
    unsafe fn effect_set_int(param: *mut c_void, value: c_int) {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_set_int(param, value) }
    }
    unsafe fn effect_set_vec2(param: *mut c_void, value: *const Vec2) {
        // SAFETY: see the trait; `struct vec2` is two floats.
        unsafe { c::oracle_deint_set_vec2(param, value.cast()) }
    }
    unsafe fn effect_set_bool(param: *mut c_void, value: bool) {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_set_bool(param, value) }
    }
    unsafe fn effect_loop(effect: *mut c_void, technique: *const c_char) -> bool {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_effect_loop(effect, technique) }
    }
    unsafe fn draw_sprite(texture: *mut c_void, flip: u32, width: u32, height: u32) {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_draw_sprite(texture, flip, width, height) }
    }
    unsafe fn texture_destroy(texture: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_texture_destroy(texture) }
    }
    unsafe fn texrender_destroy(texrender: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_deint_texrender_destroy(texrender) }
    }
}

/// One call on a world.
#[derive(Debug, Clone, Copy)]
enum Op {
    /// `deinterlace_process_last_frame(source, sys_time)` under
    /// `async_mutex`.
    Process(u64),
    /// `deinterlace_update_async_video(source)`.
    Update,
    /// `deinterlace_render(source)`.
    Render,
    /// `obs_source_set_deinterlace_mode(source, mode)`.
    SetMode(c_int),
    /// `obs_source_set_deinterlace_field_order(source, order)`.
    SetFieldOrder(c_int),
    /// Both setters on NULL.
    Null(c_int),
}

/// What a world starts with. Frames are `(timestamp, prev_frame)`;
/// handles are fake ones by number.
#[derive(Debug, Clone)]
struct World {
    queued: Vec<(u64, bool)>,
    cur: Option<(u64, bool)>,
    prev: Option<(u64, bool)>,
    unbuffered: bool,
    last_sys_timestamp: u64,
    last_frame_ts: u64,
    offset: u64,
    frame_ts: u64,
    half_duration: u32,
    format: c_int,
    trc: c_int,
    width: u32,
    height: u32,
    flip: bool,
    textures: [Option<usize>; 8],
    prev_textures: [Option<usize>; 8],
    texrender: Option<usize>,
    prev_texrender: Option<usize>,
    rendered: bool,
    effect: Option<usize>,
    mode: c_int,
    top_first: bool,
    answers: Answers,
    interval: u64,
    video_time: u64,
    ops: Vec<Op>,
}

impl Default for World {
    fn default() -> Self {
        Self {
            queued: Vec::new(),
            cur: None,
            prev: None,
            unbuffered: false,
            last_sys_timestamp: 0,
            last_frame_ts: 0,
            offset: 0,
            frame_ts: 0,
            half_duration: 0,
            format: 0,
            trc: 0,
            width: 0,
            height: 0,
            flip: false,
            textures: [None; 8],
            prev_textures: [None; 8],
            texrender: None,
            prev_texrender: None,
            rendered: false,
            effect: None,
            mode: 0,
            top_first: false,
            answers: Answers {
                load_ok: true,
                params_found: true,
                white_level: 300.0,
                ..Answers::default()
            },
            interval: 33_333_333,
            video_time: T,
            ops: Vec::new(),
        }
    }
}

/// A pointer in a log or a field, as the two sides can compare it: the
/// source, a frame by its index in the world, or a fake handle (the same
/// static on both sides).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ptr {
    Null,
    Source,
    Frame(usize),
    Fixed(usize),
}

/// One logged call, with its pointers as [`Ptr`] and its name as text.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ev {
    kind: c_int,
    p: [Ptr; 2],
    name: Option<String>,
    i: [c_int; 3],
    f: [u32; 2],
    held: c_int,
    graphics: c_int,
}

/// The fields after a call, pointers as [`Ptr`].
#[derive(Debug, Clone, PartialEq)]
struct Snap {
    state: State,
    cur: Ptr,
    prev: Ptr,
    textures: [Ptr; 8],
    prev_textures: [Ptr; 8],
    texrender: Ptr,
    prev_texrender: Ptr,
    effect: Ptr,
    queue: Vec<Ptr>,
    flags: Vec<bool>,
    refs: Vec<c_long>,
    core_effects: [Ptr; 8],
}

/// A side's live copy of a world.
struct Live {
    source: *mut c_void,
    frames: Vec<*mut c_void>,
}

impl Live {
    fn ptr(&self, p: *const c_void) -> Ptr {
        if p.is_null() {
            Ptr::Null
        } else if p == self.source.cast_const() {
            Ptr::Source
        } else if let Some(i) = self.frames.iter().position(|f| f.cast_const() == p) {
            Ptr::Frame(i)
        } else {
            Ptr::Fixed(p.addr())
        }
    }
}

fn handle(make: unsafe extern "C" fn(usize) -> *mut c_void, n: Option<usize>) -> *mut c_void {
    // SAFETY: the fake handle constructors take any index.
    n.map_or(ptr::null_mut(), |n| unsafe { make(n % HANDLES) })
}

fn build(w: &World) -> Live {
    // SAFETY: the oracle's constructors; every pointer stays live until
    // `free`.
    unsafe {
        let source = c::oracle_deint_source_new();
        let mut frames = Vec::new();
        for &(ts, flag) in &w.queued {
            let f = c::oracle_deint_frame_new(ts, flag);
            c::oracle_deint_push(source, f);
            frames.push(f);
        }
        let mut slot = |f: Option<(u64, bool)>| {
            f.map_or(ptr::null_mut(), |(ts, flag)| {
                let f = c::oracle_deint_frame_new(ts, flag);
                frames.push(f);
                f
            })
        };
        let cur = slot(w.cur);
        let prev = slot(w.prev);
        let state = State {
            unbuffered: w.unbuffered,
            last_sys_timestamp: w.last_sys_timestamp,
            last_frame_ts: w.last_frame_ts,
            offset: w.offset,
            frame_ts: w.frame_ts,
            half_duration: w.half_duration,
            cur,
            prev,
            format: w.format,
            trc: w.trc,
            width: w.width,
            height: w.height,
            flip: w.flip,
            textures: w.textures.map(|t| handle(c::oracle_deint_texture, t)),
            prev_textures: w.prev_textures.map(|t| handle(c::oracle_deint_texture, t)),
            texrender: handle(c::oracle_deint_texrender, w.texrender),
            prev_texrender: handle(c::oracle_deint_texrender, w.prev_texrender),
            rendered: w.rendered,
            effect: handle(c::oracle_deint_effect, w.effect),
            mode: w.mode,
            top_first: w.top_first,
        };
        c::oracle_deint_set_state(source, &raw const state);
        Live { source, frames }
    }
}

fn snap(live: &Live) -> Snap {
    let mut state = State::default();
    // SAFETY: the live source and frames of this side.
    unsafe {
        c::oracle_deint_get_state(live.source, &raw mut state);
        let queue = (0..c::oracle_deint_queued(live.source))
            .map(|i| live.ptr(c::oracle_deint_queue(live.source, i)))
            .collect();
        let flags = live
            .frames
            .iter()
            .map(|&f| obs_source_deinterlace_seam_get_frame_prev_frame(f))
            .collect();
        let refs = live
            .frames
            .iter()
            .map(|&f| c::oracle_deint_frame_refs(f))
            .collect();
        let core_effects: [Ptr; 8] =
            std::array::from_fn(|i| live.ptr(c::oracle_deint_core_effect(i)));
        Snap {
            cur: live.ptr(state.cur),
            prev: live.ptr(state.prev),
            textures: state.textures.map(|t| live.ptr(t)),
            prev_textures: state.prev_textures.map(|t| live.ptr(t)),
            texrender: live.ptr(state.texrender),
            prev_texrender: live.ptr(state.prev_texrender),
            effect: live.ptr(state.effect),
            state: State {
                cur: ptr::null_mut(),
                prev: ptr::null_mut(),
                textures: [ptr::null_mut(); 8],
                prev_textures: [ptr::null_mut(); 8],
                texrender: ptr::null_mut(),
                prev_texrender: ptr::null_mut(),
                effect: ptr::null_mut(),
                ..state
            },
            queue,
            flags,
            refs,
            core_effects,
        }
    }
}

fn events(live: &Live) -> Vec<Ev> {
    // SAFETY: the oracle's log, read under the lock.
    unsafe {
        (0..c::oracle_deint_logged())
            .map(|i| {
                let e = *c::oracle_deint_event(i);
                Ev {
                    kind: e.kind,
                    p: e.p.map(|p| live.ptr(p)),
                    name: (!e.name.is_null())
                        .then(|| CStr::from_ptr(e.name).to_string_lossy().into_owned()),
                    i: e.i,
                    f: e.f,
                    held: e.held,
                    graphics: e.graphics,
                }
            })
            .collect()
    }
}

fn free(live: Live) {
    // SAFETY: this side's source and frames, freed once.
    unsafe {
        c::oracle_deint_source_free(live.source);
        for f in live.frames {
            c::oracle_deint_frame_free(f);
        }
    }
}

unsafe extern "C" {
    fn obs_source_deinterlace_seam_get_frame_prev_frame(frame: *const c_void) -> bool;
}

/// Runs a world on one side: the snapshot after every call, then the log.
fn run(w: &World, rust: bool) -> (Vec<Snap>, Vec<Ev>) {
    let live = build(w);
    // SAFETY: the lock is held by the caller; the answers live across the
    // call.
    unsafe { c::oracle_deint_reset(live.source, &raw const w.answers, w.interval, w.video_time) };
    let mut snaps = Vec::new();
    for op in &w.ops {
        // SAFETY: the live source of this side, with the lock async_tick()
        // holds around the take.
        unsafe {
            match (*op, rust) {
                (Op::Process(t), false) => {
                    c::oracle_deint_lock(live.source);
                    c::oracle_deinterlace_process_last_frame(live.source, t);
                    c::oracle_deint_unlock(live.source);
                }
                (Op::Process(t), true) => {
                    c::oracle_deint_lock(live.source);
                    rs::process_last_frame::<Oracle>(live.source, t);
                    c::oracle_deint_unlock(live.source);
                }
                (Op::Update, false) => c::oracle_deinterlace_update_async_video(live.source),
                (Op::Update, true) => rs::update_async_video::<Oracle>(live.source),
                (Op::Render, false) => c::oracle_deinterlace_render(live.source),
                (Op::Render, true) => rs::render::<Oracle>(live.source),
                (Op::SetMode(m), false) => {
                    c::oracle_obs_source_set_deinterlace_mode(live.source, m)
                }
                (Op::SetMode(m), true) => rs::set_mode::<Oracle>(live.source, m),
                (Op::SetFieldOrder(o), false) => {
                    c::oracle_obs_source_set_deinterlace_field_order(live.source, o);
                }
                (Op::SetFieldOrder(o), true) => rs::set_field_order::<Oracle>(live.source, o),
                (Op::Null(v), false) => {
                    c::oracle_obs_source_set_deinterlace_mode(ptr::null_mut(), v);
                    c::oracle_obs_source_set_deinterlace_field_order(ptr::null_mut(), v);
                }
                (Op::Null(v), true) => {
                    rs::set_mode::<Oracle>(ptr::null_mut(), v);
                    rs::set_field_order::<Oracle>(ptr::null_mut(), v);
                }
            }
        }
        snaps.push(snap(&live));
    }
    let log = events(&live);
    free(live);
    (snaps, log)
}

fn compare(w: &World) -> Result<(), TestCaseError> {
    let _guard = c::lock();
    let (c_snaps, c_log) = run(w, false);
    let (r_snaps, r_log) = run(w, true);
    for (i, (cs, rs)) in c_snaps.iter().zip(&r_snaps).enumerate() {
        prop_assert_eq!(cs, rs, "after call {} ({:?})", i, w.ops[i]);
    }
    prop_assert_eq!(c_log, r_log);
    Ok(())
}

fn check(w: &World) {
    compare(w).unwrap();
}

fn fields(n: usize, step: u64) -> Vec<(u64, bool)> {
    (0..n).map(|i| (T + i as u64 * step, false)).collect()
}

#[test]
fn take_paths_match_c() {
    let base = World {
        queued: fields(5, FIELD),
        ops: vec![
            Op::Process(T),
            Op::Process(T + FIELD),
            Op::Process(T + 3 * FIELD),
            Op::Process(T + 10 * FIELD),
        ],
        ..World::default()
    };
    check(&base);
    check(&World {
        unbuffered: true,
        ..base.clone()
    });
    check(&World {
        last_frame_ts: T - FIELD,
        last_sys_timestamp: 100 * MS,
        ops: vec![
            Op::Process(100 * MS + 4 * FIELD),
            Op::Process(100 * MS + 9 * FIELD),
        ],
        ..base.clone()
    });
    check(&World {
        queued: fields(40, FIELD),
        unbuffered: true,
        last_frame_ts: T,
        ops: vec![Op::Process(T + 39 * FIELD)],
        ..World::default()
    });
    check(&World {
        queued: fields(40, FIELD),
        last_frame_ts: T,
        last_sys_timestamp: 100 * MS,
        ops: vec![Op::Process(100 * MS + 35 * FIELD)],
        ..World::default()
    });
    check(&World {
        queued: vec![(T + 10_000 * MS, false), (T + 10_000 * MS + FIELD, false)],
        last_frame_ts: T,
        offset: 77,
        ops: vec![Op::Process(T)],
        ..World::default()
    });
    check(&World {
        queued: fields(2, FIELD),
        cur: Some((T - FIELD, true)),
        prev: Some((T - 2 * FIELD, false)),
        unbuffered: true,
        offset: 3 * MS,
        frame_ts: T,
        half_duration: 8_000_000,
        ops: vec![Op::Process(T + 4 * MS), Op::Process(T + 30 * MS)],
        ..World::default()
    });
}

#[test]
fn update_paths_match_c() {
    let textures = [Some(1), Some(2), None, Some(3), None, None, None, Some(4)];
    let base = World {
        cur: Some((T, false)),
        prev: Some((T - FIELD, false)),
        textures,
        prev_textures: [Some(9), None, Some(10), None, None, None, None, None],
        texrender: Some(5),
        prev_texrender: Some(6),
        ops: vec![Op::Update, Op::Update],
        ..World::default()
    };
    for ok in [false, true] {
        let mut w = base.clone();
        w.answers.texture_size_ok = ok;
        check(&w);
    }
    check(&World {
        prev: None,
        ..base.clone()
    });
    check(&World {
        prev: None,
        texrender: None,
        ..base.clone()
    });
    check(&World {
        prev: None,
        cur: None,
        ..base.clone()
    });
    check(&World {
        rendered: true,
        ..base.clone()
    });
}

#[test]
fn render_paths_match_c() {
    let base = World {
        textures: [Some(1), None, None, None, None, None, None, None],
        prev_textures: [Some(2), None, None, None, None, None, None, None],
        width: 64,
        height: 48,
        effect: Some(3),
        mode: 7,
        frame_ts: T,
        offset: 5,
        half_duration: 9,
        ops: vec![Op::Render],
        ..World::default()
    };
    for mode in [-1, 0, 1, 3, 4, 5, 6, 7, 8, 9] {
        for space in 0..4 {
            for linear in [false, true] {
                for flip in [false, true] {
                    let mut w = base.clone();
                    w.mode = mode;
                    w.answers.color_space = space;
                    w.answers.linear_srgb = linear;
                    w.answers.loops = 2;
                    w.flip = flip;
                    w.top_first = flip;
                    check(&w);
                }
            }
        }
    }
    for trc in 0..4 {
        for format in [0, 1, 3, 9, 14, 20] {
            let mut w = base.clone();
            w.trc = trc;
            w.format = format;
            w.answers.loops = 1;
            check(&w);
        }
    }
    check(&World {
        textures: [None; 8],
        ..base.clone()
    });
    check(&World {
        width: 0,
        ..base.clone()
    });
    check(&World {
        height: 0,
        ..base.clone()
    });
    check(&World {
        effect: None,
        ..base.clone()
    });
    for texrender_texture in [false, true] {
        let mut w = World {
            texrender: Some(7),
            prev_texrender: Some(8),
            ..base.clone()
        };
        w.answers.texrender_texture = texrender_texture;
        w.answers.params_found = !texrender_texture;
        check(&w);
    }
}

#[test]
fn setter_paths_match_c() {
    let base = World {
        prev: Some((T, false)),
        format: 3,
        width: 64,
        height: 48,
        prev_textures: [Some(1), Some(2), Some(3), Some(4), None, None, None, None],
        prev_texrender: Some(5),
        effect: Some(6),
        ..World::default()
    };
    let modes = [-1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 99];
    for &from in &modes {
        for &to in &modes {
            for load_ok in [false, true] {
                let mut w = World {
                    mode: from,
                    ops: vec![Op::SetMode(to), Op::SetMode(to), Op::SetMode(0)],
                    ..base.clone()
                };
                w.answers.load_ok = load_ok;
                check(&w);
            }
        }
    }
    for (format, width, height) in [(0, 64, 48), (3, 0, 48), (3, 64, 0)] {
        check(&World {
            format,
            width,
            height,
            ops: vec![Op::SetMode(4)],
            ..base.clone()
        });
    }
    check(&World {
        ops: vec![
            Op::SetFieldOrder(0),
            Op::SetFieldOrder(1),
            Op::SetFieldOrder(7),
            Op::SetFieldOrder(-1),
            Op::Null(3),
        ],
        ..base
    });
}

fn timestamps() -> impl Strategy<Value = Vec<(u64, bool)>> {
    let step = prop_oneof![
        Just(0u64),
        Just(FIELD),
        1u64..40 * MS,
        2_000_000_001u64..5_000_000_000,
        any::<u64>(),
    ];
    let base = prop_oneof![Just(T), any::<u64>(), Just(u64::MAX - 5 * FIELD)];
    (
        base,
        prop::collection::vec((step, any::<bool>(), prop::bool::weighted(0.1)), 0..=40),
    )
        .prop_map(|(base, steps)| {
            let mut ts = base;
            steps
                .into_iter()
                .map(|(step, flag, back)| {
                    ts = if back {
                        ts.wrapping_sub(step)
                    } else {
                        ts.wrapping_add(step)
                    };
                    (ts, flag)
                })
                .collect()
        })
}

fn clock() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0u64),
        Just(T),
        T - 100 * MS..T + 200 * MS,
        any::<u64>()
    ]
}

fn slot_frame() -> impl Strategy<Value = Option<(u64, bool)>> {
    prop::option::of((clock(), any::<bool>()))
}

fn handles() -> impl Strategy<Value = [Option<usize>; 8]> {
    prop::array::uniform8(prop::option::of(0..HANDLES))
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => clock().prop_map(Op::Process),
        2 => Just(Op::Update),
        2 => Just(Op::Render),
        2 => (-2..12).prop_map(Op::SetMode),
        1 => (-1..3).prop_map(Op::SetFieldOrder),
        1 => (-1..10).prop_map(Op::Null),
    ]
}

fn answers() -> impl Strategy<Value = Answers> {
    (
        any::<[bool; 6]>(),
        0..4,
        0..4,
        prop_oneof![
            Just(0.0f32),
            Just(80.0f32),
            Just(300.0f32),
            1.0f32..10_000.0
        ],
    )
        .prop_map(|(b, color_space, loops, white_level)| Answers {
            texture_size_ok: b[0],
            load_ok: b[1],
            params_found: b[2],
            texrender_texture: b[3],
            linear_srgb: b[4],
            framebuffer_srgb: b[5],
            color_space,
            loops,
            white_level,
        })
}

fn world() -> impl Strategy<Value = World> {
    (
        (timestamps(), slot_frame(), slot_frame(), any::<bool>()),
        (
            clock(),
            clock(),
            prop_oneof![Just(0u64), 0u64..100 * MS, any::<u64>()],
            clock(),
        ),
        (
            any::<u32>(),
            0..30,
            0..4,
            prop_oneof![Just(0u32), Just(1u32), Just(64u32), any::<u32>()],
        ),
        (
            prop_oneof![Just(0u32), Just(48u32), any::<u32>()],
            any::<bool>(),
            handles(),
            handles(),
        ),
        (
            prop::option::of(0..HANDLES),
            prop::option::of(0..HANDLES),
            any::<bool>(),
            prop::option::of(0..HANDLES),
        ),
        (-2..12, any::<bool>(), answers(), 1u64..50 * MS),
        (clock(), prop::collection::vec(op(), 1..=6)),
    )
        .prop_map(
            |(
                (queued, cur, prev, unbuffered),
                (last_sys_timestamp, last_frame_ts, offset, frame_ts),
                (half_duration, format, trc, width),
                (height, flip, textures, prev_textures),
                (texrender, prev_texrender, rendered, effect),
                (mode, top_first, answers, interval),
                (video_time, ops),
            )| World {
                queued,
                cur,
                prev,
                unbuffered,
                last_sys_timestamp,
                last_frame_ts,
                offset,
                frame_ts,
                half_duration: half_duration % 100_000_000,
                format,
                trc,
                width,
                height,
                flip,
                textures,
                prev_textures,
                texrender,
                prev_texrender,
                rendered,
                effect,
                mode,
                top_first,
                answers,
                interval,
                video_time,
                ops,
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(512)))]

    #[test]
    fn random_worlds_match_c(w in world()) {
        compare(&w)?;
    }
}
