//! Tier 1 tests for `obs_source::transition_switch`, the safe core of
//! `libobs/obs-source-transition-switch.c.h`.
//!
//! The functions run on a fake transition: its slots, flags, mode, clock,
//! fixed duration, size and lock, and a small world around it (a reference
//! count per child, its active-child links, the trees the recursion walk
//! visits, children whose add fails or whose last reference is gone, a
//! clock, and a plugin `transition_start` that can re-enter once). The fake
//! records every field read and write (with whether the lock was held),
//! every lock call and every call into the rest of libobs, in order, and
//! panics on a lock taken twice or released unheld.
//!
//! The `c_*` cases follow `test_source_transition_switch.c` (same names and
//! the same steps), with what the real libobs shows (the slots, the running
//! state, the callback and signal counts, the active children and the log)
//! read off the fake. The other cases pin the C's order of reads, writes,
//! locks and calls for a whole start and set, each step of `set_source()`
//! and `activate_transition()`, the clock, the refusals and their warnings,
//! the admission that a start on another thread makes `Done`, and (a
//! proptest) that references and active-child links follow the slots over
//! any sequence of starts and sets.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CStr;

use obs_c_oracle::parity::cases;
use obs_source::transition_switch::{
    SET_LOG_NAME, SET_RECURSION_WARNING, SOURCE_A, SOURCE_B, START_LOG_NAME,
    START_RECURSION_WARNING, START_SIGNALS, STOP_SIGNALS, StartClock, Transition,
    activate_transition, recursive, set, set_source, start, write_start_clock,
};
use proptest::prelude::*;

/// A source the fake knows: [`TR`] is the transition itself, the others are
/// children.
type Id = usize;

const TR: Id = 0;
const A: Id = 1;
const B: Id = 2;
const C: Id = 3;
/// A scene whose tree holds the transition.
const SCENE: Id = 4;

/// `OBS_TRANSITION_MODE_AUTO`, `OBS_TRANSITION_MODE_MANUAL`.
const AUTO: i32 = 0;
const MANUAL: i32 = 1;

/// A field of the transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum F {
    Source(usize),
    Active(usize),
    Video,
    Audio,
    Mode,
    ManualVal,
    ManualTarget,
    StartTime,
    Duration,
    UseFixed,
    Fixed,
    Cx,
    Cy,
}

