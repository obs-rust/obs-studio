//! Tier 1 tests for `obs_source::transition_audio`, the safe core of the
//! rest of `libobs/obs-source-transition.c.h`: `obs_transition_init()`,
//! `obs_transition_free()` and `obs_transition_audio_render()`.
//!
//! The functions run on a fake transition: its slots, active flags, clock
//! fields, transitioning flags, alignment, texrenders, mutexes and lock,
//! whether its type has a `transition_stop` callback, a reference count per
//! child, each child's removed, pending and duplicated flags, timestamp and
//! audio block, and the caller's mix as one block. The fake records every
//! field read and write (with whether the lock was held), every mutex, lock,
//! graphics and libobs call and every child read, in order, and the time
//! each mix callback call got; it panics on a lock taken twice or released
//! unheld.
//!
//! The cases pin, against `test/cmocka/test_source_transition_audio.c`'s
//! cases: init's calls and its early returns, free's order, the argument
//! check, the copy of slot A while idle, the mix of each child with its
//! callback by mixer mask and channel on top of the mix, the skip of a
//! pending or duplicated child, the place of a child's audio (partial,
//! past the block, timestamp 0), each sample's time in manual and auto
//! mode, the audio stop for each step (with the stop's slot shift under the
//! lock and its signal after), the re-read of the children after a stop and
//! a removed child it leaves in slot A (#2185), the state copied under the
//! lock and the mix reading only that copy (#3499).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::CStr;

use obs_source::transition_audio::{
    AUDIO_RENDER_LOG_NAME, INIT_ALIGNMENT, MAX_AUDIO_MIXES, Request, Transition, audio_render,
    free, init,
};
use obs_source::transition_clock::Clock;
use obs_source::transition_sample_time::sample_time;
use obs_source::transition_stop::{self, STOP_SIGNALS};

/// A child the fake knows.
type Id = usize;

const A: Id = 1;
const B: Id = 2;

/// `OBS_TRANSITION_MODE_AUTO`, `OBS_TRANSITION_MODE_MANUAL`.
const AUTO: i32 = 0;
const MANUAL: i32 = 1;

const FRAMES: usize = 1024;
const CHANNELS: usize = 8;
const BLOCK: usize = MAX_AUDIO_MIXES * CHANNELS * FRAMES;
const RATE: usize = 48_000;
const T0: u64 = 1_000_000_000;
/// 512 frames at 48 kHz.
const HALF_BLOCK_NS: u64 = 10_666_667;
/// 1100 frames at 48 kHz.
const LATE_NS: u64 = 22_916_667;
/// The context data the fake hands its callbacks.
const DATA: usize = 77;

/// A field of the transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum F {
    Source(usize),
    Active(usize),
    Mode,
    ManualVal,
    StartTime,
    Duration,
    Video,
    Audio,
    Alignment,
    Texrender(usize),
    ContextData,
}

/// One thing the functions did, in order. A read or write carries whether
/// the lock was held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ev {
    R(F, bool),
    W(F, bool),
    Lock,
    Unlock,
    InitValues,
    MutexInit,
    TexMutexInit,
    MutexDestroy,
    TexMutexDestroy,
    Graphics,
    TexCreate,
    TexDestroy(Option<u32>),
    Enter(Option<u32>),
    Leave,
    Removed(Id),
    Pending(Id, bool),
    Duplicated(Id, bool),
    Timestamp(Id, bool),
    GetRef(Option<Id>, bool),
    AudioMix(Id),
    OutputBuf(Id),
    Copy(Id),
    CallStop,
    Signal(&'static CStr, &'static CStr),
    Release(Option<Id>, bool),
    VideoTime,
    RemoveActive(Id),
}

/// The mix callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cb {
    A,
    B,
}

/// A block of samples: the caller's mix or a child's audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Buf {
    Out,
    Child(Id),
}

#[derive(Debug, Clone)]
struct Child {
    removed: bool,
    pending: bool,
    duplicated: bool,
    ts: u64,
    refs: i64,
    block: Vec<f32>,
}

#[derive(Debug)]
struct State {
    sources: [Option<Id>; 2],
    active: [bool; 2],
    mode: i32,
    manual_val: f32,
    start_time: u64,
    duration: u64,
    video: bool,
    audio: bool,
    alignment: u32,
    texrender: [Option<u32>; 2],
    is_transition: bool,
    locked: bool,
    mutex_init: i32,
    tex_mutex_init: i32,
    /// The texrenders the next creates return.
    creates: Vec<Option<u32>>,
    children: BTreeMap<Id, Child>,
    out: Vec<f32>,
    events: Vec<Ev>,
    /// The time each call of each callback got.
    times: Vec<(Cb, f32)>,
    /// Set by a callback: the clock fields it overwrites, as another thread
    /// would.
    clobber: bool,
}

struct Tr {
    st: RefCell<State>,
}

