//! Tier 1 tests for `obs_source::transition_video`, the safe core of
//! `libobs/obs-source-transition-video.c.h`.
//!
//! The functions run on a fake transition: its two slots, both
//! transitioning flags, matrices and texrenders, `transition_mutex` and
//! `transition_tex_mutex` (which another holder may have), a video time
//! and size, whether it is active and whether it is a transition at all,
//! a reference count per child, a colour space per child, and the graphics
//! state the calls read (each texrender's format and texture, whether a
//! texrender begins, the current colour space). The fake records every
//! field read and write (with whether `transition_mutex` was held), every
//! lock call, every `transition_valid()` and every call into the rest of
//! libobs and the graphics, in order, and panics on a lock taken twice or
//! released unheld.
//!
//! The cases pin the full call order of each draw (the callback draw, slot
//! A and slot B), the texrender re-creation, the placeholder for an empty
//! slot or a texrender with no texture, the skipped blend at size zero, the
//! texture lock that another holder has, the video stop for each flag pair
//! and active state (with its signal and handler after the locks), the
//! direct index for each target, the colour-space mix for each pair of
//! child spaces, and the argument checks: no transition and a source that
//! is not one do nothing but ask `transition_valid()`, under each
//! function's own name (the intentional differences from C: the C's colour
//! space read checks nothing, and the direct render logs the wrong name).
//! A proptest checks, over any state, that every lock and matrix push is
//! undone, every reference taken is released, the callback runs only with
//! the texture lock and without the transition lock, and the stop leaves
//! `transitioning_video` cleared.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::ffi::CStr;

use obs_c_oracle::parity::cases;
use obs_source::transition_clock::{self, CS_709_EXTENDED, CS_709_SCRGB, CS_SRGB, CS_SRGB_16F};
use obs_source::transition_video::{
    BLEND_INVSRCALPHA, BLEND_ONE, CLEAR_COLOR, GET_COLOR_SPACE_LOG_NAME, Matrix, ORTHO_Z,
    PREFERRED_SPACES, RENDER_DIRECT_LOG_NAME, RENDER_LOG_NAME, Transition, VIDEO_STOP_SIGNALS,
    ZS_NONE, get_color_space, render, render_direct, render2,
};
use proptest::prelude::*;

/// A child the fake knows.
type Id = usize;

const A: Id = 1;
const B: Id = 2;

/// A texrender the fake knows (never 0).
type Tr = u32;
/// A texture the fake knows (never 0).
type Tex = u32;

/// The core's transparent texture.
const TRANSPARENT: Tex = 900;
/// A placeholder [`render2`] is handed.
const PLACEHOLDER: Tex = 901;
/// The callback the fake is handed.
const CB: u8 = 7;

/// `OBS_TRANSITION_SOURCE_A` and `_B`.
const TARGET_A: i32 = 0;
const TARGET_B: i32 = 1;

/// The format the fake's `gs_get_format_from_space()` gives a space.
fn format_of(space: i32) -> i32 {
    100 + space
}

/// A field of the transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum F {
    Source(usize),
    Tv,
    Ta,
    Matrix(usize),
    Texrender(usize),
}

