//! Tier 1 tests for `obs_source::transition_stop`, the safe core of
//! `libobs/obs-source-transition-stop.c.h`.
//!
//! The functions run on a fake transition: its slots, active flags, mode,
//! manual value, clock and lock, a video frame time, whether its type has a
//! `transition_stop` callback, and a reference count and active-child link
//! count per child. The fake records every field read and write (with
//! whether the lock was held), every lock call, every `transition_valid()`
//! and every call into the rest of libobs, in order, and panics on a lock
//! taken twice or released unheld.
//!
//! The cases pin the time rule in each mode (against
//! `obs_source::transition_clock::time`), the order of the reads, the lock
//! of the public read, the slot shift of the stop for each slot state, the
//! callback and signals of the stop, that the stop and the force stop take
//! no lock and that the force stop touches no field, and the argument
//! checks of the two public functions (the intentional difference from C:
//! the C checks nothing). A proptest checks the time and the slot shift,
//! references and links over any fields.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::CStr;

use obs_c_oracle::parity::cases;
use obs_source::transition_clock::{self, Clock};
use obs_source::transition_stop::{
    FORCE_STOP_LOG_NAME, GET_TIME_LOG_NAME, SOURCE_A, SOURCE_B, STOP_SIGNALS, Transition,
    calc_time, force_stop, get_time, get_video_time, handle_stop, stop,
};
use proptest::prelude::*;

/// A child the fake knows.
type Id = usize;

const A: Id = 1;
const B: Id = 2;

/// `OBS_TRANSITION_MODE_AUTO`, `OBS_TRANSITION_MODE_MANUAL`.
const AUTO: i32 = 0;
const MANUAL: i32 = 1;

/// A field of the transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum F {
    Source(usize),
    Active(usize),
    Mode,
    ManualVal,
    StartTime,
    Duration,
}

/// One thing the functions did, in order. A read or write, and the video
/// time, carry whether the lock was held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ev {
    R(F, bool),
    W(F, bool),
    Lock,
    Unlock,
    VideoTime(bool),
    /// The callback, and whether the type has one.
    CallStop(bool),
    Signal(&'static CStr, &'static CStr),
    Remove(Id),
    Release(Option<Id>),
}

#[derive(Debug, Clone, PartialEq)]
struct Fields {
    sources: [Option<Id>; 2],
    active: [bool; 2],
    mode: i32,
    manual_val: f32,
    start_time: u64,
    duration: u64,
}

#[derive(Debug)]
struct State {
    f: Fields,
    is_transition: bool,
    has_stop: bool,
    video_time: u64,
    locked: bool,
    events: Vec<Ev>,
    /// References held on each child.
    refs: BTreeMap<Id, i64>,
    /// The transition's active-child links to each child.
    links: BTreeMap<Id, i64>,
}

struct Tr {
    st: RefCell<State>,
}