thread_local! {
    /// Every `transition_valid()` call: the name, and whether a source was
    /// handed in.
    static VALID: RefCell<Vec<(&'static CStr, bool)>> = const { RefCell::new(Vec::new()) };
}

fn valid_calls() -> Vec<(&'static CStr, bool)> {
    VALID.with(|v| std::mem::take(&mut *v.borrow_mut()))
}

/// Child `id`'s sample for mix, channel and frame.
fn child_value(id: Id, mix: usize, ch: usize, frame: usize) -> f32 {
    // Exact in f32: at most a few bits above the 1/1024 step.
    let id = f32::from(u16::try_from(id).unwrap());
    let mix = f32::from(u16::try_from(mix).unwrap());
    let ch = f32::from(u16::try_from(ch).unwrap());
    let frame = f32::from(u16::try_from(frame).unwrap());
    id + mix * 0.5 + ch * 0.0625 + frame / 1024.0
}

fn at(mix: usize, ch: usize, frame: usize) -> usize {
    (mix * CHANNELS + ch) * FRAMES + frame
}

fn child_block(id: Id) -> Vec<f32> {
    let mut block = vec![0.0; BLOCK];
    for mix in 0..MAX_AUDIO_MIXES {
        for ch in 0..CHANNELS {
            for f in 0..FRAMES {
                block[at(mix, ch, f)] = child_value(id, mix, ch, f);
            }
        }
    }
    block
}

impl Tr {
    /// An idle auto transition holding child A in slot A, active; A and B
    /// are valid at T0, each holding one reference of its own (A's slot
    /// holds another).
    fn new() -> Self {
        let mut children = BTreeMap::new();
        for (id, refs) in [(A, 2), (B, 1)] {
            children.insert(
                id,
                Child {
                    removed: false,
                    pending: false,
                    duplicated: false,
                    ts: T0,
                    refs,
                    block: child_block(id),
                },
            );
        }
        Self {
            st: RefCell::new(State {
                sources: [Some(A), None],
                active: [true, false],
                mode: AUTO,
                manual_val: 0.0,
                start_time: 0,
                duration: 0,
                video: false,
                audio: false,
                alignment: 0,
                texrender: [None; 2],
                is_transition: true,
                locked: false,
                mutex_init: 0,
                tex_mutex_init: 0,
                creates: vec![Some(10), Some(11)],
                children,
                out: vec![0.125; BLOCK],
                events: Vec::new(),
                times: Vec::new(),
                clobber: false,
            }),
        }
    }

    /// Transitioning from A to B in manual mode at `t`.
    fn transitioning(t: f32) -> Self {
        let tr = Self::new();
        {
            let mut st = tr.st.borrow_mut();
            st.sources[1] = Some(B);
            st.children.get_mut(&B).unwrap().refs += 1;
            st.mode = MANUAL;
            st.manual_val = t;
            st.video = true;
            st.audio = true;
        }
        tr
    }

    fn child(&self, id: Id) -> std::cell::RefMut<'_, Child> {
        std::cell::RefMut::map(self.st.borrow_mut(), |st| st.children.get_mut(&id).unwrap())
    }

    fn set_child(&self, id: Id, ts: u64, pending: bool, duplicated: bool) {
        let mut c = self.child(id);
        c.ts = ts;
        c.pending = pending;
        c.duplicated = duplicated;
    }

    fn events(&self) -> Vec<Ev> {
        std::mem::take(&mut self.st.borrow_mut().events)
    }

    fn times(&self, cb: Cb) -> Vec<f32> {
        self.st
            .borrow()
            .times
            .iter()
            .filter(|(c, _)| *c == cb)
            .map(|(_, t)| *t)
            .collect()
    }

    fn out(&self) -> Vec<f32> {
        self.st.borrow().out.clone()
    }

    fn ev(&self, e: Ev) {
        self.st.borrow_mut().events.push(e);
    }

    fn read(&self, f: F) {
        let locked = self.st.borrow().locked;
        self.ev(Ev::R(f, locked));
    }

    fn write(&self, f: F) {
        let locked = self.st.borrow().locked;
        self.ev(Ev::W(f, locked));
    }

    fn locked(&self) -> bool {
        self.st.borrow().locked
    }

    fn render(&self, mixers: u32, channels: usize) -> Option<u64> {
        audio_render(
            Some(self),
            Request {
                audio: (),
                mixers,
                channels,
                sample_rate: RATE,
                mix_a: Cb::A,
                mix_b: Cb::B,
            },
        )
    }
}

impl transition_stop::Transition for Tr {
    type Child = Id;