/// One thing the functions did, in order. A read or write, and the libobs
/// calls, carry whether `transition_mutex` was held; the callback and the
/// child renders carry whether the texture lock was held too.
#[derive(Debug, Clone, PartialEq)]
enum Ev {
    Valid(&'static CStr),
    R(F, bool),
    WFlag(F, bool),
    WTexrender(usize, Option<Tr>),
    Lock,
    Unlock,
    TryTex(bool),
    UnlockTex,
    VideoTime(bool),
    Stop(bool),
    HandleStop(bool),
    Cx,
    Cy,
    Active(bool),
    GetRef(Option<Id>),
    Release(Option<Id>, bool),
    Signal(&'static CStr, &'static CStr, bool),
    ColorSpace(Vec<i32>),
    ChildColorSpace(Id, Vec<i32>),
    Render(Id, bool),
    Callback {
        a: Option<Tex>,
        b: Option<Tex>,
        t: u32,
        cx: u32,
        cy: u32,
        locked: bool,
        tex_locked: bool,
    },
    TexTexture(Option<Tr>),
    TexFormat(Option<Tr>),
    TexDestroy(Option<Tr>),
    TexCreate(i32, i32),
    TexBegin(Option<Tr>, u32, u32, i32),
    TexEnd(Option<Tr>),
    FormatFromSpace(i32),
    CurrentSpace,
    Clear(u32, [u32; 4], u32, u8),
    Ortho([u32; 6]),
    Push,
    Mul([u32; 16]),
    Pop,
    BlendPush,
    BlendFn(i32, i32),
    BlendPop,
}

/// How a fake starts.
#[derive(Debug, Clone)]
struct State {
    is_transition: bool,
    sources: [Option<Id>; 2],
    tv: bool,
    ta: bool,
    matrices: [Matrix; 2],
    texrenders: [Option<Tr>; 2],
    /// Another holder has `transition_tex_mutex`.
    tex_busy: bool,
    time: f32,
    cx: u32,
    cy: u32,
    active: bool,
    /// The space `obs_source_get_color_space()` gives the transition.
    source_space: i32,
    /// The space each child gives.
    child_space: [i32; 2],
    current_space: i32,
    /// Each texrender's format (missing: the space's format).
    formats: BTreeMap<Tr, i32>,
    /// Texrenders whose texture is NULL.
    no_texture: Vec<Tr>,
    /// `gs_texrender_begin_with_color_space()` succeeds.
    begins: bool,
}

fn matrix(seed: f32) -> Matrix {
    let mut m = [0.0; 16];
    for (i, v) in m.iter_mut().enumerate() {
        *v = seed + f32::from(u8::try_from(i).unwrap_or(0));
    }
    m
}

impl Default for State {
    fn default() -> Self {
        Self {
            is_transition: true,
            sources: [Some(A), Some(B)],
            tv: false,
            ta: false,
            matrices: [matrix(10.0), matrix(50.0)],
            texrenders: [Some(11), Some(12)],
            tex_busy: false,
            time: 0.25,
            cx: 640,
            cy: 360,
            active: true,
            source_space: CS_SRGB,
            child_space: [CS_SRGB, CS_SRGB],
            current_space: CS_SRGB,
            formats: BTreeMap::new(),
            no_texture: Vec::new(),
            begins: true,
        }
    }
}

thread_local! {
    /// The core's transparent texture, read through the type.
    static TRANSPARENT_READS: Cell<usize> = const { Cell::new(0) };
}

/// The fake transition.
struct Fake {
    s: RefCell<State>,
    locked: Cell<bool>,
    tex_locked: Cell<bool>,
    refs: RefCell<BTreeMap<Id, i64>>,
    next_texrender: Cell<Tr>,
    ev: RefCell<Vec<Ev>>,
}

impl Fake {
    fn new(state: State) -> Self {
        Self {
            s: RefCell::new(state),
            locked: Cell::new(false),
            tex_locked: Cell::new(false),
            refs: RefCell::new(BTreeMap::from([(A, 1), (B, 1)])),
            next_texrender: Cell::new(40),
            ev: RefCell::new(Vec::new()),
        }
    }
    fn push(&self, ev: Ev) {
        self.ev.borrow_mut().push(ev);
    }
    fn events(&self) -> Vec<Ev> {
        self.ev.borrow().clone()
    }
    fn held(&self) -> bool {
        self.locked.get()
    }
    fn refs(&self) -> BTreeMap<Id, i64> {
        self.refs.borrow().clone()
    }
}

fn bits(m: &Matrix) -> [u32; 16] {
    m.map(f32::to_bits)
}

impl Transition for Fake {
    type Child = Id;
    type Texrender = Tr;
    type Texture = Tex;
    type Callback = u8;

    fn valid(source: Option<&Self>, func: &'static CStr) -> bool {
        let Some(s) = source else { return false };
        s.push(Ev::Valid(func));
        s.s.borrow().is_transition
    }

    fn source(&self, idx: usize) -> Option<Id> {
        self.push(Ev::R(F::Source(idx), self.held()));
        self.s.borrow().sources[idx]
    }
    fn transitioning_video(&self) -> bool {
        self.push(Ev::R(F::Tv, self.held()));
        self.s.borrow().tv
    }
    fn set_transitioning_video(&self, value: bool) {
        self.push(Ev::WFlag(F::Tv, value));
        self.s.borrow_mut().tv = value;
    }
    fn transitioning_audio(&self) -> bool {
        self.push(Ev::R(F::Ta, self.held()));
        self.s.borrow().ta
    }
    fn set_transitioning_audio(&self, value: bool) {
        self.push(Ev::WFlag(F::Ta, value));
        self.s.borrow_mut().ta = value;
    }
    fn matrix(&self, idx: usize) -> Matrix {
        self.push(Ev::R(F::Matrix(idx), self.held()));
        self.s.borrow().matrices[idx]
    }
    fn texrender(&self, idx: usize) -> Option<Tr> {
        self.push(Ev::R(F::Texrender(idx), self.held()));
        self.s.borrow().texrenders[idx]
    }
    fn set_texrender(&self, idx: usize, value: Option<Tr>) {
        self.push(Ev::WTexrender(idx, value));
        self.s.borrow_mut().texrenders[idx] = value;
    }

    fn lock(&self) {
        assert!(!self.locked.get(), "transition_mutex taken twice");
        self.locked.set(true);
        self.push(Ev::Lock);
    }
    fn unlock(&self) {
        assert!(self.locked.get(), "transition_mutex released unheld");
        self.locked.set(false);
        self.push(Ev::Unlock);
    }
    fn trylock_textures(&self) -> bool {
        assert!(!self.tex_locked.get(), "transition_tex_mutex tried twice");
        let got = !self.s.borrow().tex_busy;
        self.tex_locked.set(got);
        self.push(Ev::TryTex(got));
        got
    }
    fn unlock_textures(&self) {
        assert!(
            self.tex_locked.get(),
            "transition_tex_mutex released unheld"
        );
        self.tex_locked.set(false);
        self.push(Ev::UnlockTex);
    }