thread_local! {
    /// Every `transition_valid()` call: the name, and whether a source was
    /// handed in (the fake can't record a call on no source on itself).
    static VALID: RefCell<Vec<(&'static CStr, bool)>> = const { RefCell::new(Vec::new()) };
}

fn valid_calls() -> Vec<(&'static CStr, bool)> {
    VALID.with(|v| std::mem::take(&mut *v.borrow_mut()))
}

impl Tr {
    /// An idle auto transition with nothing in its slots and a stop
    /// callback; each child holds one reference of its own.
    fn new() -> Self {
        let mut refs = BTreeMap::new();
        refs.insert(A, 1);
        refs.insert(B, 1);
        Self {
            st: RefCell::new(State {
                f: Fields {
                    sources: [None; 2],
                    active: [false; 2],
                    mode: AUTO,
                    manual_val: 0.25,
                    start_time: 1_000,
                    duration: 4_000,
                },
                is_transition: true,
                has_stop: true,
                video_time: 2_000,
                locked: false,
                events: Vec::new(),
                refs,
                links: BTreeMap::new(),
            }),
        }
    }

    /// `child` in slot `idx`, holding a reference of the slot's, and an
    /// active-child link when `active`.
    fn put(&self, idx: usize, child: Id, active: bool) {
        let mut st = self.st.borrow_mut();
        st.f.sources[idx] = Some(child);
        st.f.active[idx] = active;
        *st.refs.entry(child).or_insert(0) += 1;
        if active {
            *st.links.entry(child).or_insert(0) += 1;
        }
    }

    fn with(self, edit: impl FnOnce(&mut State)) -> Self {
        edit(&mut self.st.borrow_mut());
        self
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
    fn refs(&self, id: Id) -> i64 {
        self.st.borrow().refs.get(&id).copied().unwrap_or(0)
    }
    fn link(&self, id: Id) -> i64 {
        self.st.borrow().links.get(&id).copied().unwrap_or(0)
    }
    fn clock(&self) -> Clock {
        let f = &self.st.borrow().f;
        Clock {
            mode: f.mode,
            manual_val: f.manual_val,
            start_time: f.start_time,
            duration: f.duration,
        }
    }
}

impl Transition for Tr {
    type Child = Id;

    fn valid(source: Option<&Self>, func: &'static CStr) -> bool {
        VALID.with(|v| v.borrow_mut().push((func, source.is_some())));
        source.is_some_and(|s| s.st.borrow().is_transition)
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
    fn mode(&self) -> i32 {
        self.read(F::Mode, |f| f.mode)
    }
    fn manual_val(&self) -> f32 {
        self.read(F::ManualVal, |f| f.manual_val)
    }
    fn start_time(&self) -> u64 {
        self.read(F::StartTime, |f| f.start_time)
    }
    fn duration(&self) -> u64 {
        self.read(F::Duration, |f| f.duration)
    }
    fn lock(&self) {
        let mut st = self.st.borrow_mut();
        assert!(!st.locked, "transition_mutex locked twice");
        st.locked = true;
        st.events.push(Ev::Lock);
    }
    fn unlock(&self) {
        let mut st = self.st.borrow_mut();
        assert!(st.locked, "transition_mutex unlocked unheld");
        st.locked = false;
        st.events.push(Ev::Unlock);
    }
    fn video_time(&self) -> u64 {
        let locked = self.locked();
        self.ev(Ev::VideoTime(locked));
        self.st.borrow().video_time
    }
    fn call_stop(&self) {
        let has = self.st.borrow().has_stop;
        self.ev(Ev::CallStop(has));
    }
    fn dosignal(&self, signal_obs: &'static CStr, signal_source: &'static CStr) {
        self.ev(Ev::Signal(signal_obs, signal_source));
    }
    fn remove_active_child(&self, child: Id) {
        self.ev(Ev::Remove(child));
        *self.st.borrow_mut().links.entry(child).or_insert(0) -= 1;
    }
    fn release(&self, child: Option<Id>) {
        self.ev(Ev::Release(child));
        if let Some(c) = child {
            *self.st.borrow_mut().refs.entry(c).or_insert(0) -= 1;
        }
    }
}

/// The four clock reads of `transition_calc_time()`, in order.
fn clock_reads(locked: bool) -> Vec<Ev> {
    vec![
        Ev::R(F::Mode, locked),
        Ev::R(F::ManualVal, locked),
        Ev::R(F::StartTime, locked),
        Ev::R(F::Duration, locked),
    ]
}

/// The writes of a stop after slot A's child is dealt with.
fn shift(locked: bool) -> Vec<Ev> {
    vec![
        Ev::W(F::Active(SOURCE_A), locked),
        Ev::W(F::Active(SOURCE_B), locked),
        Ev::R(F::Source(SOURCE_B), locked),
        Ev::W(F::Source(SOURCE_A), locked),
        Ev::W(F::Source(SOURCE_B), locked),
    ]
}

#[test]
fn constants_are_the_cs() {
    assert_eq!(GET_TIME_LOG_NAME, c"obs_transition_get_time");
    assert_eq!(FORCE_STOP_LOG_NAME, c"obs_transition_force_stop");
    assert_eq!(
        STOP_SIGNALS,
        (c"source_transition_stop", c"transition_stop")
    );
    assert_eq!((SOURCE_A, SOURCE_B), (0, 1));
}

#[test]
fn calc_time_follows_the_clock_rule_in_auto_mode() {
    // (start, duration, ts): before, at, in, at the end of and past a
    // start, and a duration of 0.
    let cases = [
        (1_000, 4_000, 0),
        (1_000, 4_000, 999),
        (1_000, 4_000, 1_000),
        (1_000, 4_000, 1_001),
        (1_000, 4_000, 2_000),
        (1_000, 4_000, 4_999),
        (1_000, 4_000, 5_000),
        (1_000, 4_000, u64::MAX),
        (1_000, 0, 1_001),
        (0, 0, 0),
        (u64::MAX, 1, u64::MAX),
    ];
    for (start_time, duration, ts) in cases {
        let tr = Tr::new().with(|s| {
            s.f.start_time = start_time;
            s.f.duration = duration;
        });
        let want = transition_clock::time(tr.clock(), ts);
        let t = calc_time(&tr, ts);
        assert_eq!(t.to_bits(), want.to_bits(), "{start_time} {duration} {ts}");
        assert_eq!(tr.events(), clock_reads(false));
    }
    let tr = Tr::new();
    assert_eq!(calc_time(&tr, 2_000).to_bits(), 0.25f32.to_bits());
    assert_eq!(calc_time(&tr, 900).to_bits(), 0.0f32.to_bits());
    assert_eq!(calc_time(&tr, 5_000).to_bits(), 1.0f32.to_bits());
}

#[test]
fn calc_time_is_the_manual_value_in_manual_mode() {
    for manual_val in [0.0, 0.5, 1.0, -1.0, 2.0, f32::NAN, f32::INFINITY] {
        let tr = Tr::new().with(|s| {
            s.f.mode = MANUAL;
            s.f.manual_val = manual_val;
        });
        for ts in [0, 1_000, 3_000, u64::MAX] {
            assert_eq!(calc_time(&tr, ts).to_bits(), manual_val.to_bits());
            assert_eq!(tr.events(), clock_reads(false));
        }
    }
}

#[test]
fn calc_time_treats_modes_off_the_enum_as_auto() {
    for mode in [-1, 2, 3, i32::MIN, i32::MAX] {
        let tr = Tr::new().with(|s| s.f.mode = mode);
        assert_eq!(calc_time(&tr, 2_000).to_bits(), 0.25f32.to_bits(), "{mode}");
        assert_eq!(tr.events(), clock_reads(false));
    }
}

#[test]
fn calc_time_reads_under_the_callers_lock() {
    let tr = Tr::new();
    tr.lock();
    tr.events();
    calc_time(&tr, 2_000);
    assert_eq!(tr.events(), clock_reads(true));
    tr.unlock();
}

#[test]
fn get_video_time_reads_the_frame_time_first() {
    let tr = Tr::new().with(|s| s.video_time = 3_000);
    tr.lock();
    tr.events();
    assert_eq!(get_video_time(&tr).to_bits(), 0.5f32.to_bits());
    let mut want = vec![Ev::VideoTime(true)];
    want.extend(clock_reads(true));
    assert_eq!(tr.events(), want);
    tr.unlock();

    let tr = Tr::new().with(|s| {
        s.video_time = 0;
        s.f.mode = MANUAL;
        s.f.manual_val = 0.75;
    });
    assert_eq!(get_video_time(&tr).to_bits(), 0.75f32.to_bits());
}

#[test]
fn get_time_reads_the_video_time_under_the_lock() {
    let tr = Tr::new().with(|s| s.video_time = 2_000);
    valid_calls();
    assert_eq!(get_time(Some(&tr)).to_bits(), 0.25f32.to_bits());
    assert_eq!(valid_calls(), vec![(GET_TIME_LOG_NAME, true)]);
    let mut want = vec![Ev::Lock, Ev::VideoTime(true)];
    want.extend(clock_reads(true));
    want.push(Ev::Unlock);
    assert_eq!(tr.events(), want);
    assert!(!tr.locked());
}

#[test]
fn get_time_in_each_mode_and_phase() {
    // Never started (no duration): 1. Early in a long start: just above 0.
    // Past a short start's end: 1, and still 1 while left running.
    let idle = Tr::new().with(|s| {
        s.f.start_time = 0;
        s.f.duration = 0;
        s.video_time = 9_000;
    });
    assert_eq!(get_time(Some(&idle)).to_bits(), 1.0f32.to_bits());
    let early = Tr::new().with(|s| {
        s.f.start_time = 1_000;
        s.f.duration = 1_000_000_000;
        s.video_time = 1_001;
    });
    let t = get_time(Some(&early));
    assert!(t > 0.0 && t < 0.01, "{t}");
    let past = Tr::new().with(|s| s.video_time = 9_000);
    assert_eq!(get_time(Some(&past)).to_bits(), 1.0f32.to_bits());
    assert_eq!(get_time(Some(&past)).to_bits(), 1.0f32.to_bits());
    // Manual: the manual value, whatever the clock.
    let manual = Tr::new().with(|s| {
        s.f.mode = MANUAL;
        s.f.manual_val = 0.0;
    });
    assert_eq!(get_time(Some(&manual)).to_bits(), 0.0f32.to_bits());
    manual.st.borrow_mut().f.manual_val = 0.4;
    assert_eq!(get_time(Some(&manual)).to_bits(), 0.4f32.to_bits());
}

#[test]
fn get_time_of_no_transition_is_zero() {
    valid_calls();
    assert_eq!(get_time::<Tr>(None).to_bits(), 0.0f32.to_bits());
    assert_eq!(valid_calls(), vec![(GET_TIME_LOG_NAME, false)]);
}

#[test]
fn get_time_of_a_source_that_is_no_transition_is_zero() {
    // Its fields would give 1 (past the end) or the manual value.
    for (mode, video_time) in [(AUTO, 9_000), (MANUAL, 0)] {
        let src = Tr::new().with(|s| {
            s.is_transition = false;
            s.f.mode = mode;
            s.f.manual_val = 0.5;
            s.video_time = video_time;
        });
        valid_calls();
        assert_eq!(get_time(Some(&src)).to_bits(), 0.0f32.to_bits());
        assert_eq!(valid_calls(), vec![(GET_TIME_LOG_NAME, true)]);
        assert_eq!(src.events(), vec![]);
    }
}

#[test]
fn stop_with_both_slots_empty() {
    let tr = Tr::new();
    stop(&tr);
    let mut want = vec![Ev::R(F::Source(SOURCE_A), false), Ev::Release(None)];
    want.extend(shift(false));
    assert_eq!(tr.events(), want);
    let f = tr.fields();
    assert_eq!(f.sources, [None, None]);
    assert_eq!(f.active, [true, false]);
}

#[test]
fn stop_removes_and_releases_an_active_slot_a() {
    let tr = Tr::new();
    tr.put(SOURCE_A, A, true);
    tr.put(SOURCE_B, B, true);
    stop(&tr);
    let mut want = vec![
        Ev::R(F::Source(SOURCE_A), false),
        Ev::R(F::Active(SOURCE_A), false),
        Ev::Remove(A),
        Ev::Release(Some(A)),
    ];
    want.extend(shift(false));
    assert_eq!(tr.events(), want);
    let f = tr.fields();
    assert_eq!(f.sources, [Some(B), None]);
    assert_eq!(f.active, [true, false]);
    assert_eq!((tr.refs(A), tr.link(A)), (1, 0));
    // Slot B's reference and link move with it.
    assert_eq!((tr.refs(B), tr.link(B)), (2, 1));
}

#[test]
fn stop_only_releases_an_inactive_slot_a() {
    let tr = Tr::new();
    tr.put(SOURCE_A, A, false);
    tr.put(SOURCE_B, B, false);
    stop(&tr);
    let mut want = vec![
        Ev::R(F::Source(SOURCE_A), false),
        Ev::R(F::Active(SOURCE_A), false),
        Ev::Release(Some(A)),
    ];
    want.extend(shift(false));
    assert_eq!(tr.events(), want);
    let f = tr.fields();
    assert_eq!(f.sources, [Some(B), None]);
    // Slot A is marked active though nothing made B an active child.
    assert_eq!(f.active, [true, false]);
    assert_eq!((tr.refs(A), tr.link(A)), (1, 0));
    assert_eq!((tr.refs(B), tr.link(B)), (2, 0));
}

#[test]
fn stop_with_only_slot_a() {
    let tr = Tr::new();
    tr.put(SOURCE_A, A, true);
    stop(&tr);
    let mut want = vec![
        Ev::R(F::Source(SOURCE_A), false),
        Ev::R(F::Active(SOURCE_A), false),
        Ev::Remove(A),
        Ev::Release(Some(A)),
    ];
    want.extend(shift(false));
    assert_eq!(tr.events(), want);
    assert_eq!(tr.fields().sources, [None, None]);
    assert_eq!(tr.fields().active, [true, false]);
    assert_eq!((tr.refs(A), tr.link(A)), (1, 0));
}

#[test]
fn stop_with_only_slot_b_reads_no_active_flag() {
    let tr = Tr::new();
    tr.put(SOURCE_B, B, true);
    // An active flag on the empty slot A is not read.
    tr.st.borrow_mut().f.active[SOURCE_A] = true;
    stop(&tr);
    let mut want = vec![Ev::R(F::Source(SOURCE_A), false), Ev::Release(None)];
    want.extend(shift(false));
    assert_eq!(tr.events(), want);
    assert_eq!(tr.fields().sources, [Some(B), None]);
    assert_eq!(tr.fields().active, [true, false]);
    assert_eq!((tr.refs(B), tr.link(B)), (2, 1));
}

#[test]
fn stop_runs_under_the_callers_lock_and_takes_none() {
    let tr = Tr::new();
    tr.put(SOURCE_A, A, true);
    tr.put(SOURCE_B, B, false);
    tr.lock();
    tr.events();
    stop(&tr);
    let mut want = vec![
        Ev::R(F::Source(SOURCE_A), true),
        Ev::R(F::Active(SOURCE_A), true),
        Ev::Remove(A),
        Ev::Release(Some(A)),
    ];
    want.extend(shift(true));
    assert_eq!(tr.events(), want);
    tr.unlock();
}

#[test]
fn stop_leaves_the_clock_and_mode() {
    let tr = Tr::new().with(|s| {
        s.f.mode = MANUAL;
        s.f.manual_val = 0.6;
    });
    tr.put(SOURCE_A, A, true);
    tr.put(SOURCE_B, B, true);
    let before = tr.fields();
    stop(&tr);
    let after = tr.fields();
    assert_eq!(
        (
            after.mode,
            after.manual_val,
            after.start_time,
            after.duration
        ),
        (
            before.mode,
            before.manual_val,
            before.start_time,
            before.duration
        )
    );
}

#[test]
fn handle_stop_calls_back_then_signals() {
    let tr = Tr::new();
    handle_stop(&tr);
    assert_eq!(
        tr.events(),
        vec![
            Ev::CallStop(true),
            Ev::Signal(STOP_SIGNALS.0, STOP_SIGNALS.1)
        ]
    );
}

#[test]
fn handle_stop_without_a_callback_only_signals() {
    let tr = Tr::new().with(|s| s.has_stop = false);
    handle_stop(&tr);
    assert_eq!(
        tr.events(),
        vec![
            Ev::CallStop(false),
            Ev::Signal(STOP_SIGNALS.0, STOP_SIGNALS.1)
        ]
    );
}

#[test]
fn force_stop_checks_then_handles_the_stop() {
    let tr = Tr::new();
    tr.put(SOURCE_A, A, true);
    tr.put(SOURCE_B, B, true);
    let before = tr.fields();
    valid_calls();
    force_stop(Some(&tr));
    assert_eq!(valid_calls(), vec![(FORCE_STOP_LOG_NAME, true)]);
    // No lock, no field: the slots and running state stay.
    assert_eq!(
        tr.events(),
        vec![
            Ev::CallStop(true),
            Ev::Signal(STOP_SIGNALS.0, STOP_SIGNALS.1)
        ]
    );
    assert_eq!(tr.fields(), before);
    assert_eq!(
        (tr.refs(A), tr.link(A), tr.refs(B), tr.link(B)),
        (2, 1, 2, 1)
    );

    // Each force stop calls back and signals again, idle or not.
    force_stop(Some(&tr));
    assert_eq!(tr.events().len(), 2);
    let bare = Tr::new().with(|s| s.has_stop = false);
    force_stop(Some(&bare));
    assert_eq!(
        bare.events(),
        vec![
            Ev::CallStop(false),
            Ev::Signal(STOP_SIGNALS.0, STOP_SIGNALS.1)
        ]
    );
}

#[test]
fn force_stop_of_no_transition_does_nothing() {
    valid_calls();
    force_stop::<Tr>(None);
    assert_eq!(valid_calls(), vec![(FORCE_STOP_LOG_NAME, false)]);
}

#[test]
fn force_stop_of_a_source_that_is_no_transition_does_nothing() {
    let src = Tr::new().with(|s| s.is_transition = false);
    valid_calls();
    force_stop(Some(&src));
    assert_eq!(valid_calls(), vec![(FORCE_STOP_LOG_NAME, true)]);
    assert_eq!(src.events(), vec![]);
}

/// A slot's starting state: empty, or a child, active or not.
fn slot() -> impl Strategy<Value = Option<bool>> {
    prop_oneof![Just(None), any::<bool>().prop_map(Some)]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(256)))]

    #[test]
    fn time_is_the_clock_rule(
        mode in prop_oneof![Just(AUTO), Just(MANUAL), any::<i32>()],
        manual_val in any::<f32>(),
        start_time in any::<u64>(),
        duration in prop_oneof![Just(0u64), 1u64..10_000, any::<u64>()],
        video_time in any::<u64>(),
        ts in any::<u64>(),
    ) {
        let tr = Tr::new().with(|s| {
            s.f.mode = mode;
            s.f.manual_val = manual_val;
            s.f.start_time = start_time;
            s.f.duration = duration;
            s.video_time = video_time;
        });
        let clock = tr.clock();
        prop_assert_eq!(
            calc_time(&tr, ts).to_bits(),
            transition_clock::time(clock, ts).to_bits()
        );
        prop_assert_eq!(
            get_time(Some(&tr)).to_bits(),
            transition_clock::time(clock, video_time).to_bits()
        );
        prop_assert!(!tr.locked());
    }

    #[test]
    fn stop_shifts_the_slots(a in slot(), b in slot()) {
        let tr = Tr::new();
        if let Some(active) = a {
            tr.put(SOURCE_A, A, active);
        }
        if let Some(active) = b {
            tr.put(SOURCE_B, B, active);
        }
        let (refs_a, link_a, refs_b, link_b) = (tr.refs(A), tr.link(A), tr.refs(B), tr.link(B));
        stop(&tr);
        let f = tr.fields();
        prop_assert_eq!(f.sources, [b.map(|_| B), None]);
        prop_assert_eq!(f.active, [true, false]);
        prop_assert_eq!(tr.refs(A), refs_a - i64::from(a.is_some()));
        prop_assert_eq!(tr.link(A), link_a - i64::from(a == Some(true)));
        prop_assert_eq!((tr.refs(B), tr.link(B)), (refs_b, link_b));
        prop_assert!(!tr.events().iter().any(|e| matches!(e, Ev::Lock | Ev::Unlock)));
    }
}