    fn valid(source: Option<&Self>, func: &'static CStr) -> bool {
        VALID.with(|v| v.borrow_mut().push((func, source.is_some())));
        source.is_some_and(|s| s.st.borrow().is_transition)
    }
    fn source(&self, idx: usize) -> Option<Id> {
        self.read(F::Source(idx));
        self.st.borrow().sources[idx]
    }
    fn set_source(&self, idx: usize, value: Option<Id>) {
        self.write(F::Source(idx));
        self.st.borrow_mut().sources[idx] = value;
    }
    fn source_active(&self, idx: usize) -> bool {
        self.read(F::Active(idx));
        self.st.borrow().active[idx]
    }
    fn set_source_active(&self, idx: usize, value: bool) {
        self.write(F::Active(idx));
        self.st.borrow_mut().active[idx] = value;
    }
    fn mode(&self) -> i32 {
        self.read(F::Mode);
        self.st.borrow().mode
    }
    fn manual_val(&self) -> f32 {
        self.read(F::ManualVal);
        self.st.borrow().manual_val
    }
    fn start_time(&self) -> u64 {
        self.read(F::StartTime);
        self.st.borrow().start_time
    }
    fn duration(&self) -> u64 {
        self.read(F::Duration);
        self.st.borrow().duration
    }
    fn lock(&self) {
        let mut st = self.st.borrow_mut();
        assert!(!st.locked, "transition_mutex taken twice");
        st.locked = true;
        st.events.push(Ev::Lock);
    }
    fn unlock(&self) {
        let mut st = self.st.borrow_mut();
        assert!(st.locked, "transition_mutex released unheld");
        st.locked = false;
        st.events.push(Ev::Unlock);
    }
    fn video_time(&self) -> u64 {
        self.ev(Ev::VideoTime);
        0
    }
    fn call_stop(&self) {
        self.ev(Ev::CallStop);
    }
    fn dosignal(&self, signal_obs: &'static CStr, signal_source: &'static CStr) {
        self.ev(Ev::Signal(signal_obs, signal_source));
    }
    fn remove_active_child(&self, child: Id) {
        self.ev(Ev::RemoveActive(child));
    }
    fn release(&self, child: Option<Id>) {
        let locked = self.locked();
        self.ev(Ev::Release(child, locked));
        if let Some(id) = child {
            self.child(id).refs -= 1;
        }
    }
}

impl Transition for Tr {
    type Texrender = u32;
    type Graphics = u32;
    type Data = usize;
    type MixCallback = Cb;
    type Mix = ();
    type ChildAudio = Id;
    type Samples = (Buf, usize);

    fn init_mutex_values(&self) {
        self.ev(Ev::InitValues);
    }
    fn mutex_init(&self) -> i32 {
        self.ev(Ev::MutexInit);
        self.st.borrow().mutex_init
    }
    fn tex_mutex_init(&self) -> i32 {
        self.ev(Ev::TexMutexInit);
        self.st.borrow().tex_mutex_init
    }
    fn mutex_destroy(&self) {
        self.ev(Ev::MutexDestroy);
    }
    fn tex_mutex_destroy(&self) {
        self.ev(Ev::TexMutexDestroy);
    }
    fn set_alignment(&self, value: u32) {
        self.write(F::Alignment);
        self.st.borrow_mut().alignment = value;
    }
    fn texrender(&self, idx: usize) -> Option<u32> {
        self.read(F::Texrender(idx));
        self.st.borrow().texrender[idx]
    }
    fn set_texrender(&self, idx: usize, value: Option<u32>) {
        self.write(F::Texrender(idx));
        self.st.borrow_mut().texrender[idx] = value;
    }
    fn transitioning_video(&self) -> bool {
        self.read(F::Video);
        self.st.borrow().video
    }
    fn transitioning_audio(&self) -> bool {
        self.read(F::Audio);
        self.st.borrow().audio
    }
    fn set_transitioning_audio(&self, value: bool) {
        self.write(F::Audio);
        self.st.borrow_mut().audio = value;
    }
    fn context_data(&self) -> usize {
        self.read(F::ContextData);
        DATA
    }
    fn graphics(&self) -> Option<u32> {
        self.ev(Ev::Graphics);
        Some(99)
    }
    fn texrender_create(&self) -> Option<u32> {
        self.ev(Ev::TexCreate);
        let mut st = self.st.borrow_mut();
        if st.creates.is_empty() {
            None
        } else {
            st.creates.remove(0)
        }
    }
    fn texrender_destroy(&self, texrender: Option<u32>) {
        self.ev(Ev::TexDestroy(texrender));
    }
    fn enter_context(&self, graphics: Option<u32>) {
        self.ev(Ev::Enter(graphics));
    }
    fn leave_context(&self) {
        self.ev(Ev::Leave);
    }
    fn removed(&self, child: Id) -> bool {
        self.ev(Ev::Removed(child));
        self.child(child).removed
    }
    fn child_audio_pending(&self, child: Id) -> bool {
        let locked = self.locked();
        self.ev(Ev::Pending(child, locked));
        self.child(child).pending
    }
    fn child_audio_is_duplicated(&self, child: Id) -> bool {
        let locked = self.locked();
        self.ev(Ev::Duplicated(child, locked));
        self.child(child).duplicated
    }
    fn audio_timestamp(&self, child: Id) -> u64 {
        let locked = self.locked();
        self.ev(Ev::Timestamp(child, locked));
        self.child(child).ts
    }
    fn get_ref(&self, child: Option<Id>) -> Option<Id> {
        let locked = self.locked();
        self.ev(Ev::GetRef(child, locked));
        let id = child?;
        let mut c = self.child(id);
        if c.refs > 0 {
            c.refs += 1;
            Some(id)
        } else {
            None
        }
    }
    fn audio_mix(&self, child: Id) -> Id {
        self.ev(Ev::AudioMix(child));
        child
    }
    fn child_audio_output_buf(&self, child: Id) -> (Buf, usize) {
        self.ev(Ev::OutputBuf(child));
        (Buf::Child(child), 0)
    }
    fn call_mix(&self, mix: Cb, data: usize, t: f32) -> f32 {
        assert_eq!(data, DATA);
        assert!(
            !self.locked(),
            "a mix callback runs with transition_mutex held"
        );
        let mut st = self.st.borrow_mut();
        st.times.push((mix, t));
        if st.clobber {
            st.mode = AUTO;
            st.manual_val = 0.75;
            st.start_time = 5;
            st.duration = 7;
            st.audio = false;
        }
        match mix {
            Cb::A => 0.25,
            Cb::B => 0.5,
        }
    }
    fn output_samples(&self, (): (), mix_idx: usize, ch: usize) -> (Buf, usize) {
        (Buf::Out, at(mix_idx, ch, 0))
    }
    fn input_samples(&self, audio: &Id, mix_idx: usize, ch: usize) -> (Buf, usize) {
        (Buf::Child(*audio), at(mix_idx, ch, 0))
    }
    fn sample(&self, (buf, start): (Buf, usize), frame: usize) -> f32 {
        let st = self.st.borrow();
        match buf {
            Buf::Out => st.out[start + frame],
            Buf::Child(id) => st.children[&id].block[start + frame],
        }
    }
    fn set_sample(&self, (buf, start): (Buf, usize), frame: usize, value: f32) {
        assert_eq!(buf, Buf::Out, "the render writes only the caller's mix");
        self.st.borrow_mut().out[start + frame] = value;
    }
    fn copy_audio(&self, (): (), (buf, start): (Buf, usize)) {
        let Buf::Child(id) = buf else {
            panic!("the copy is from a child");
        };
        assert_eq!(start, 0);
        self.ev(Ev::Copy(id));
        let mut st = self.st.borrow_mut();
        let block = st.children[&id].block.clone();
        st.out.copy_from_slice(&block);
    }
}