    fn video_time(&self) -> f32 {
        self.push(Ev::VideoTime(self.held()));
        self.s.borrow().time
    }
    fn stop(&self) {
        self.push(Ev::Stop(self.held()));
        // transition_stop(): slot B moves into slot A.
        let mut s = self.s.borrow_mut();
        let old = s.sources[0];
        if let Some(old) = old {
            *self.refs.borrow_mut().entry(old).or_default() -= 1;
        }
        s.sources[0] = s.sources[1];
        s.sources[1] = None;
    }
    fn handle_stop(&self) {
        self.push(Ev::HandleStop(self.held()));
    }
    fn cx(&self) -> u32 {
        self.push(Ev::Cx);
        self.s.borrow().cx
    }
    fn cy(&self) -> u32 {
        self.push(Ev::Cy);
        self.s.borrow().cy
    }

    fn active(&self) -> bool {
        self.push(Ev::Active(self.held()));
        self.s.borrow().active
    }
    fn get_ref(&self, child: Option<Id>) -> Option<Id> {
        self.push(Ev::GetRef(child));
        let c = child?;
        *self.refs.borrow_mut().entry(c).or_default() += 1;
        Some(c)
    }
    fn release(&self, child: Option<Id>) {
        self.push(Ev::Release(child, self.held()));
        if let Some(c) = child {
            *self.refs.borrow_mut().entry(c).or_default() -= 1;
        }
    }
    fn dosignal(&self, signal_obs: &'static CStr, signal_source: &'static CStr) {
        self.push(Ev::Signal(signal_obs, signal_source, self.held()));
    }
    fn color_space(&self, preferred: &[i32]) -> i32 {
        self.push(Ev::ColorSpace(preferred.to_vec()));
        self.s.borrow().source_space
    }
    fn child_color_space(&self, child: Id, preferred: &[i32]) -> i32 {
        self.push(Ev::ChildColorSpace(child, preferred.to_vec()));
        let s = self.s.borrow();
        if child == A {
            s.child_space[0]
        } else {
            s.child_space[1]
        }
    }
    fn video_render(&self, child: Id) {
        self.push(Ev::Render(child, self.tex_locked.get()));
    }
    fn call_render(&self, callback: u8, a: Option<Tex>, b: Option<Tex>, t: f32, cx: u32, cy: u32) {
        assert_eq!(callback, CB);
        self.push(Ev::Callback {
            a,
            b,
            t: t.to_bits(),
            cx,
            cy,
            locked: self.held(),
            tex_locked: self.tex_locked.get(),
        });
    }
    fn transparent_texture() -> Option<Tex> {
        TRANSPARENT_READS.with(|r| r.set(r.get() + 1));
        Some(TRANSPARENT)
    }