/// One thing the functions did, in order. A read or write carries whether
/// the lock was held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ev {
    Valid(&'static CStr),
    /// `transition_active()`, and whether the lock was held.
    IsActive(bool),
    R(F, bool),
    W(F, bool),
    Lock,
    Unlock,
    CallStart,
    GetRef(Option<Id>),
    Release(Option<Id>),
    Add(Option<Id>, bool),
    Remove(Id),
    Walk(Id),
    Visit(Id, Id),
    Now,
    Signal(&'static CStr, &'static CStr),
    RecalcSize,
    RecalcMatrices,
    Warn(&'static CStr, Option<Id>),
}

/// What the plugin's `transition_start` does the first time it is called.
#[derive(Debug, Clone, Copy)]
enum Reenter {
    Start(i32, u32, Option<Id>),
    Set(Option<Id>),
}

#[derive(Debug, Clone, PartialEq)]
struct Fields {
    sources: [Option<Id>; 2],
    active: [bool; 2],
    video: bool,
    audio: bool,
    mode: i32,
    manual_val: f32,
    manual_target: f32,
    start_time: u64,
    duration: u64,
    use_fixed: bool,
    fixed: u32,
    cx: u32,
    cy: u32,
}

/// Something another thread does to the fields between two lock calls.
type Between = fn(&mut Fields);

#[derive(Debug)]
struct State {
    f: Fields,
    is_transition: bool,
    locked: bool,
    /// Lock calls so far, and something another thread does just before
    /// the lock call of that number (1 for the first) takes the lock.
    locks: usize,
    between: Option<(usize, Between)>,
    events: Vec<Ev>,
    /// References held on each child.
    refs: BTreeMap<Id, i64>,
    /// The transition's active-child links to each child.
    links: BTreeMap<Id, i64>,
    /// The parent and child pairs the walk of each root visits.
    trees: BTreeMap<Id, Vec<(Id, Id)>>,
    /// Children whose add as an active child fails.
    refuses: BTreeSet<Id>,
    /// Children whose last reference is gone.
    dying: BTreeSet<Id>,
    clock: u64,
    reenter: Option<Reenter>,
    starts: usize,
}

struct Tr {
    st: RefCell<State>,
}

impl Tr {
    /// An idle transition with nothing in its slots, no size and no fixed
    /// duration; every child holds one reference of its own.
    fn new() -> Self {
        let mut refs = BTreeMap::new();
        for id in [A, B, C, SCENE] {
            refs.insert(id, 1);
        }
        let mut trees = BTreeMap::new();
        trees.insert(SCENE, vec![(SCENE, A), (SCENE, TR)]);
        Self {
            st: RefCell::new(State {
                f: Fields {
                    sources: [None; 2],
                    active: [false; 2],
                    video: false,
                    audio: false,
                    mode: AUTO,
                    manual_val: 0.5,
                    manual_target: 0.75,
                    start_time: 7,
                    duration: 9,
                    use_fixed: false,
                    fixed: 0,
                    cx: 0,
                    cy: 0,
                },
                is_transition: true,
                locked: false,
                locks: 0,
                between: None,
                events: Vec::new(),
                refs,
                links: BTreeMap::new(),
                trees,
                refuses: BTreeSet::new(),
                dying: BTreeSet::new(),
                clock: 1_000,
                reenter: None,
                starts: 0,
            }),
        }
    }

    /// As `make_transition()`: `child` set in slot A, the events emptied.
    fn holding(child: Id) -> Self {
        let tr = Self::new();
        set(Some(&tr), Some(child));
        tr.events();
        tr
    }

    fn ev(&self, ev: Ev) {
        self.st.borrow_mut().events.push(ev);
    }
    fn locked(&self) -> bool {
        self.st.borrow().locked
    }
    fn read<V>(&self, f: F, get: impl FnOnce(&Fields) -> V) -> V {
        let locked = self.locked();
        self.ev(Ev::R(f, locked));
        get(&self.st.borrow().f)
    }
    fn write(&self, f: F, put: impl FnOnce(&mut Fields)) {
        let locked = self.locked();
        self.ev(Ev::W(f, locked));
        put(&mut self.st.borrow_mut().f);
    }
    fn events(&self) -> Vec<Ev> {
        std::mem::take(&mut self.st.borrow_mut().events)
    }
    fn fields(&self) -> Fields {
        self.st.borrow().f.clone()
    }
    fn slots(&self) -> [Option<Id>; 2] {
        self.st.borrow().f.sources
    }
    fn running(&self) -> bool {
        let f = &self.st.borrow().f;
        f.audio || f.video
    }
    fn refs(&self, id: Id) -> i64 {
        self.st.borrow().refs.get(&id).copied().unwrap_or(0)
    }
    fn link(&self, id: Id) -> i64 {
        self.st.borrow().links.get(&id).copied().unwrap_or(0)
    }
    /// `obs_source_active()` of a child, while the transition is on an
    /// output channel: it has an active-child link.
    fn child_active(&self, id: Id) -> bool {
        self.link(id) > 0
    }
    fn starts(&self) -> usize {
        self.st.borrow().starts
    }
    /// The signals sent so far: (start, stop).
    fn signals(&self, events: &[Ev]) -> (usize, usize) {
        let count = |s| {
            events
                .iter()
                .filter(|e| matches!(e, Ev::Signal(o, _) if *o == s))
                .count()
        };
        (count(START_SIGNALS.0), count(STOP_SIGNALS.0))
    }
    fn warnings(events: &[Ev]) -> Vec<(&'static CStr, Option<Id>)> {
        events
            .iter()
            .filter_map(|e| match e {
                Ev::Warn(f, o) => Some((*f, *o)),
                _ => None,
            })
            .collect()
    }
}

impl Transition for Tr {
    type Child = Id;

    fn valid(source: Option<&Self>, func: &'static CStr) -> bool {
        source.is_some_and(|s| {
            s.ev(Ev::Valid(func));
            s.st.borrow().is_transition
        })
    }
    fn is(&self, child: Id) -> bool {
        child == TR
    }
    fn active(&self) -> bool {
        let locked = self.locked();
        self.ev(Ev::IsActive(locked));
        self.running()
    }
    fn source(&self, idx: usize) -> Option<Id> {
        self.read(F::Source(idx), |f| f.sources[idx])
    }
    fn set_source(&self, idx: usize, value: Option<Id>) {
        self.write(F::Source(idx), |f| f.sources[idx] = value);
    }
    fn source_active(&self, idx: usize) -> bool {
        self.read(F::Active(idx), |f| f.active[idx])
    }
    fn set_source_active(&self, idx: usize, value: bool) {
        self.write(F::Active(idx), |f| f.active[idx] = value);
    }
    fn transitioning_video(&self) -> bool {
        self.read(F::Video, |f| f.video)
    }
    fn set_transitioning_video(&self, value: bool) {
        self.write(F::Video, |f| f.video = value);
    }
    fn transitioning_audio(&self) -> bool {
        self.read(F::Audio, |f| f.audio)
    }
    fn set_transitioning_audio(&self, value: bool) {
        self.write(F::Audio, |f| f.audio = value);
    }
    fn mode(&self) -> i32 {
        self.read(F::Mode, |f| f.mode)
    }
    fn set_mode(&self, value: i32) {
        self.write(F::Mode, |f| f.mode = value);
    }
    fn set_manual_val(&self, value: f32) {
        self.write(F::ManualVal, |f| f.manual_val = value);
    }
    fn set_manual_target(&self, value: f32) {
        self.write(F::ManualTarget, |f| f.manual_target = value);
    }
    fn set_start_time(&self, value: u64) {
        self.write(F::StartTime, |f| f.start_time = value);
    }
    fn set_duration(&self, value: u64) {
        self.write(F::Duration, |f| f.duration = value);
    }
    fn use_fixed_duration(&self) -> bool {
        self.read(F::UseFixed, |f| f.use_fixed)
    }
    fn fixed_duration(&self) -> u32 {
        self.read(F::Fixed, |f| f.fixed)
    }
    fn cx(&self) -> u32 {
        self.read(F::Cx, |f| f.cx)
    }
    fn cy(&self) -> u32 {
        self.read(F::Cy, |f| f.cy)
    }
    fn lock(&self) {
        self.ev(Ev::Lock);
        let mut st = self.st.borrow_mut();
        assert!(!st.locked, "transition_mutex taken twice");
        st.locks += 1;
        if let Some((at, action)) = st.between
            && at == st.locks
        {
            action(&mut st.f);
        }
        st.locked = true;
    }
    fn unlock(&self) {
        self.ev(Ev::Unlock);
        let mut st = self.st.borrow_mut();
        assert!(st.locked, "transition_mutex released unheld");
        st.locked = false;
    }
    fn call_start(&self) {
        assert!(!self.locked(), "the plugin called with the lock held");
        self.ev(Ev::CallStart);
        let reenter = {
            let mut st = self.st.borrow_mut();
            st.starts += 1;
            st.reenter.take()
        };
        match reenter {
            Some(Reenter::Start(mode, ms, dest)) => {
                start(Some(self), mode, ms, dest);
            }
            Some(Reenter::Set(source)) => set(Some(self), source),
            None => {}
        }
    }
    fn get_ref(&self, child: Option<Id>) -> Option<Id> {
        self.ev(Ev::GetRef(child));
        let mut st = self.st.borrow_mut();
        let c = child?;
        if c == TR {
            return Some(TR);
        }
        if st.dying.contains(&c) {
            return None;
        }
        *st.refs.entry(c).or_insert(0) += 1;
        Some(c)
    }
    fn release(&self, child: Option<Id>) {
        self.ev(Ev::Release(child));
        if let Some(c) = child
            && c != TR
        {
            *self.st.borrow_mut().refs.entry(c).or_insert(0) -= 1;
        }
    }
    fn add_active_child(&self, child: Option<Id>) -> bool {
        let ok = {
            let st = self.st.borrow();
            child.is_some_and(|c| c != TR && !st.refuses.contains(&c))
        };
        self.ev(Ev::Add(child, ok));
        if let (true, Some(c)) = (ok, child) {
            *self.st.borrow_mut().links.entry(c).or_insert(0) += 1;
        }
        ok
    }
    fn remove_active_child(&self, child: Id) {
        self.ev(Ev::Remove(child));
        *self.st.borrow_mut().links.entry(child).or_insert(0) -= 1;
    }
    fn enum_full_tree(&self, root: Id, visit: &mut dyn FnMut(Id, Id)) {
        self.ev(Ev::Walk(root));
        let pairs = self
            .st
            .borrow()
            .trees
            .get(&root)
            .cloned()
            .unwrap_or_default();
        for (parent, child) in pairs {
            self.ev(Ev::Visit(parent, child));
            visit(parent, child);
        }
    }
    fn now(&self) -> u64 {
        self.ev(Ev::Now);
        let mut st = self.st.borrow_mut();
        st.clock += 1_000;
        st.clock
    }
    fn dosignal(&self, signal_obs: &'static CStr, signal_source: &'static CStr) {
        self.ev(Ev::Signal(signal_obs, signal_source));
    }
    fn recalculate_size(&self) {
        self.ev(Ev::RecalcSize);
    }
    fn recalculate_matrices(&self) {
        self.ev(Ev::RecalcMatrices);
    }
    fn warn(&self, format: &'static CStr, other: Option<Id>) {
        self.ev(Ev::Warn(format, other));
    }
}

use Ev::*;
use F::*;

/// `check_slots()`: what each slot holds, and whether the transition runs.
fn check_slots(tr: &Tr, a: Option<Id>, b: Option<Id>, active: bool) {
    assert_eq!(tr.slots(), [a, b], "the slots");
    assert_eq!(tr.running(), active, "running");
}

/// `check_counts()`: the callback count and the start and stop signals,
/// counted over `events`.
fn check_counts(tr: &Tr, events: &[Ev], calls: usize, starts: usize, stops: usize) {
    assert_eq!(tr.starts(), calls, "plugin starts");
    assert_eq!(tr.signals(events), (starts, stops), "(start, stop) signals");
}

// ---------------------------------------------------------------------------
// The constants

#[test]
fn names_warnings_and_signals_are_the_cs() {
    assert_eq!(START_LOG_NAME, c"obs_transition_start");
    assert_eq!(SET_LOG_NAME, c"obs_transition_set");
    assert_eq!(
        START_RECURSION_WARNING,
        c"obs_transition_start: transition '%s' has recursive destination '%s'"
    );
    assert_eq!(
        SET_RECURSION_WARNING,
        c"obs_transition_set: cannot set transition '%s' to source '%s' due to infinite recursion"
    );
    assert_eq!(
        START_SIGNALS,
        (c"source_transition_start", c"transition_start")
    );
    assert_eq!(
        STOP_SIGNALS,
        (c"source_transition_stop", c"transition_stop")
    );
    assert_eq!((SOURCE_A, SOURCE_B), (0, 1));
    assert_eq!(
        StartClock::new(5),
        StartClock {
            duration: 5,
            same_as_source: false,
            same_as_dest: false,
            restart: false
        }
    );
}

// ---------------------------------------------------------------------------
// test_source_transition_switch.c, case by case

#[test]
fn c_start_from_idle() {
    let tr = Tr::holding(A);
    let mut ev = Vec::new();
    check_counts(&tr, &ev, 0, 0, 0);

    assert!(start(Some(&tr), AUTO, 60_000, Some(B)));
    ev.extend(tr.events());
    check_slots(&tr, Some(A), Some(B), true);
    check_counts(&tr, &ev, 1, 1, 0);
    assert!(Tr::warnings(&ev).is_empty());
}

#[test]
fn c_start_to_source_a_idle_is_refused() {
    let tr = Tr::holding(A);
    let mut ev = Vec::new();

    assert!(!start(Some(&tr), AUTO, 60_000, Some(A)));
    check_slots(&tr, Some(A), None, false);
    assert!(!start(Some(&tr), MANUAL, 0, Some(A)));
    check_slots(&tr, Some(A), None, false);
    ev.extend(tr.events());
    check_counts(&tr, &ev, 0, 0, 0);

    set(Some(&tr), None);
    assert!(!start(Some(&tr), AUTO, 60_000, None));
    ev.extend(tr.events());
    check_slots(&tr, None, None, false);
    check_counts(&tr, &ev, 0, 0, 0);
    assert!(Tr::warnings(&ev).is_empty());
}

#[test]
fn c_start_to_source_a_while_running() {
    let tr = Tr::holding(A);
    let mut ev = Vec::new();

    assert!(start(Some(&tr), AUTO, 60_000, Some(B)));
    assert!(start(Some(&tr), AUTO, 60_000, Some(A)));
    ev.extend(tr.events());
    check_slots(&tr, Some(B), Some(A), true);
    check_counts(&tr, &ev, 2, 2, 1);

    assert!(start(Some(&tr), AUTO, 60_000, Some(C)));
    ev.extend(tr.events());
    check_slots(&tr, Some(A), Some(C), true);
    check_counts(&tr, &ev, 3, 3, 2);
    assert!(Tr::warnings(&ev).is_empty());
}

#[test]
fn c_start_to_running_destination() {
    let tr = Tr::holding(A);
    let mut ev = Vec::new();

    assert!(start(Some(&tr), MANUAL, 0, Some(B)));
    check_slots(&tr, Some(A), Some(B), true);
    assert!(!start(Some(&tr), MANUAL, 0, Some(B)));
    check_slots(&tr, Some(B), None, false);
    ev.extend(tr.events());
    check_counts(&tr, &ev, 1, 1, 1);

    assert!(start(Some(&tr), AUTO, 60_000, Some(C)));
    assert!(!start(Some(&tr), AUTO, 60_000, Some(C)));
    ev.extend(tr.events());
    check_slots(&tr, Some(C), None, false);
    check_counts(&tr, &ev, 2, 2, 2);

    assert!(start(Some(&tr), MANUAL, 0, Some(A)));
    assert!(!start(Some(&tr), AUTO, 60_000, Some(A)));
    ev.extend(tr.events());
    check_slots(&tr, Some(A), None, false);
    check_counts(&tr, &ev, 3, 3, 3);
    assert!(Tr::warnings(&ev).is_empty());
}

#[test]
fn c_start_to_no_destination() {
    let tr = Tr::holding(A);
    let mut ev = Vec::new();

    assert!(start(Some(&tr), AUTO, 60_000, None));
    ev.extend(tr.events());
    check_slots(&tr, Some(A), None, true);
    check_counts(&tr, &ev, 1, 1, 0);
    // obs_transition_get_active_source(): slot B while running.
    assert_eq!(tr.slots()[1], None);

    assert!(!start(Some(&tr), AUTO, 60_000, None));
    ev.extend(tr.events());
    check_slots(&tr, None, None, false);
    check_counts(&tr, &ev, 1, 1, 1);
    assert!(Tr::warnings(&ev).is_empty());
}

#[test]
fn c_fixed_duration_replaces_the_asked_one() {
    let tr = Tr::holding(A);
    {
        let mut st = tr.st.borrow_mut();
        st.f.use_fixed = true;
        st.f.fixed = 200;
    }
    assert!(start(Some(&tr), AUTO, 600_000, Some(B)));
    assert_eq!(tr.fields().duration, 200_000_000);
    assert!(tr.events().contains(&R(Fixed, false)));

    set(Some(&tr), Some(A));
    tr.st.borrow_mut().f.use_fixed = false;
    tr.events();
    assert!(start(Some(&tr), AUTO, 600_000, Some(B)));
    assert_eq!(tr.fields().duration, 600_000_000_000);
    assert_eq!(tr.fields().fixed, 200, "the fixed duration stays stored");
    assert!(!tr.events().contains(&R(Fixed, false)), "not read when off");
    assert!(tr.running());
}

#[test]
fn c_reentering_callback() {
    let tr = Tr::holding(A);
    tr.st.borrow_mut().reenter = Some(Reenter::Start(AUTO, 60_000, Some(C)));

    assert!(start(Some(&tr), AUTO, 60_000, Some(B)));
    let ev = tr.events();
    check_slots(&tr, Some(A), Some(B), true);
    check_counts(&tr, &ev, 2, 2, 0);
    assert!(Tr::warnings(&ev).is_empty());
    // The nested start's child is let go again: only the slots hold refs.
    assert_eq!((tr.refs(A), tr.refs(B), tr.refs(C)), (2, 2, 1));
    assert_eq!((tr.link(A), tr.link(B), tr.link(C)), (1, 1, 0));
}

#[test]
fn c_set_clears_both_slots() {
    let tr = Tr::holding(A);
    let mut ev = Vec::new();
    check_counts(&tr, &ev, 0, 0, 0);

    assert!(start(Some(&tr), AUTO, 60_000, Some(B)));
    set(Some(&tr), Some(C));
    ev.extend(tr.events());
    check_slots(&tr, Some(C), None, false);
    check_counts(&tr, &ev, 1, 1, 1);

    set(Some(&tr), Some(A));
    check_slots(&tr, Some(A), None, false);
    set(Some(&tr), None);
    check_slots(&tr, None, None, false);
    ev.extend(tr.events());
    check_counts(&tr, &ev, 1, 1, 1);
    assert!(Tr::warnings(&ev).is_empty());

    set(Some(&tr), Some(A));
    set(Some(&tr), Some(TR));
    ev.extend(tr.events());
    check_slots(&tr, Some(A), None, false);
    check_counts(&tr, &ev, 1, 1, 1);
    assert_eq!(Tr::warnings(&ev), vec![(SET_RECURSION_WARNING, Some(TR))]);
}

#[test]
fn c_active_children_follow_the_slots() {
    let tr = Tr::holding(A);
    let check_active = |a, b, c| {
        assert_eq!(
            (tr.child_active(A), tr.child_active(B), tr.child_active(C)),
            (a, b, c)
        );
    };
    check_active(true, false, false);

    assert!(start(Some(&tr), AUTO, 60_000, Some(B)));
    check_active(true, true, false);

    assert!(start(Some(&tr), AUTO, 60_000, Some(C)));
    check_slots(&tr, Some(B), Some(C), true);
    check_active(false, true, true);

    set(Some(&tr), Some(A));
    check_active(true, false, false);

    set(Some(&tr), None);
    check_active(false, false, false);
}

// ---------------------------------------------------------------------------
// The C's order, whole

/// A start from idle, read for read: the first check under the lock, the
/// walk, the second check, the mode, the plugin, the fixed duration (read
/// unlocked), `set_source()` with the add under the lock, the size, the
/// signal and the size again.
#[test]
fn a_start_from_idle_in_the_cs_order() {
    let tr = Tr::holding(A);
    assert!(start(Some(&tr), AUTO, 60_000, Some(B)));
    assert_eq!(
        tr.events(),
        vec![
            Valid(START_LOG_NAME),
            Lock,
            R(Source(0), true),
            IsActive(true),
            Unlock,
            Walk(B),
            IsActive(false),
            Lock,
            R(Source(0), true),
            R(Source(1), true),
            R(Mode, true),
            IsActive(true),
            Unlock,
            Lock,
            W(Mode, true),
            W(ManualVal, true),
            W(ManualTarget, true),
            Unlock,
            CallStart,
            R(UseFixed, false),
            // set_source(B)
            GetRef(Some(B)),
            Lock,
            R(Source(1), true),
            R(Source(0), true),
            R(Audio, true),
            R(Video, true),
            R(Active(1), true),
            // activate_transition()
            R(Active(1), true),
            Add(Some(B), true),
            W(Active(1), true),
            W(Video, true),
            W(Audio, true),
            // write_start_clock(), then the slot
            Now,
            W(StartTime, true),
            W(Duration, true),
            W(Source(1), true),
            Unlock,
            R(Cx, false),
            RecalcSize,
            RecalcMatrices,
            Release(None),
            Signal(START_SIGNALS.0, START_SIGNALS.1),
            RecalcSize,
            RecalcMatrices,
        ]
    );
    let f = tr.fields();
    assert_eq!(
        (
            f.mode,
            f.manual_val,
            f.manual_target,
            f.start_time,
            f.duration
        ),
        (AUTO, 0.0, 0.0, 2_000, 60_000_000_000)
    );
}

/// A start while running finishes the running transition first
/// (`obs_transition_set()` to its source B, read without the lock).
#[test]
fn a_start_while_running_sets_source_b_first() {
    let tr = Tr::holding(A);
    assert!(start(Some(&tr), AUTO, 60_000, Some(B)));
    tr.events();
    assert!(start(Some(&tr), MANUAL, 0, Some(C)));
    let ev = tr.events();
    assert_eq!(
        ev[..9],
        [
            Valid(START_LOG_NAME),
            Lock,
            R(Source(0), true),
            IsActive(true),
            Unlock,
            Walk(C),
            IsActive(false),
            R(Source(1), false),
            Valid(SET_LOG_NAME),
        ]
    );
    check_slots(&tr, Some(B), Some(C), true);
    assert_eq!(tr.fields().mode, MANUAL);
    assert_eq!(tr.signals(&ev), (1, 1));
}

/// A set of a running transition, read for read: the walk, the reference,
/// the stop signal, both slots emptied and slot A filled under the lock,
/// then the old children let go and the new one added.
#[test]
fn a_set_while_running_in_the_cs_order() {
    let tr = Tr::holding(A);
    assert!(start(Some(&tr), AUTO, 60_000, Some(B)));
    tr.events();
    set(Some(&tr), Some(C));
    assert_eq!(
        tr.events(),
        vec![
            Valid(SET_LOG_NAME),
            Walk(C),
            GetRef(Some(C)),
            IsActive(false),
            Signal(STOP_SIGNALS.0, STOP_SIGNALS.1),
            Lock,
            R(Source(0), true),
            R(Active(0), true),
            W(Source(0), true),
            W(Active(0), true),
            R(Source(1), true),
            R(Active(1), true),
            W(Source(1), true),
            W(Active(1), true),
            W(Active(0), true),
            W(Source(0), true),
            W(Video, true),
            W(Audio, true),
            W(ManualVal, true),
            W(ManualTarget, true),
            Unlock,
            Remove(A),
            Release(Some(A)),
            Remove(B),
            Release(Some(B)),
            Add(Some(C), true),
        ]
    );
    let f = tr.fields();
    assert_eq!(f.sources, [Some(C), None]);
    assert_eq!(f.active, [true, false]);
    assert!(!f.video && !f.audio);
    assert_eq!((f.manual_val, f.manual_target), (0.0, 0.0));
    assert_eq!((tr.refs(A), tr.refs(B), tr.refs(C)), (1, 1, 2));
}

/// An idle set to nothing: no walk, the reference call with NULL, no
/// signal, no add.
#[test]
fn a_set_to_nothing() {
    let tr = Tr::new();
    set(Some(&tr), None);
    assert_eq!(
        tr.events(),
        vec![
            Valid(SET_LOG_NAME),
            GetRef(None),
            IsActive(false),
            Lock,
            R(Source(0), true),
            R(Active(0), true),
            W(Source(0), true),
            W(Active(0), true),
            R(Source(1), true),
            R(Active(1), true),
            W(Source(1), true),
            W(Active(1), true),
            W(Active(0), true),
            W(Source(0), true),
            W(Video, true),
            W(Audio, true),
            W(ManualVal, true),
            W(ManualTarget, true),
            Unlock,
            Release(None),
            Release(None),
        ]
    );
    assert_eq!(tr.fields().active, [true, false]);
}

// ---------------------------------------------------------------------------
// The refusals

#[test]
fn no_source_and_not_a_transition_do_nothing() {
    assert!(!start::<Tr>(None, AUTO, 1, Some(A)));
    set::<Tr>(None, Some(A));

    let tr = Tr::new();
    tr.st.borrow_mut().is_transition = false;
    assert!(!start(Some(&tr), AUTO, 1, Some(A)));
    set(Some(&tr), Some(A));
    assert_eq!(
        tr.events(),
        vec![Valid(START_LOG_NAME), Valid(SET_LOG_NAME)]
    );
}

/// The transition itself, or a scene holding it, as a start's destination:
/// one warning and nothing else; the transition itself is not walked.
#[test]
fn a_recursive_destination_is_refused_before_anything_changes() {
    for (dest, walked) in [(TR, false), (SCENE, true)] {
        let tr = Tr::holding(A);
        let before = tr.fields();
        assert!(!start(Some(&tr), AUTO, 60_000, Some(dest)));
        let mut want = vec![
            Valid(START_LOG_NAME),
            Lock,
            R(Source(0), true),
            IsActive(true),
            Unlock,
        ];
        if walked {
            want.extend([Walk(SCENE), Visit(SCENE, A), Visit(SCENE, TR)]);
        }
        want.push(Warn(START_RECURSION_WARNING, Some(dest)));
        assert_eq!(tr.events(), want, "destination {dest}");
        assert_eq!(tr.fields(), before);
        assert_eq!(tr.starts(), 0);
    }
}

#[test]
fn a_recursive_source_is_refused_before_anything_changes() {
    for (source, walked) in [(TR, false), (SCENE, true)] {
        let tr = Tr::holding(A);
        assert!(start(Some(&tr), AUTO, 60_000, Some(B)));
        tr.events();
        let before = tr.fields();
        set(Some(&tr), Some(source));
        let mut want = vec![Valid(SET_LOG_NAME)];
        if walked {
            want.extend([Walk(SCENE), Visit(SCENE, A), Visit(SCENE, TR)]);
        }
        want.push(Warn(SET_RECURSION_WARNING, Some(source)));
        assert_eq!(tr.events(), want, "source {source}");
        assert_eq!(tr.fields(), before);
    }
}

#[test]
fn recursive_checks_the_destination_and_its_tree() {
    let tr = Tr::new();
    assert!(recursive(&tr, Some(TR)));
    assert!(recursive(&tr, Some(SCENE)));
    assert!(!recursive(&tr, Some(A)));
    assert!(!recursive(&tr, None));
    // A tree where the transition is a parent counts too.
    tr.st.borrow_mut().trees.insert(B, vec![(B, C), (TR, A)]);
    assert!(recursive(&tr, Some(B)));
    assert_eq!(
        tr.events(),
        vec![
            Walk(SCENE),
            Visit(SCENE, A),
            Visit(SCENE, TR),
            Walk(A),
            Walk(B),
            Visit(B, C),
            Visit(TR, A),
        ]
    );
}

/// Source B refusing the destination (its add fails): the start's warning,
/// the reference let go, no signal; the mode and plugin call stay.
#[test]
fn a_refused_add_warns_and_lets_the_destination_go() {
    let tr = Tr::holding(A);
    tr.st.borrow_mut().refuses.insert(B);
    let before = tr.fields();
    assert!(!start(Some(&tr), MANUAL, 60_000, Some(B)));
    let ev = tr.events();
    assert_eq!(Tr::warnings(&ev), vec![(START_RECURSION_WARNING, Some(B))]);
    assert_eq!(tr.signals(&ev), (0, 0));
    assert_eq!(tr.starts(), 1);
    let f = tr.fields();
    assert_eq!(f.sources, before.sources);
    assert_eq!(f.active, before.active);
    assert_eq!(
        (f.start_time, f.duration),
        (before.start_time, before.duration)
    );
    assert_eq!(f.mode, MANUAL, "written before the plugin call");
    assert!(!tr.running());
    assert_eq!(tr.refs(B), 1);
    assert!(!ev.contains(&RecalcSize));
}

/// A going-away destination: no reference, so its new child is NULL, the
/// same as the empty slot B. The start succeeds with nothing added and
/// signals, but does not run: the null-destination flags are only for a
/// start asked to go to nothing (characterized, not endorsed).
#[test]
fn a_dying_destination_starts_nothing() {
    let tr = Tr::holding(A);
    tr.st.borrow_mut().dying.insert(B);
    assert!(start(Some(&tr), AUTO, 60_000, Some(B)));
    let ev = tr.events();
    assert!(ev.contains(&GetRef(Some(B))));
    assert!(!ev.iter().any(|e| matches!(e, Add(..))));
    assert!(Tr::warnings(&ev).is_empty());
    assert_eq!(tr.signals(&ev), (1, 0));
    check_slots(&tr, Some(A), None, false);
    assert_eq!(tr.refs(B), 1);
}

/// A start to nothing with slot B empty takes no reference and adds
/// nothing: the NULL is the slot's own.
#[test]
fn a_start_to_nothing_takes_and_adds_nothing() {
    let tr = Tr::holding(A);
    assert!(start(Some(&tr), AUTO, 1, None));
    let ev = tr.events();
    assert!(!ev.iter().any(|e| matches!(e, GetRef(_) | Add(..))));
}

// ---------------------------------------------------------------------------
// The admission

/// A manual start to the running manual destination is done with nothing
/// called, once another thread starts it between the finish and the second
/// check.
#[test]
fn a_running_manual_destination_is_done() {
    let tr = Tr::holding(A);
    assert!(start(Some(&tr), MANUAL, 0, Some(B)));
    tr.events();
    // The third lock call of the next start is its second check (the
    // first check, then the finishing set's).
    let second_check = tr.st.borrow().locks + 3;
    tr.st.borrow_mut().between = Some((second_check, |f| {
        f.sources = [Some(A), Some(B)];
        f.video = true;
        f.mode = MANUAL;
    }));
    let before_starts = tr.starts();
    assert!(start(Some(&tr), MANUAL, 0, Some(B)));
    let ev = tr.events();
    assert_eq!(tr.starts(), before_starts);
    assert!(!ev.contains(&W(Mode, true)));
    assert_eq!(tr.signals(&ev), (0, 1));
    assert_eq!(ev.last(), Some(&Unlock));
}

/// The same start in auto mode is not done: it goes on.
#[test]
fn a_running_auto_destination_goes_on() {
    let tr = Tr::holding(A);
    tr.st.borrow_mut().between = Some((3, |f| {
        f.sources = [Some(A), Some(B)];
        f.video = true;
        f.mode = AUTO;
    }));
    assert!(start(Some(&tr), AUTO, 0, Some(B)));
    assert_eq!(tr.starts(), 1);
}

// ---------------------------------------------------------------------------
// set_source() and activate_transition()

fn admit_all(_: &Tr, _: usize, _: Option<Id>) -> bool {
    true
}

fn admit_none(_: &Tr, _: usize, _: Option<Id>) -> bool {
    false
}

/// The same child again: the clock written when it restarts, nothing else,
/// and the new reference let go.
#[test]
fn set_source_same_child() {
    for running in [false, true] {
        let tr = Tr::holding(A);
        tr.st.borrow_mut().f.sources[1] = Some(B);
        tr.st.borrow_mut().f.audio = running;
        let mut clock = StartClock::new(42);
        assert!(set_source(&tr, 1, Some(B), Some(admit_all), &mut clock));
        let mut want = vec![
            GetRef(Some(B)),
            Lock,
            R(Source(1), true),
            R(Source(0), true),
            R(Audio, true),
        ];
        if !running {
            want.push(R(Video, true));
            want.extend([Now, W(StartTime, true), W(Duration, true)]);
        }
        want.extend([Unlock, Release(Some(B))]);
        assert_eq!(tr.events(), want, "running {running}");
        assert_eq!(
            clock,
            StartClock {
                duration: 42,
                same_as_source: false,
                same_as_dest: true,
                restart: !running
            }
        );
    }
}

/// An active slot hands over: the new child added, then the old one
/// removed, then the callback; a failed add removes nothing and calls
/// nothing.
#[test]
fn set_source_active_slot_hand_over() {
    let tr = Tr::holding(A);
    {
        let mut st = tr.st.borrow_mut();
        st.f.sources[1] = Some(B);
        st.f.active[1] = true;
        st.f.cx = 5;
        st.f.cy = 6;
    }
    let mut clock = StartClock::new(1);
    assert!(set_source(&tr, 1, Some(C), Some(admit_all), &mut clock));
    assert_eq!(
        tr.events(),
        vec![
            GetRef(Some(C)),
            Lock,
            R(Source(1), true),
            R(Source(0), true),
            R(Audio, true),
            R(Video, true),
            R(Active(1), true),
            Add(Some(C), true),
            Remove(B),
            Now,
            W(StartTime, true),
            W(Duration, true),
            W(Source(1), true),
            Unlock,
            R(Cx, false),
            R(Cy, false),
            Release(Some(B)),
        ]
    );

    tr.st.borrow_mut().refuses.insert(A);
    let mut clock = StartClock::new(1);
    assert!(!set_source(&tr, 1, Some(A), Some(admit_all), &mut clock));
    assert_eq!(
        tr.events(),
        vec![
            GetRef(Some(A)),
            Lock,
            R(Source(1), true),
            R(Source(0), true),
            R(Audio, true),
            R(Video, true),
            R(Active(1), true),
            Add(Some(A), false),
            Unlock,
            Release(Some(A)),
        ]
    );
    assert_eq!(tr.slots(), [Some(A), Some(C)]);
    assert!(clock.same_as_source);
}

/// The callback refusing: the slot kept, the clock not written, the new
/// reference let go; no callback at all stores the child.
#[test]
fn set_source_callback_decides() {
    let tr = Tr::holding(A);
    let mut clock = StartClock::new(1);
    assert!(!set_source(&tr, 1, Some(B), Some(admit_none), &mut clock));
    let ev = tr.events();
    assert!(!ev.contains(&Now));
    assert_eq!(ev.last(), Some(&Release(Some(B))));
    assert_eq!(tr.slots(), [Some(A), None]);
    assert_eq!(tr.refs(B), 1);

    let mut clock = StartClock::new(1);
    assert!(set_source(&tr, 1, Some(B), None, &mut clock));
    assert_eq!(tr.slots(), [Some(A), Some(B)]);
    assert_eq!(tr.refs(B), 2);
    assert_eq!(tr.link(B), 0, "an inactive slot adds nothing");
}

/// The size: `cy` is read only when `cx` is not 0, and either 0
/// recalculates.
#[test]
fn set_source_recalculates_when_unsized() {
    for (cx, cy, reads_cy, recalc) in [
        (0, 0, false, true),
        (0, 9, false, true),
        (9, 0, true, true),
        (9, 9, true, false),
    ] {
        let tr = Tr::holding(A);
        tr.st.borrow_mut().f.cx = cx;
        tr.st.borrow_mut().f.cy = cy;
        let mut clock = StartClock::new(1);
        assert!(set_source(&tr, 1, Some(B), None, &mut clock));
        let ev = tr.events();
        assert_eq!(ev.contains(&R(Cy, false)), reads_cy, "{cx}x{cy}");
        assert_eq!(ev.contains(&RecalcSize), recalc, "{cx}x{cy}");
        assert_eq!(ev.contains(&RecalcMatrices), recalc, "{cx}x{cy}");
    }
}

#[test]
fn activate_transition_steps() {
    // An active slot: no add, the flags set.
    let tr = Tr::new();
    tr.st.borrow_mut().f.active[1] = true;
    assert!(activate_transition(&tr, 1, Some(B)));
    assert_eq!(
        tr.events(),
        vec![R(Active(1), false), W(Video, false), W(Audio, false)]
    );
    assert!(tr.running());

    // An inactive slot: the add, the slot active, the flags.
    let tr = Tr::new();
    assert!(activate_transition(&tr, 1, Some(B)));
    assert_eq!(
        tr.events(),
        vec![
            R(Active(1), false),
            Add(Some(B), true),
            W(Active(1), false),
            W(Video, false),
            W(Audio, false),
        ]
    );

    // A failed add: false, nothing written.
    let tr = Tr::new();
    assert!(!activate_transition(&tr, 0, None));
    assert_eq!(tr.events(), vec![R(Active(0), false), Add(None, false)]);
    assert!(!tr.running());
}

// ---------------------------------------------------------------------------
// The clock

#[test]
fn write_start_clock_writes_only_a_restart() {
    let tr = Tr::new();
    let mut clock = StartClock::new(77);
    write_start_clock(&tr, &clock);
    assert!(tr.events().is_empty());
    clock.restart = true;
    write_start_clock(&tr, &clock);
    assert_eq!(
        tr.events(),
        vec![Now, W(StartTime, false), W(Duration, false)]
    );
    assert_eq!((tr.fields().start_time, tr.fields().duration), (2_000, 77));
}

/// A start the plugin makes stays when the outer start is then refused: the
/// nested start's clock and destination are kept.
#[test]
fn a_refused_start_keeps_a_nested_starts_clock() {
    let tr = Tr::holding(A);
    {
        let mut st = tr.st.borrow_mut();
        st.reenter = Some(Reenter::Start(AUTO, 5, Some(C)));
        st.refuses.insert(B);
    }
    assert!(!start(Some(&tr), AUTO, 60_000, Some(B)));
    let f = tr.fields();
    assert_eq!(f.sources, [Some(A), Some(C)]);
    assert_eq!(f.duration, 5_000_000);
    assert_eq!(tr.starts(), 2);
    let ev = tr.events();
    assert_eq!(tr.signals(&ev), (1, 0));
    assert_eq!(Tr::warnings(&ev), vec![(START_RECURSION_WARNING, Some(B))]);
}

/// The plugin setting the transition to the destination: the outer start
/// then finds it in slot A and restarts the clock towards it again.
#[test]
fn a_set_from_the_plugin_is_read_again() {
    let tr = Tr::holding(A);
    tr.st.borrow_mut().reenter = Some(Reenter::Set(Some(B)));
    assert!(start(Some(&tr), AUTO, 60_000, Some(B)));
    let f = tr.fields();
    assert_eq!(f.sources, [Some(B), Some(B)]);
    assert!(tr.running());
    assert_eq!(f.duration, 60_000_000_000);
}

/// A restart to source B's own destination keeps the clock: the start is
/// to a running transition's source B, which re-entry put there.
#[test]
fn the_same_destination_while_running_keeps_the_clock() {
    let tr = Tr::holding(A);
    tr.st.borrow_mut().reenter = Some(Reenter::Start(AUTO, 5, Some(B)));
    assert!(start(Some(&tr), AUTO, 60_000, Some(B)));
    // The nested start wrote its clock; the outer one found B again while
    // running, so it kept it.
    assert_eq!(tr.fields().duration, 5_000_000);
    assert_eq!(tr.signals(&tr.events()), (2, 0));
}

/// The null destination's flags: a start to nothing with slot B empty and
/// slot A held sets both running flags itself.
#[test]
fn a_start_to_nothing_sets_the_flags() {
    let tr = Tr::holding(A);
    assert!(start(Some(&tr), AUTO, 60_000, None));
    let ev = tr.events();
    let after_unlock = ev
        .iter()
        .rposition(|e| *e == Release(None))
        .map(|i| ev[i + 1..].to_vec());
    assert_eq!(
        after_unlock,
        Some(vec![
            W(Video, false),
            W(Audio, false),
            Signal(START_SIGNALS.0, START_SIGNALS.1),
            RecalcSize,
            RecalcMatrices,
        ])
    );
}

// ---------------------------------------------------------------------------
// Any sequence

#[derive(Debug, Clone, Copy)]
enum Op {
    Start(i32, u32, Option<Id>),
    Set(Option<Id>),
}

fn gen_child() -> impl Strategy<Value = Option<Id>> {
    proptest::option::of(A..=C)
}

fn gen_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..2i32, 0..100u32, gen_child()).prop_map(|(m, d, c)| Op::Start(m, d, c)),
        gen_child().prop_map(Op::Set),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(128)))]

    /// After any starts and sets: each child holds its own reference plus
    /// one per slot that holds it, an active-child link per active slot
    /// holding it, the lock is free, and every start signal had its
    /// plugin call.
    #[test]
    fn references_and_links_follow_the_slots(
        first in gen_child(),
        ops in proptest::collection::vec(gen_op(), 1..12),
    ) {
        let tr = Tr::new();
        set(Some(&tr), first);
        for op in ops {
            match op {
                Op::Start(m, d, c) => {
                    start(Some(&tr), m, d, c);
                }
                Op::Set(c) => set(Some(&tr), c),
            }
            let f = tr.fields();
            for id in [A, B, C] {
                let held = f.sources.iter().filter(|s| **s == Some(id)).count();
                let active = f
                    .sources
                    .iter()
                    .zip(f.active)
                    .filter(|(s, a)| **s == Some(id) && *a)
                    .count();
                prop_assert_eq!(tr.refs(id), 1 + i64::try_from(held).unwrap_or(0));
                prop_assert_eq!(tr.link(id), i64::try_from(active).unwrap_or(0));
            }
            prop_assert!(!tr.locked());
        }
        let ev = tr.events();
        prop_assert_eq!(tr.signals(&ev).0, tr.starts());
    }
}