/// The caller's mix with child `id`'s audio times `mul` added for the
/// selected mixes and channels, from frame `pos`.
fn mixed(
    mut out: Vec<f32>,
    id: Id,
    mul: f32,
    mixers: u32,
    channels: usize,
    pos: usize,
) -> Vec<f32> {
    for mix in 0..MAX_AUDIO_MIXES {
        if mixers & (1 << mix) == 0 {
            continue;
        }
        for ch in 0..channels {
            for f in pos..FRAMES {
                out[at(mix, ch, f)] += child_value(id, mix, ch, f - pos) * mul;
            }
        }
    }
    out
}

fn check_out(tr: &Tr, expected: &[f32]) {
    let out = tr.out();
    for (i, (got, want)) in out.iter().zip(expected).enumerate() {
        assert!(
            got.to_bits() == want.to_bits(),
            "sample {i} (mix {}, channel {}, frame {}): {got}, expected {want}",
            i / (CHANNELS * FRAMES),
            (i / FRAMES) % CHANNELS,
            i % FRAMES
        );
    }
}

/// The reads of `transition_live_sources()` and `transition_calc_min_ts()`
/// for the slots' children `(id, removed, pending)`, all under the lock.
fn live_and_min_ts(slots: [Option<(Id, bool, bool)>; 2]) -> Vec<Ev> {
    let mut evs = Vec::new();
    for (idx, slot) in slots.iter().enumerate() {
        evs.push(Ev::R(F::Source(idx), true));
        if let Some((id, _, _)) = slot {
            evs.push(Ev::Removed(*id));
        }
    }
    for slot in slots.iter().flatten() {
        let (id, removed, pending) = *slot;
        if removed {
            continue;
        }
        evs.push(Ev::Pending(id, true));
        if !pending {
            evs.push(Ev::Timestamp(id, true));
        }
    }
    evs
}

/// The reads of the clock by `transition_calc_time()`, under the lock.
fn calc_time_reads() -> Vec<Ev> {
    vec![
        Ev::R(F::Mode, true),
        Ev::R(F::ManualVal, true),
        Ev::R(F::StartTime, true),
        Ev::R(F::Duration, true),
    ]
}

/// The state copy: the references, flags and clock, under the lock.
fn state_copy(a: Option<Id>, b: Option<Id>) -> Vec<Ev> {
    vec![
        Ev::GetRef(a, true),
        Ev::GetRef(b, true),
        Ev::R(F::Video, true),
        Ev::R(F::Audio, true),
        Ev::R(F::Mode, true),
        Ev::R(F::ManualVal, true),
        Ev::R(F::StartTime, true),
        Ev::R(F::Duration, true),
    ]
}

#[test]
fn init_makes_mutexes_alignment_and_texrenders() {
    let tr = Tr::new();
    assert!(init(&tr));
    assert_eq!(
        tr.events(),
        vec![
            Ev::InitValues,
            Ev::MutexInit,
            Ev::TexMutexInit,
            Ev::W(F::Alignment, false),
            Ev::TexCreate,
            Ev::W(F::Texrender(0), false),
            Ev::TexCreate,
            Ev::W(F::Texrender(1), false),
            Ev::W(F::Active(0), false),
            Ev::R(F::Texrender(0), false),
            Ev::R(F::Texrender(1), false),
        ]
    );
    let st = tr.st.borrow();
    assert_eq!(st.alignment, INIT_ALIGNMENT);
    assert_eq!(INIT_ALIGNMENT, 5);
    assert_eq!(st.texrender, [Some(10), Some(11)]);
    assert!(st.active[0]);
}