    fn texrender_texture(&self, texrender: Option<Tr>) -> Option<Tex> {
        self.push(Ev::TexTexture(texrender));
        let t = texrender?;
        (!self.s.borrow().no_texture.contains(&t)).then_some(t + 500)
    }
    fn texrender_format(&self, texrender: Option<Tr>) -> i32 {
        self.push(Ev::TexFormat(texrender));
        let s = self.s.borrow();
        texrender.map_or(0, |t| {
            s.formats
                .get(&t)
                .copied()
                .unwrap_or(format_of(s.source_space))
        })
    }
    fn texrender_destroy(&self, texrender: Option<Tr>) {
        self.push(Ev::TexDestroy(texrender));
    }
    fn texrender_create(&self, format: i32, zsformat: i32) -> Option<Tr> {
        self.push(Ev::TexCreate(format, zsformat));
        let t = self.next_texrender.get();
        self.next_texrender.set(t + 1);
        self.s.borrow_mut().formats.insert(t, format);
        Some(t)
    }
    fn texrender_begin(&self, texrender: Option<Tr>, cx: u32, cy: u32, space: i32) -> bool {
        self.push(Ev::TexBegin(texrender, cx, cy, space));
        self.s.borrow().begins
    }
    fn texrender_end(&self, texrender: Option<Tr>) {
        self.push(Ev::TexEnd(texrender));
    }
    fn format_from_space(&self, space: i32) -> i32 {
        self.push(Ev::FormatFromSpace(space));
        format_of(space)
    }
    fn current_color_space(&self) -> i32 {
        self.push(Ev::CurrentSpace);
        self.s.borrow().current_space
    }
    fn clear(&self, flags: u32, color: [f32; 4], depth: f32, stencil: u8) {
        self.push(Ev::Clear(
            flags,
            color.map(f32::to_bits),
            depth.to_bits(),
            stencil,
        ));
    }
    fn ortho(&self, left: f32, right: f32, top: f32, bottom: f32, znear: f32, zfar: f32) {
        self.push(Ev::Ortho(
            [left, right, top, bottom, znear, zfar].map(f32::to_bits),
        ));
    }
    fn matrix_push(&self) {
        self.push(Ev::Push);
    }
    fn matrix_mul(&self, matrix: &Matrix) {
        self.push(Ev::Mul(bits(matrix)));
    }
    fn matrix_pop(&self) {
        self.push(Ev::Pop);
    }
    fn blend_state_push(&self) {
        self.push(Ev::BlendPush);
    }
    fn blend_function(&self, src: i32, dest: i32) {
        self.push(Ev::BlendFn(src, dest));
    }
    fn blend_state_pop(&self) {
        self.push(Ev::BlendPop);
    }
}

use Ev::*;
use F::{Matrix as M, Source as S, Ta, Texrender as T, Tv};

/// The reads and calls of the state copy, under the lock.
fn copy(sources: [Option<Id>; 2]) -> Vec<Ev> {
    vec![
        R(S(0), true),
        GetRef(sources[0]),
        R(S(1), true),
        GetRef(sources[1]),
        R(Tv, true),
        R(Ta, true),
        R(M(0), true),
        R(M(1), true),
    ]
}

/// The draw of one child with its matrix.
fn draw_one(child: Id, m: &Matrix, tex_locked: bool) -> Vec<Ev> {
    vec![Push, Mul(bits(m)), Render(child, tex_locked), Pop]
}

/// `render_child()` of slot `idx` into texrender `tr` (whose format fits),
/// then `get_texture()`.
fn render_child(idx: usize, child: Id, tr: Tr, space: i32, m: &Matrix, st: &State) -> Vec<Ev> {
    vec![
        Cx,
        Cy,
        FormatFromSpace(space),
        R(T(idx), false),
        TexFormat(Some(tr)),
        R(T(idx), false),
        TexBegin(Some(tr), st.cx, st.cy, space),
        Clear(CLEAR_COLOR, [0; 4], 0.0f32.to_bits(), 0),
        Ortho(
            [
                0.0,
                f32::from(u16::try_from(st.cx).unwrap_or(0)),
                0.0,
                f32::from(u16::try_from(st.cy).unwrap_or(0)),
                ORTHO_Z.0,
                ORTHO_Z.1,
            ]
            .map(f32::to_bits),
        ),
        Push,
        Mul(bits(m)),
        Render(child, true),
        Pop,
        R(T(idx), false),
        TexEnd(Some(tr)),
        R(T(idx), false),
        TexTexture(Some(tr)),
    ]
}

fn run2(st: State, cb: Option<u8>) -> Fake {
    let fake = Fake::new(st);
    render2(Some(&fake), cb, Some(PLACEHOLDER));
    fake
}

#[test]
fn render2_idle_draws_slot_a_with_its_matrix() {
    let st = State::default();
    let m0 = st.matrices[0];
    let fake = run2(st, Some(CB));
    let mut want = vec![Valid(RENDER_LOG_NAME), Lock, VideoTime(true), R(Tv, true)];
    want.extend(copy([Some(A), Some(B)]));
    want.push(Unlock);
    want.extend(draw_one(A, &m0, false));
    want.extend([Release(Some(A), false), Release(Some(B), false)]);
    assert_eq!(fake.events(), want);
    assert_eq!(fake.refs(), BTreeMap::from([(A, 1), (B, 1)]));
}

#[test]
fn render2_audio_only_draws_slot_b() {
    let st = State {
        ta: true,
        ..State::default()
    };
    let m1 = st.matrices[1];
    let fake = run2(st, Some(CB));
    let mut want = vec![Valid(RENDER_LOG_NAME), Lock, VideoTime(true), R(Tv, true)];
    want.extend(copy([Some(A), Some(B)]));
    want.push(Unlock);
    want.extend(draw_one(B, &m1, false));
    want.extend([Release(Some(A), false), Release(Some(B), false)]);
    assert_eq!(fake.events(), want);
}

#[test]
fn render2_callback_draw_renders_both_children_and_blends() {
    let st = State {
        tv: true,
        time: 0.5,
        current_space: CS_SRGB_16F,
        source_space: CS_709_EXTENDED,
        ..State::default()
    };
    let (m0, m1) = (st.matrices[0], st.matrices[1]);
    let fake = run2(st.clone(), Some(CB));
    let mut want = vec![Valid(RENDER_LOG_NAME), Lock, VideoTime(true), R(Tv, true)];
    want.extend(copy([Some(A), Some(B)]));
    want.extend([
        Unlock,
        TryTex(true),
        CurrentSpace,
        ColorSpace(vec![CS_SRGB_16F]),
    ]);
    want.extend(render_child(0, A, 11, CS_709_EXTENDED, &m0, &st));
    want.extend(render_child(1, B, 12, CS_709_EXTENDED, &m1, &st));
    want.extend([
        Cx,
        Cy,
        BlendPush,
        BlendFn(BLEND_ONE, BLEND_INVSRCALPHA),
        Callback {
            a: Some(511),
            b: Some(512),
            t: 0.5f32.to_bits(),
            cx: 640,
            cy: 360,
            locked: false,
            tex_locked: true,
        },
        BlendPop,
        UnlockTex,
        Release(Some(A), false),
        Release(Some(B), false),
    ]);
    assert_eq!(fake.events(), want);
    assert_eq!(fake.refs(), BTreeMap::from([(A, 1), (B, 1)]));
}

#[test]
fn render2_recreates_a_texrender_whose_format_no_longer_fits() {
    let mut st = State {
        tv: true,
        ..State::default()
    };
    st.formats.insert(11, 999);
    let fake = run2(st, Some(CB));
    let ev = fake.events();
    let at = ev
        .iter()
        .position(|e| *e == TexFormat(Some(11)))
        .unwrap_or(usize::MAX);
    assert_eq!(
        ev.get(at..at + 6),
        Some(
            &[
                TexFormat(Some(11)),
                R(T(0), false),
                TexDestroy(Some(11)),
                TexCreate(format_of(CS_SRGB), ZS_NONE),
                WTexrender(0, Some(40)),
                R(T(0), false),
            ][..]
        )
    );
    assert!(ev.contains(&TexBegin(Some(40), 640, 360, CS_SRGB)));
    assert!(ev.contains(&TexEnd(Some(40))));
    assert!(ev.contains(&TexTexture(Some(40))));
    // slot B's fits, and stays.
    assert!(!ev.contains(&TexDestroy(Some(12))));
    assert_eq!(fake.s.borrow().texrenders, [Some(40), Some(12)]);
}

#[test]
fn render2_hands_the_placeholder_for_an_empty_slot_or_no_texture() {
    for (sources, no_texture, want) in [
        ([None, Some(B)], vec![], (Some(PLACEHOLDER), Some(512))),
        ([Some(A), None], vec![], (Some(511), Some(PLACEHOLDER))),
        ([None, None], vec![], (Some(PLACEHOLDER), Some(PLACEHOLDER))),
        ([Some(A), Some(B)], vec![11], (Some(PLACEHOLDER), Some(512))),
        ([Some(A), Some(B)], vec![12], (Some(511), Some(PLACEHOLDER))),
    ] {
        let st = State {
            tv: true,
            sources,
            no_texture,
            ..State::default()
        };
        let fake = run2(st, Some(CB));
        let ev = fake.events();
        let cb = ev.iter().find_map(|e| match e {
            Callback { a, b, .. } => Some((*a, *b)),
            _ => None,
        });
        assert_eq!(cb, Some(want), "{sources:?}");
        for (i, s) in sources.iter().enumerate() {
            let rendered = ev.iter().any(|e| *e == Render(i + 1, true));
            assert_eq!(rendered, s.is_some(), "{sources:?}");
        }
    }
}

#[test]
fn render2_skips_the_blend_and_callback_at_size_zero() {
    for (cx, cy) in [(0, 360), (640, 0), (0, 0)] {
        let st = State {
            tv: true,
            cx,
            cy,
            ..State::default()
        };
        let fake = run2(st, Some(CB));
        let ev = fake.events();
        assert!(ev.contains(&Render(A, true)) && ev.contains(&Render(B, true)));
        assert!(!ev.iter().any(|e| matches!(e, Callback { .. } | BlendPush)));
        assert_eq!(
            ev.iter().rev().take(3).cloned().collect::<Vec<_>>(),
            vec![Release(Some(B), false), Release(Some(A), false), UnlockTex]
        );
    }
}

#[test]
fn render2_without_the_texture_lock_or_callback_draws_one_child() {
    for (tex_busy, cb, ta, want) in [
        (true, Some(CB), false, A),
        (true, Some(CB), true, B),
        (false, None, false, A),
        (false, None, true, B),
    ] {
        let st = State {
            tv: true,
            ta,
            tex_busy,
            ..State::default()
        };
        let m = st.matrices[usize::from(want == B)];
        let fake = run2(st, cb);
        let ev = fake.events();
        let at = ev.iter().position(|e| *e == Unlock).unwrap_or(usize::MAX) + 1;
        let mut tail = vec![TryTex(!tex_busy)];
        tail.extend(draw_one(want, &m, !tex_busy));
        if !tex_busy {
            tail.push(UnlockTex);
        }
        tail.extend([Release(Some(A), false), Release(Some(B), false)]);
        assert_eq!(ev.get(at..), Some(&tail[..]), "{tex_busy} {cb:?} {ta}");
    }
}

#[test]
fn render2_does_not_end_or_clear_a_texrender_that_did_not_begin() {
    let st = State {
        tv: true,
        begins: false,
        ..State::default()
    };
    let fake = run2(st, Some(CB));
    let ev = fake.events();
    assert!(ev.contains(&TexBegin(Some(11), 640, 360, CS_SRGB)));
    assert!(
        !ev.iter()
            .any(|e| matches!(e, TexEnd(_) | Clear(..) | Ortho(_) | Render(..)))
    );
    assert!(ev.iter().any(|e| matches!(e, Callback { .. })));
}

#[test]
fn render2_video_stop_stops_unless_the_audio_runs_on() {
    for ta in [false, true] {
        let st = State {
            tv: true,
            ta,
            time: 1.0,
            ..State::default()
        };
        let fake = run2(st, Some(CB));
        let ev = fake.events();
        let mut want = vec![
            Valid(RENDER_LOG_NAME),
            Lock,
            VideoTime(true),
            R(Tv, true),
            WFlag(Tv, false),
            R(Ta, true),
        ];
        if !ta {
            want.push(Stop(true));
        }
        let sources = if ta {
            [Some(A), Some(B)]
        } else {
            [Some(B), None]
        };
        want.extend(copy(sources));
        want.push(Unlock);
        assert_eq!(ev.get(..want.len()), Some(&want[..]), "{ta}");
        let mut tail = vec![
            Release(sources[0], false),
            Release(sources[1], false),
            Signal(VIDEO_STOP_SIGNALS.0, VIDEO_STOP_SIGNALS.1, false),
        ];
        if !ta {
            tail.push(HandleStop(false));
        }
        assert_eq!(ev.get(ev.len() - tail.len()..), Some(&tail[..]), "{ta}");
        // the copy saw the cleared flag: no trylock, one child drawn.
        assert!(!ev.iter().any(|e| matches!(e, TryTex(_))));
        assert!(!fake.s.borrow().tv);
    }
}

#[test]
fn render2_video_stop_waits_for_the_time() {
    for time in [1.0 - f32::EPSILON / 2.0, f32::NAN, -1.0] {
        let st = State {
            tv: true,
            time,
            ..State::default()
        };
        let fake = run2(st, Some(CB));
        let ev = fake.events();
        assert!(
            !ev.iter()
                .any(|e| matches!(e, WFlag(..) | Stop(_) | Signal(..)))
        );
        assert!(fake.s.borrow().tv);
    }
    // due only while the video transitions
    let st = State {
        time: 2.0,
        ..State::default()
    };
    let ev = run2(st, Some(CB)).events();
    assert!(
        !ev.iter()
            .any(|e| matches!(e, WFlag(..) | Stop(_) | Signal(..)))
    );
}

#[test]
fn render_reads_the_transparent_texture_first_and_hands_it_on() {
    TRANSPARENT_READS.with(|r| r.set(0));
    let st = State {
        tv: true,
        sources: [None, Some(B)],
        ..State::default()
    };
    let fake = Fake::new(st);
    render(Some(&fake), Some(CB));
    assert_eq!(TRANSPARENT_READS.with(Cell::get), 1);
    let ev = fake.events();
    assert_eq!(ev.first(), Some(&Valid(RENDER_LOG_NAME)));
    let cb = ev.iter().find_map(|e| match e {
        Callback { a, b, .. } => Some((*a, *b)),
        _ => None,
    });
    assert_eq!(cb, Some((Some(TRANSPARENT), Some(512))));

    // read even when the transition is not one, as the C's argument
    TRANSPARENT_READS.with(|r| r.set(0));
    render::<Fake>(None, Some(CB));
    assert_eq!(TRANSPARENT_READS.with(Cell::get), 1);
}

#[test]
fn render2_does_nothing_for_no_transition_or_a_source_that_is_not_one() {
    render2::<Fake>(None, Some(CB), Some(PLACEHOLDER));
    let fake = run2(
        State {
            is_transition: false,
            tv: true,
            ..State::default()
        },
        Some(CB),
    );
    assert_eq!(fake.events(), vec![Valid(RENDER_LOG_NAME)]);
}

#[test]
fn get_color_space_mixes_the_children_slot_a_first() {
    let spaces = [CS_SRGB, CS_SRGB_16F, CS_709_EXTENDED, CS_709_SCRGB, 7, -1];
    for a in spaces {
        for b in spaces {
            for sources in [
                [Some(A), Some(B)],
                [None, Some(B)],
                [Some(A), None],
                [None, None],
            ] {
                let fake = Fake::new(State {
                    sources,
                    child_space: [a, b],
                    ..State::default()
                });
                let got = get_color_space(Some(&fake));
                let mut want_space = CS_SRGB;
                let mut want = vec![
                    Valid(GET_COLOR_SPACE_LOG_NAME),
                    R(S(0), false),
                    R(S(1), false),
                ];
                if sources[0].is_some() {
                    want.push(ChildColorSpace(A, PREFERRED_SPACES.to_vec()));
                    want_space = transition_clock::mix_spaces(want_space, a);
                }
                if sources[1].is_some() {
                    want.push(ChildColorSpace(B, PREFERRED_SPACES.to_vec()));
                    want_space = transition_clock::mix_spaces(want_space, b);
                }
                assert_eq!(got, want_space, "{a} {b} {sources:?}");
                assert_eq!(fake.events(), want);
            }
        }
    }
}

#[test]
fn get_color_space_checks_its_argument() {
    // R118-1: the C reads the slots of a NULL or non-transition source.
    assert_eq!(get_color_space::<Fake>(None), CS_SRGB);
    let fake = Fake::new(State {
        is_transition: false,
        child_space: [CS_709_EXTENDED, CS_709_EXTENDED],
        ..State::default()
    });
    assert_eq!(get_color_space(Some(&fake)), CS_SRGB);
    assert_eq!(fake.events(), vec![Valid(GET_COLOR_SPACE_LOG_NAME)]);
}

#[test]
fn render_direct_draws_the_target_while_transitioning_and_slot_a_after() {
    for (tv, ta) in [(false, false), (true, false), (false, true), (true, true)] {
        for target in [TARGET_A, TARGET_B, 2, -1] {
            let st = State {
                tv,
                ta,
                ..State::default()
            };
            let transitioning = tv || ta;
            let idx = usize::from(transitioning && target == TARGET_B);
            let m = st.matrices[idx];
            let fake = Fake::new(st);
            let got = render_direct(Some(&fake), target);
            assert_eq!(got, transitioning);
            let mut want = vec![
                Valid(RENDER_DIRECT_LOG_NAME),
                Lock,
                VideoTime(true),
                R(Tv, true),
            ];
            want.extend(copy([Some(A), Some(B)]));
            want.push(Unlock);
            want.extend(draw_one([A, B][idx], &m, false));
            want.extend([Release(Some(A), false), Release(Some(B), false)]);
            assert_eq!(fake.events(), want, "{tv} {ta} {target}");
        }
    }
}

#[test]
fn render_direct_skips_an_empty_slot() {
    let fake = Fake::new(State {
        sources: [None, Some(B)],
        ..State::default()
    });
    assert!(!render_direct(Some(&fake), TARGET_B));
    let ev = fake.events();
    assert!(!ev.iter().any(|e| matches!(e, Push | Render(..))));
    assert_eq!(ev.last(), Some(&Release(Some(B), false)));
}

#[test]
fn render_direct_video_stop_for_each_flag_and_active_state() {
    for ta in [false, true] {
        for active in [false, true] {
            let st = State {
                tv: true,
                ta,
                active,
                time: 1.5,
                ..State::default()
            };
            let fake = Fake::new(st);
            let got = render_direct(Some(&fake), TARGET_B);
            let mut want = vec![
                Valid(RENDER_DIRECT_LOG_NAME),
                Lock,
                VideoTime(true),
                R(Tv, true),
                WFlag(Tv, false),
                R(Ta, true),
                Active(true),
            ];
            // inactive: the audio ends too; then a stop unless the audio
            // runs on.
            let stops = !active || !ta;
            if !active {
                want.push(WFlag(Ta, false));
            }
            if stops {
                want.push(Stop(true));
            }
            let sources = if stops {
                [Some(B), None]
            } else {
                [Some(A), Some(B)]
            };
            want.extend(copy(sources));
            want.push(Unlock);
            let still = ta && active;
            let idx = usize::from(still);
            if let Some(child) = sources[idx] {
                want.extend(draw_one(child, &fake.s.borrow().matrices[idx], false));
            }
            want.extend([
                Release(sources[0], false),
                Release(sources[1], false),
                Signal(VIDEO_STOP_SIGNALS.0, VIDEO_STOP_SIGNALS.1, false),
            ]);
            if stops {
                want.push(HandleStop(false));
            }
            assert_eq!(got, still, "{ta} {active}");
            assert_eq!(fake.events(), want, "{ta} {active}");
            assert_eq!(fake.s.borrow().ta, still);
        }
    }
}

#[test]
fn render_direct_logs_its_own_name() {
    // R118-2: the C logs "obs_transition_video_render".
    assert_eq!(
        RENDER_DIRECT_LOG_NAME.to_bytes(),
        b"obs_transition_video_render_direct"
    );
    assert!(!render_direct::<Fake>(None, TARGET_A));
    let fake = Fake::new(State {
        is_transition: false,
        tv: true,
        ..State::default()
    });
    assert!(!render_direct(Some(&fake), TARGET_B));
    assert_eq!(fake.events(), vec![Valid(RENDER_DIRECT_LOG_NAME)]);
}

#[test]
fn constants_are_the_cs() {
    assert_eq!(RENDER_LOG_NAME.to_bytes(), b"obs_transition_video_render");
    assert_eq!(
        GET_COLOR_SPACE_LOG_NAME.to_bytes(),
        b"obs_transition_video_get_color_space"
    );
    assert_eq!(
        VIDEO_STOP_SIGNALS.0.to_bytes(),
        b"source_transition_video_stop"
    );
    assert_eq!(VIDEO_STOP_SIGNALS.1.to_bytes(), b"transition_video_stop");
    assert_eq!(PREFERRED_SPACES, [CS_SRGB, CS_SRGB_16F, CS_709_EXTENDED]);
    assert_eq!((CLEAR_COLOR, ZS_NONE), (1, 0));
    assert_eq!((BLEND_ONE, BLEND_INVSRCALPHA), (1, 5));
    assert_eq!(ORTHO_Z, (-100.0, 100.0));
}

fn any_state() -> impl Strategy<Value = State> {
    let child = prop_oneof![Just(None), Just(Some(A)), Just(Some(B))];
    let tr = prop_oneof![Just(None), (1u32..4).prop_map(Some)];
    (
        (
            any::<bool>(),
            [child.clone(), child],
            any::<bool>(),
            any::<bool>(),
        ),
        (
            [tr.clone(), tr],
            any::<bool>(),
            -0.5f32..2.0,
            0u32..3,
            0u32..3,
        ),
        (any::<bool>(), 0i32..5, [0i32..5, 0i32..5], 0i32..5),
        (
            proptest::collection::btree_map(1u32..4, 99i32..106, 0..3),
            proptest::collection::vec(1u32..4, 0..3),
            any::<bool>(),
        ),
    )
        .prop_map(
            |(
                (is_transition, sources, tv, ta),
                (texrenders, tex_busy, time, cx, cy),
                (active, source_space, child_space, current_space),
                (formats, no_texture, begins),
            )| State {
                is_transition,
                sources,
                tv,
                ta,
                matrices: [matrix(1.0), matrix(2.0)],
                texrenders,
                tex_busy,
                time,
                cx,
                cy,
                active,
                source_space,
                child_space,
                current_space,
                formats,
                no_texture,
                begins,
            },
        )
}

#[derive(Debug, Clone, Copy)]
enum Call {
    Render,
    Render2(bool),
    Direct(i32),
    ColorSpace,
}

fn any_call() -> impl Strategy<Value = Call> {
    prop_oneof![
        Just(Call::Render),
        any::<bool>().prop_map(Call::Render2),
        (-1i32..3).prop_map(Call::Direct),
        Just(Call::ColorSpace),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(256)))]

    #[test]
    fn every_lock_reference_and_push_is_undone(st in any_state(), call in any_call()) {
        let due = st.tv && st.time >= 1.0;
        let is_transition = st.is_transition;
        let fake = Fake::new(st);
        let before = fake.refs();
        match call {
            Call::Render => render(Some(&fake), Some(CB)),
            Call::Render2(cb) => render2(Some(&fake), cb.then_some(CB), Some(PLACEHOLDER)),
            Call::Direct(target) => { render_direct(Some(&fake), target); }
            Call::ColorSpace => { get_color_space(Some(&fake)); }
        }
        let ev = fake.events();
        prop_assert!(!fake.locked.get() && !fake.tex_locked.get());
        prop_assert_eq!(ev.iter().filter(|e| **e == Push).count(), ev.iter().filter(|e| **e == Pop).count());
        prop_assert_eq!(ev.iter().filter(|e| **e == BlendPush).count(), ev.iter().filter(|e| **e == BlendPop).count());
        let gets = ev.iter().filter(|e| matches!(e, GetRef(_))).count();
        let releases = ev.iter().filter(|e| matches!(e, Release(..))).count();
        prop_assert_eq!(gets, releases);
        // the stop moves slot A's reference out; every other is undone.
        let mut after = fake.refs();
        if ev.iter().any(|e| matches!(e, Stop(_)))
            && let Some(stopped) = before.keys().find(|k| after[*k] != before[*k])
        {
            *after.entry(*stopped).or_default() += 1;
        }
        prop_assert_eq!(after, before);
        for e in &ev {
            match e {
                Callback { locked, tex_locked, .. } => prop_assert!(!locked && *tex_locked),
                R(..) | GetRef(_) | VideoTime(_) | Stop(_) | Active(_) => {}
                Signal(.., held) | HandleStop(held) | Release(_, held) => prop_assert!(!held),
                _ => {}
            }
        }
        if !is_transition {
            prop_assert_eq!(ev.len(), 1);
        } else if due && !matches!(call, Call::ColorSpace) {
            prop_assert!(!fake.s.borrow().tv);
            prop_assert!(ev.contains(&Signal(VIDEO_STOP_SIGNALS.0, VIDEO_STOP_SIGNALS.1, false)));
        }
    }
}

#[test]
fn format_from_space_is_the_header_inline() {
    use obs_source::transition_video::{FORMAT_RGBA, FORMAT_RGBA16F, format_from_space};
    assert_eq!((FORMAT_RGBA, FORMAT_RGBA16F), (3, 9));
    assert_eq!(format_from_space(CS_SRGB), FORMAT_RGBA);
    for space in [CS_SRGB_16F, CS_709_EXTENDED, CS_709_SCRGB] {
        assert_eq!(format_from_space(space), FORMAT_RGBA16F);
    }
    for space in [-1, 4, 99, i32::MIN, i32::MAX] {
        assert_eq!(format_from_space(space), FORMAT_RGBA);
    }
}