#[test]
fn init_returns_on_a_failed_mutex_before_the_texrenders() {
    let tr = Tr::new();
    tr.st.borrow_mut().mutex_init = 22;
    assert!(!init(&tr));
    assert_eq!(tr.events(), vec![Ev::InitValues, Ev::MutexInit]);

    let tr = Tr::new();
    tr.st.borrow_mut().tex_mutex_init = 12;
    assert!(!init(&tr));
    assert_eq!(
        tr.events(),
        vec![Ev::InitValues, Ev::MutexInit, Ev::TexMutexInit]
    );
    assert_eq!(tr.st.borrow().alignment, 0);
}

#[test]
fn init_fails_without_a_texrender() {
    let tr = Tr::new();
    tr.st.borrow_mut().creates = vec![None, Some(11)];
    assert!(!init(&tr));
    let evs = tr.events();
    // Slot B's texrender is not read once slot A's is missing.
    assert_eq!(evs.last(), Some(&Ev::R(F::Texrender(0), false)));
    assert_eq!(tr.st.borrow().texrender, [None, Some(11)]);

    let tr = Tr::new();
    tr.st.borrow_mut().creates = vec![Some(10), None];
    assert!(!init(&tr));
    assert_eq!(tr.events().last(), Some(&Ev::R(F::Texrender(1), false)));
}

#[test]
fn free_destroys_the_mutexes_then_the_texrenders_in_the_context() {
    let tr = Tr::new();
    tr.st.borrow_mut().texrender = [Some(10), None];
    free(&tr);
    assert_eq!(
        tr.events(),
        vec![
            Ev::MutexDestroy,
            Ev::TexMutexDestroy,
            Ev::Graphics,
            Ev::Enter(Some(99)),
            Ev::R(F::Texrender(0), false),
            Ev::TexDestroy(Some(10)),
            Ev::R(F::Texrender(1), false),
            Ev::TexDestroy(None),
            Ev::Leave,
        ]
    );
}

#[test]
fn audio_render_checks_its_argument() {
    valid_calls();
    let request = Request::<Tr> {
        audio: (),
        mixers: 0x3F,
        channels: 2,
        sample_rate: RATE,
        mix_a: Cb::A,
        mix_b: Cb::B,
    };
    assert_eq!(audio_render::<Tr>(None, request), None);
    assert_eq!(valid_calls(), vec![(AUDIO_RENDER_LOG_NAME, false)]);

    let tr = Tr::new();
    tr.st.borrow_mut().is_transition = false;
    let before = tr.out();
    assert_eq!(audio_render(Some(&tr), request), None);
    assert_eq!(valid_calls(), vec![(AUDIO_RENDER_LOG_NAME, true)]);
    assert!(tr.events().is_empty());
    check_out(&tr, &before);
}

/// `idle_copies_a`: idle, slot A's whole block is copied after the lock,
/// whatever the mixers and channels, and no callback runs.
#[test]
fn idle_copies_a() {
    let tr = Tr::new();
    assert_eq!(tr.render(0x1, 1), Some(T0));
    let mut want = vec![Ev::Lock];
    want.extend(live_and_min_ts([Some((A, false, false)), None]));
    want.extend(calc_time_reads());
    want.extend([Ev::R(F::Video, true), Ev::R(F::Audio, true)]);
    want.extend(live_and_min_ts([Some((A, false, false)), None]));
    want.extend(state_copy(Some(A), None));
    want.extend([
        Ev::Unlock,
        Ev::OutputBuf(A),
        Ev::Copy(A),
        Ev::Release(Some(A), false),
        Ev::Release(None, false),
    ]);
    assert_eq!(tr.events(), want);
    check_out(&tr, &child_block(A));
    assert!(tr.st.borrow().times.is_empty());
    assert_eq!(tr.child(A).refs, 2);
}

/// `no_timestamp_writes_nothing` and `zero_timestamp_is_none`: no child, a
/// pending child, a removed child or a timestamp of 0: `Some(0)`, nothing
/// written, no reference taken, and the clock not read.
#[test]
fn no_timestamp_writes_nothing() {
    for setup in 0..4 {
        let tr = Tr::new();
        match setup {
            0 => tr.set_child(A, T0, true, false),
            1 => tr.child(A).removed = true,
            2 => tr.set_child(A, 0, false, false),
            _ => tr.st.borrow_mut().sources[0] = None,
        }
        let before = tr.out();
        assert_eq!(tr.render(0x3F, 2), Some(0), "setup {setup}");
        check_out(&tr, &before);
        let evs = tr.events();
        assert!(!evs.contains(&Ev::R(F::Mode, true)), "setup {setup}");
        assert!(!evs.iter().any(|e| matches!(e, Ev::GetRef(..))));
        assert_eq!(
            &evs[evs.len() - 3..],
            &[Ev::R(F::Video, true), Ev::R(F::Audio, true), Ev::Unlock]
        );
        assert!(tr.st.borrow().times.is_empty());
    }
}

/// `transitioning_mixes_both`: each child with its callback, into the
/// selected mixes and asked channels, on top of the mix, every sample at
/// the manual value; A before B.
#[test]
fn transitioning_mixes_both() {
    let tr = Tr::transitioning(0.5);
    let before = tr.out();
    assert_eq!(tr.render(0x5, 2), Some(T0));
    let want = mixed(mixed(before, A, 0.25, 0x5, 2, 0), B, 0.5, 0x5, 2, 0);
    check_out(&tr, &want);
    let times = tr.st.borrow().times.clone();
    assert_eq!(times.len(), 2 * 2 * 2 * FRAMES);
    assert!(
        times[..2 * 2 * FRAMES]
            .iter()
            .all(|&(c, t)| c == Cb::A && t == 0.5)
    );
    assert!(
        times[2 * 2 * FRAMES..]
            .iter()
            .all(|&(c, t)| c == Cb::B && t == 0.5)
    );

    let evs = tr.events();
    let unlock = evs.iter().position(|e| *e == Ev::Unlock).unwrap();
    assert_eq!(
        &evs[unlock..],
        &[
            Ev::Unlock,
            Ev::Pending(A, false),
            Ev::Duplicated(A, false),
            Ev::Timestamp(A, false),
            Ev::AudioMix(A),
            // The context data is read once per mix and channel.
            Ev::R(F::ContextData, false),
            Ev::R(F::ContextData, false),
            Ev::R(F::ContextData, false),
            Ev::R(F::ContextData, false),
            Ev::Pending(B, false),
            Ev::Duplicated(B, false),
            Ev::Timestamp(B, false),
            Ev::AudioMix(B),
            Ev::R(F::ContextData, false),
            Ev::R(F::ContextData, false),
            Ev::R(F::ContextData, false),
            Ev::R(F::ContextData, false),
            Ev::Release(Some(A), false),
            Ev::Release(Some(B), false),
        ]
    );
    assert_eq!(tr.child(A).refs, 2);
    assert_eq!(tr.child(B).refs, 2);
}

/// `no_mixers_mix_nothing`.
#[test]
fn no_mixers_mix_nothing() {
    let tr = Tr::transitioning(0.5);
    let before = tr.out();
    assert_eq!(tr.render(0, 8), Some(T0));
    check_out(&tr, &before);
    assert!(tr.st.borrow().times.is_empty());
    // Both children are still read.
    let evs = tr.events();
    assert!(evs.contains(&Ev::AudioMix(A)));
    assert!(evs.contains(&Ev::AudioMix(B)));
}

/// Every mixer and channel.
#[test]
fn every_mix_and_channel() {
    let tr = Tr::transitioning(0.5);
    let before = tr.out();
    assert_eq!(tr.render(0x3F, 8), Some(T0));
    let want = mixed(mixed(before, A, 0.25, 0x3F, 8, 0), B, 0.5, 0x3F, 8, 0);
    check_out(&tr, &want);
    // Mixer bits past the mixes select nothing more.
    let tr = Tr::transitioning(0.5);
    let before = tr.out();
    assert_eq!(tr.render(0xFFFF_FFC2, 1), Some(T0));
    check_out(
        &tr,
        &mixed(mixed(before, A, 0.25, 0x2, 1, 0), B, 0.5, 0x2, 1, 0),
    );
}

/// `pending_and_duplicated_are_skipped`: a pending child is skipped and
/// gives no timestamp; a duplicated one is skipped, its timestamp counts.
#[test]
fn pending_and_duplicated_are_skipped() {
    let tr = Tr::transitioning(0.5);
    tr.set_child(A, T0 - 1000, true, false);
    let before = tr.out();
    assert_eq!(tr.render(0x1, 1), Some(T0));
    check_out(&tr, &mixed(before, B, 0.5, 0x1, 1, 0));
    assert!(tr.times(Cb::A).is_empty());
    assert_eq!(tr.times(Cb::B).len(), FRAMES);
    let evs = tr.events();
    // Both flags are read (they are the valid rule's arguments), then
    // nothing more of a pending child.
    assert!(evs.contains(&Ev::Pending(A, false)));
    assert!(evs.contains(&Ev::Duplicated(A, false)));
    assert!(!evs.contains(&Ev::Timestamp(A, false)));
    assert!(!evs.contains(&Ev::AudioMix(A)));

    let tr = Tr::transitioning(0.5);
    tr.set_child(B, T0 - HALF_BLOCK_NS, false, true);
    let before = tr.out();
    assert_eq!(tr.render(0x1, 1), Some(T0 - HALF_BLOCK_NS));
    check_out(&tr, &mixed(before, A, 0.25, 0x1, 1, 512));
    assert_eq!(tr.times(Cb::A).len(), FRAMES - 512);
    assert!(tr.times(Cb::B).is_empty());
    let evs = tr.events();
    assert!(evs.contains(&Ev::Duplicated(B, false)));
    assert!(!evs.contains(&Ev::Timestamp(B, false)));
    assert!(!evs.contains(&Ev::AudioMix(B)));
}

/// `place_follows_the_timestamp`: from the child's frame past the
/// earliest; nothing (but the mix read) past the block; nothing (and no
/// mix read) at timestamp 0.
#[test]
fn place_follows_the_timestamp() {
    let tr = Tr::transitioning(0.5);
    tr.set_child(B, T0 + HALF_BLOCK_NS, false, false);
    let before = tr.out();
    assert_eq!(tr.render(0x1, 1), Some(T0));
    check_out(
        &tr,
        &mixed(mixed(before, A, 0.25, 0x1, 1, 0), B, 0.5, 0x1, 1, 512),
    );
    assert_eq!(tr.times(Cb::B).len(), FRAMES - 512);

    let tr = Tr::transitioning(0.5);
    tr.set_child(B, T0 + LATE_NS, false, false);
    let before = tr.out();
    assert_eq!(tr.render(0x1, 1), Some(T0));
    check_out(&tr, &mixed(before, A, 0.25, 0x1, 1, 0));
    assert!(tr.times(Cb::B).is_empty());
    let evs = tr.events();
    assert!(evs.contains(&Ev::AudioMix(B)));

    let tr = Tr::transitioning(0.5);
    tr.set_child(B, 0, false, false);
    let before = tr.out();
    assert_eq!(tr.render(0x1, 1), Some(T0));
    check_out(&tr, &mixed(before, A, 0.25, 0x1, 1, 0));
    assert!(tr.times(Cb::B).is_empty());
    let evs = tr.events();
    assert!(evs.contains(&Ev::Timestamp(B, false)));
    assert!(!evs.contains(&Ev::AudioMix(B)));
}

/// `auto_mode_times_each_sample`: each sample gets the clock at its
/// child's timestamp plus its offset.
#[test]
fn auto_mode_times_each_sample() {
    let tr = Tr::transitioning(0.0);
    let duration = 4 * HALF_BLOCK_NS;
    {
        let mut st = tr.st.borrow_mut();
        st.mode = AUTO;
        st.start_time = T0;
        st.duration = duration;
    }
    tr.set_child(B, T0 + HALF_BLOCK_NS, false, false);
    assert_eq!(tr.render(0x1, 1), Some(T0));
    let clock = Clock {
        mode: AUTO,
        manual_val: 0.0,
        start_time: T0,
        duration,
    };
    let a = tr.times(Cb::A);
    assert_eq!(a.len(), FRAMES);
    for (i, t) in a.iter().enumerate() {
        assert_eq!(t.to_bits(), sample_time(clock, T0, RATE, i).to_bits());
    }
    let b = tr.times(Cb::B);
    assert_eq!(b.len(), FRAMES - 512);
    for (i, t) in b.iter().enumerate() {
        assert_eq!(
            t.to_bits(),
            sample_time(clock, T0 + HALF_BLOCK_NS, RATE, i).to_bits()
        );
    }
}

/// `audio_end_stops`: at t >= 1 with the video ended, the audio ends and
/// the transition stops under the lock (slot B into slot A); the children
/// are read again, the new slot A copied, and the stop's callback and
/// signals come last.
#[test]
fn audio_end_stops() {
    let tr = Tr::transitioning(1.0);
    tr.st.borrow_mut().video = false;
    tr.set_child(B, T0 + 1000, false, false);
    assert_eq!(tr.render(0x1, 1), Some(T0 + 1000));

    let mut want = vec![Ev::Lock];
    want.extend(live_and_min_ts([
        Some((A, false, false)),
        Some((B, false, false)),
    ]));
    want.extend(calc_time_reads());
    want.extend([
        Ev::R(F::Video, true),
        Ev::R(F::Audio, true),
        Ev::W(F::Audio, true),
        // transition_stop()
        Ev::R(F::Source(0), true),
        Ev::R(F::Active(0), true),
        Ev::RemoveActive(A),
        Ev::Release(Some(A), true),
        Ev::W(F::Active(0), true),
        Ev::W(F::Active(1), true),
        Ev::R(F::Source(1), true),
        Ev::W(F::Source(0), true),
        Ev::W(F::Source(1), true),
    ]);
    want.extend(live_and_min_ts([Some((B, false, false)), None]));
    want.extend(state_copy(Some(B), None));
    want.extend([
        Ev::Unlock,
        Ev::OutputBuf(B),
        Ev::Copy(B),
        Ev::Release(Some(B), false),
        Ev::Release(None, false),
        Ev::CallStop,
        Ev::Signal(STOP_SIGNALS.0, STOP_SIGNALS.1),
    ]);
    assert_eq!(tr.events(), want);
    check_out(&tr, &child_block(B));
    let st = tr.st.borrow();
    assert!(!st.audio);
    assert_eq!(st.sources, [Some(B), None]);
    assert_eq!(st.active, [true, false]);
    assert!(st.times.is_empty());
    assert_eq!(st.children[&A].refs, 1);
    assert_eq!(st.children[&B].refs, 2);
}

/// `audio_end_with_video_running`: only the audio ends, nothing stops, and
/// slot A is copied; below 1 nothing ends.
#[test]
fn audio_end_with_video_running() {
    let tr = Tr::transitioning(1.0);
    assert_eq!(tr.render(0x1, 1), Some(T0));
    let evs = tr.events();
    assert!(evs.contains(&Ev::W(F::Audio, true)));
    assert!(!evs.contains(&Ev::CallStop));
    assert!(evs.contains(&Ev::Copy(A)));
    check_out(&tr, &child_block(A));
    {
        let st = tr.st.borrow();
        assert!(!st.audio);
        assert!(st.video);
        assert_eq!(st.sources, [Some(A), Some(B)]);
    }

    let tr = Tr::transitioning(0.99);
    let before = tr.out();
    assert_eq!(tr.render(0x1, 1), Some(T0));
    assert!(!tr.events().iter().any(|e| matches!(e, Ev::W(..))));
    assert!(tr.st.borrow().audio);
    check_out(
        &tr,
        &mixed(mixed(before, A, 0.25, 0x1, 1, 0), B, 0.5, 0x1, 1, 0),
    );
}

/// `no_timestamp_stop`: with no timestamp the clock is not read; the video
/// running keeps it, the video ended stops it, with no second read of the
/// children.
#[test]
fn no_timestamp_stop() {
    let tr = Tr::transitioning(0.5);
    tr.set_child(A, T0, true, false);
    tr.set_child(B, T0, true, false);
    assert_eq!(tr.render(0x1, 1), Some(0));
    let evs = tr.events();
    assert!(!evs.contains(&Ev::W(F::Audio, true)));
    assert!(tr.st.borrow().audio);

    tr.st.borrow_mut().video = false;
    let before = tr.out();
    assert_eq!(tr.render(0x1, 1), Some(0));
    let mut want = vec![Ev::Lock];
    want.extend(live_and_min_ts([
        Some((A, false, true)),
        Some((B, false, true)),
    ]));
    want.extend([
        Ev::R(F::Video, true),
        Ev::R(F::Audio, true),
        Ev::W(F::Audio, true),
        Ev::R(F::Source(0), true),
        Ev::R(F::Active(0), true),
        Ev::RemoveActive(A),
        Ev::Release(Some(A), true),
        Ev::W(F::Active(0), true),
        Ev::W(F::Active(1), true),
        Ev::R(F::Source(1), true),
        Ev::W(F::Source(0), true),
        Ev::W(F::Source(1), true),
        Ev::Unlock,
        Ev::CallStop,
        Ev::Signal(STOP_SIGNALS.0, STOP_SIGNALS.1),
    ]);
    assert_eq!(tr.events(), want);
    check_out(&tr, &before);
    assert_eq!(tr.st.borrow().sources, [Some(B), None]);
}

/// `removed_child_after_stop`: a removed child B is left out; once the
/// stop makes it slot A, the second read finds no timestamp (#2185).
#[test]
fn removed_child_after_stop() {
    let tr = Tr::transitioning(0.5);
    tr.child(B).removed = true;
    let before = tr.out();
    assert_eq!(tr.render(0x1, 1), Some(T0));
    check_out(&tr, &mixed(before, A, 0.25, 0x1, 1, 0));
    assert!(tr.times(Cb::B).is_empty());
    let evs = tr.events();
    assert!(evs.contains(&Ev::GetRef(None, true)));

    {
        let mut st = tr.st.borrow_mut();
        st.manual_val = 1.0;
        st.video = false;
    }
    let before = tr.out();
    assert_eq!(tr.render(0x1, 1), Some(0));
    check_out(&tr, &before);
    let evs = tr.events();
    let stop = evs
        .iter()
        .position(|e| *e == Ev::W(F::Source(1), true))
        .unwrap();
    let mut after = live_and_min_ts([Some((B, true, false)), None]);
    after.extend([
        Ev::Unlock,
        Ev::CallStop,
        Ev::Signal(STOP_SIGNALS.0, STOP_SIGNALS.1),
    ]);
    assert_eq!(&evs[stop + 1..], &after[..]);
    assert_eq!(tr.st.borrow().sources, [Some(B), None]);
}

/// The mix reads only the clock copied under the lock (#3499): a callback
/// that changes the clock and flags (as another thread would after the
/// unlock) changes no sample's time.
#[test]
fn mix_reads_only_the_copied_clock() {
    let tr = Tr::transitioning(0.5);
    tr.st.borrow_mut().clobber = true;
    let before = tr.out();
    assert_eq!(tr.render(0x1, 2), Some(T0));
    assert!(tr.times(Cb::A).iter().all(|&t| t == 0.5));
    assert!(tr.times(Cb::B).iter().all(|&t| t == 0.5));
    // B is still mixed though the flag is now false.
    check_out(
        &tr,
        &mixed(mixed(before, A, 0.25, 0x1, 2, 0), B, 0.5, 0x1, 2, 0),
    );
    // No field is read after the lock but the context data.
    let evs = tr.events();
    let unlock = evs.iter().position(|e| *e == Ev::Unlock).unwrap();
    assert!(
        evs[unlock..]
            .iter()
            .all(|e| !matches!(e, Ev::R(f, _) if *f != F::ContextData))
    );
}

/// A child whose reference can't be taken (being destroyed) is left out of
/// the copied state.
#[test]
fn a_child_being_destroyed_is_left_out() {
    let tr = Tr::transitioning(0.5);
    tr.child(B).refs = 0;
    let before = tr.out();
    assert_eq!(tr.render(0x1, 1), Some(T0));
    check_out(&tr, &mixed(before, A, 0.25, 0x1, 1, 0));
    assert!(tr.times(Cb::B).is_empty());
    let evs = tr.events();
    assert!(evs.contains(&Ev::Release(None, false)));
}
