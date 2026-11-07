//! Tier 1 tests for `obs_source::transition_access`, the safe core of
//! `libobs/obs-source-transition-access.c.h`.
//!
//! The functions run on a fake transition with its two slots, running
//! flags, mode and manual fields, over a fake reference count per child.
//! The fake records each call in order (the check, the lock and unlock,
//! each field read and write, `obs_source_get_ref()`,
//! `obs_transition_set()`), with whether the lock was held, and panics on a
//! lock taken twice or an unlock without the lock.
//!
//! The `c_*` cases follow `test_source_transition_access.c`: which slot
//! each getter returns, the reference it takes, NULL and a source that is
//! not a transition, the torque and clamp, and when the manual time ends
//! the transition on slot A. The `order_*` cases pin every call of each
//! function. The `check_*` cases pin the intentional difference: both
//! setters return first unless `transition_valid()` holds, and log a NULL
//! under their own names, where the C checks nothing. A proptest compares
//! every call with a copy of the C.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::CStr;

use obs_c_oracle::parity::cases;
use obs_source::transition_access::{
    GET_ACTIVE_SOURCE_LOG_NAME, GET_SOURCE_LOG_NAME, RESET_SLOT, SET_MANUAL_TIME_LOG_NAME,
    SET_MANUAL_TORQUE_LOG_NAME, Transition, get_active_source, get_source, set_manual_time,
    set_manual_torque,
};
use proptest::prelude::*;

/// A child of the fake transition.
type Id = usize;

/// `OBS_TRANSITION_MODE_AUTO` and `_MANUAL`.
const AUTO: i32 = 0;
const MANUAL: i32 = 1;

thread_local! {
    /// The names `transition_valid()` was asked about a NULL source with.
    static NULL_CHECKS: RefCell<Vec<&'static CStr>> = const { RefCell::new(Vec::new()) };
}

fn take_null_checks() -> Vec<&'static CStr> {
    NULL_CHECKS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

/// One call, with whether `transition_mutex` was held where that matters.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Ev {
    Valid(&'static CStr),
    Lock,
    Unlock,
    Source(usize, bool),
    Video(bool),
    Audio(bool),
    Mode(bool),
    /// The floats as bits, so NaN compares.
    Torque(u32, bool),
    Clamp(u32, bool),
    Target(u32, bool),
    GetRef(Option<Id>, bool),
    Set(Option<Id>, bool),
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Fields {
    sources: [Option<Id>; 2],
    video: bool,
    audio: bool,
    mode: i32,
    torque: f32,
    clamp: f32,
    target: f32,
}

impl Fields {
    fn new(a: Option<Id>, b: Option<Id>) -> Self {
        Self {
            sources: [a, b],
            video: false,
            audio: false,
            mode: AUTO,
            torque: 0.5,
            clamp: 0.25,
            target: 0.125,
        }
    }
}

struct Tr {
    is_transition: bool,
    f: RefCell<Fields>,
    locked: RefCell<bool>,
    /// References held on each child; a child at 0 is going away.
    refs: RefCell<BTreeMap<Id, i64>>,
    events: RefCell<Vec<Ev>>,
}

impl Tr {
    fn new(f: Fields) -> Self {
        let mut refs = BTreeMap::new();
        for id in f.sources.into_iter().flatten() {
            *refs.entry(id).or_insert(0) += 1;
        }
        Self {
            is_transition: true,
            f: RefCell::new(f),
            locked: RefCell::new(false),
            refs: RefCell::new(refs),
            events: RefCell::new(Vec::new()),
        }
    }
    fn not_transition(f: Fields) -> Self {
        Self {
            is_transition: false,
            ..Self::new(f)
        }
    }
    fn ev(&self, ev: Ev) {
        self.events.borrow_mut().push(ev);
    }
    fn locked(&self) -> bool {
        *self.locked.borrow()
    }
    fn refs(&self, id: Id) -> i64 {
        self.refs.borrow().get(&id).copied().unwrap_or(0)
    }
    fn events(&self) -> Vec<Ev> {
        self.events.borrow().clone()
    }
    fn fields(&self) -> Fields {
        *self.f.borrow()
    }
}

impl Transition for Tr {
    type Child = Id;

    fn valid(source: Option<&Self>, func: &'static CStr) -> bool {
        match source {
            None => {
                NULL_CHECKS.with(|c| c.borrow_mut().push(func));
                false
            }
            Some(t) => {
                t.ev(Ev::Valid(func));
                t.is_transition
            }
        }
    }
    fn source(&self, idx: usize) -> Option<Id> {
        self.ev(Ev::Source(idx, self.locked()));
        self.f.borrow().sources.get(idx).copied().flatten()
    }
    fn transitioning_video(&self) -> bool {
        self.ev(Ev::Video(self.locked()));
        self.f.borrow().video
    }
    fn transitioning_audio(&self) -> bool {
        self.ev(Ev::Audio(self.locked()));
        self.f.borrow().audio
    }
    fn mode(&self) -> i32 {
        self.ev(Ev::Mode(self.locked()));
        self.f.borrow().mode
    }
    fn set_manual_torque(&self, value: f32) {
        self.ev(Ev::Torque(value.to_bits(), self.locked()));
        self.f.borrow_mut().torque = value;
    }
    fn set_manual_clamp(&self, value: f32) {
        self.ev(Ev::Clamp(value.to_bits(), self.locked()));
        self.f.borrow_mut().clamp = value;
    }
    fn set_manual_target(&self, value: f32) {
        self.ev(Ev::Target(value.to_bits(), self.locked()));
        self.f.borrow_mut().target = value;
    }
    fn lock(&self) {
        assert!(!self.locked(), "transition_mutex taken twice");
        self.ev(Ev::Lock);
        *self.locked.borrow_mut() = true;
    }
    fn unlock(&self) {
        assert!(self.locked(), "transition_mutex unlocked without the lock");
        self.ev(Ev::Unlock);
        *self.locked.borrow_mut() = false;
    }
    fn get_ref(&self, child: Option<Id>) -> Option<Id> {
        self.ev(Ev::GetRef(child, self.locked()));
        let id = child?;
        let mut refs = self.refs.borrow_mut();
        let r = refs.entry(id).or_insert(0);
        if *r > 0 {
            *r += 1;
            Some(id)
        } else {
            None
        }
    }
    fn set(&self, source: Option<Id>) {
        self.ev(Ev::Set(source, self.locked()));
    }
}

const A: Id = 10;
const B: Id = 11;

// ---------------------------------------------------------------------------
// The C test's cases

/// `test_get_source`: slot A, slot B, an empty slot; each a new reference.
#[test]
fn c_get_source() {
    let tr = Tr::new(Fields::new(Some(A), None));
    assert_eq!(get_source(Some(&tr), 1), None);
    assert_eq!(get_source(Some(&tr), 0), Some(A));
    assert_eq!(tr.refs(A), 2);
    tr.f.borrow_mut().sources[1] = Some(B);
    tr.refs.borrow_mut().insert(B, 1);
    assert_eq!(get_source(Some(&tr), 1), Some(B));
    assert_eq!(tr.refs(B), 2);
    assert_eq!(get_source(Some(&tr), 0), Some(A));
    assert_eq!(tr.refs(A), 3);
    assert!(!tr.locked());
}

/// `test_get_active_source`: B while the transition runs (video, audio or
/// both), A otherwise.
#[test]
fn c_get_active_source() {
    for (video, audio, want) in [
        (false, false, A),
        (true, false, B),
        (false, true, B),
        (true, true, B),
    ] {
        let mut f = Fields::new(Some(A), Some(B));
        f.video = video;
        f.audio = audio;
        let tr = Tr::new(f);
        assert_eq!(get_active_source(Some(&tr)), Some(want), "{video} {audio}");
        assert_eq!(tr.refs(want), 2);
        assert!(!tr.locked());
    }
    let mut f = Fields::new(Some(A), None);
    f.video = true;
    assert_eq!(get_active_source(Some(&Tr::new(f))), None);
    assert_eq!(
        get_active_source(Some(&Tr::new(Fields::new(None, Some(B))))),
        None
    );
}

/// `test_get_null_and_not_transition`: NULL gives no source and logs the
/// getter's name, `obs_transition_get_source`'s for both (characterized,
/// not endorsed); a source that is not a transition gives no source, and
/// neither reads a slot.
#[test]
fn c_get_null_and_not_transition() {
    take_null_checks();
    assert_eq!(get_source::<Tr>(None, 0), None);
    assert_eq!(take_null_checks(), [c"obs_transition_get_source"]);
    assert_eq!(get_active_source::<Tr>(None), None);
    assert_eq!(take_null_checks(), [c"obs_transition_get_source"]);
    assert_eq!(GET_SOURCE_LOG_NAME, c"obs_transition_get_source");
    assert_eq!(GET_ACTIVE_SOURCE_LOG_NAME, c"obs_transition_get_source");

    let input = Tr::not_transition(Fields::new(Some(A), Some(B)));
    assert_eq!(get_source(Some(&input), 0), None);
    assert_eq!(get_source(Some(&input), 1), None);
    assert_eq!(get_active_source(Some(&input)), None);
    assert_eq!(
        input.events(),
        [
            Ev::Valid(GET_SOURCE_LOG_NAME),
            Ev::Valid(GET_SOURCE_LOG_NAME),
            Ev::Valid(GET_ACTIVE_SOURCE_LOG_NAME),
        ]
    );
    assert_eq!(input.refs(A), 1);
    assert_eq!(input.refs(B), 1);
}

/// `test_manual_torque`: each call replaces both the torque and the clamp.
#[test]
fn c_manual_torque() {
    let tr = Tr::new(Fields::new(Some(A), Some(B)));
    for (torque, clamp) in [(0.0, 0.0), (0.1, 1000.0), (0.1, 0.0), (-3.0, f32::NAN)] {
        set_manual_torque(Some(&tr), torque, clamp);
        let f = tr.fields();
        assert_eq!(f.torque.to_bits(), f32::to_bits(torque));
        assert_eq!(f.clamp.to_bits(), f32::to_bits(clamp));
    }
    assert!(!tr.locked());
}

/// `test_manual_time`: the target is always set; in manual mode a time of
/// 0 or -0 ends the transition on slot A; no other time (NaN included), and
/// not in auto mode.
#[test]
fn c_manual_time() {
    for mode in [AUTO, MANUAL] {
        for t in [
            0.0f32,
            -0.0,
            0.25,
            0.5,
            1.0,
            -1.0,
            f32::NAN,
            f32::MIN_POSITIVE,
        ] {
            let mut f = Fields::new(Some(A), Some(B));
            f.mode = mode;
            let tr = Tr::new(f);
            set_manual_time(Some(&tr), t);
            assert_eq!(tr.fields().target.to_bits(), t.to_bits());
            let sets: Vec<Ev> = tr
                .events()
                .into_iter()
                .filter(|e| matches!(e, Ev::Set(..)))
                .collect();
            if mode == MANUAL && t == 0.0 {
                assert_eq!(sets, [Ev::Set(Some(A), false)], "{mode} {t}");
            } else {
                assert!(sets.is_empty(), "{mode} {t}");
            }
            assert!(!tr.locked());
        }
    }
}

/// The reset hands `obs_transition_set()` whatever slot A holds, empty
/// included.
#[test]
fn c_manual_time_empty_slot_a() {
    let mut f = Fields::new(None, Some(B));
    f.mode = MANUAL;
    let tr = Tr::new(f);
    set_manual_time(Some(&tr), 0.0);
    assert_eq!(tr.events().last(), Some(&Ev::Set(None, false)));
}

// ---------------------------------------------------------------------------
// Every call, in order

#[test]
fn order_get_source() {
    let tr = Tr::new(Fields::new(Some(A), Some(B)));
    assert_eq!(get_source(Some(&tr), 1), Some(B));
    assert_eq!(
        tr.events(),
        [
            Ev::Valid(GET_SOURCE_LOG_NAME),
            Ev::Lock,
            Ev::Source(1, true),
            Ev::GetRef(Some(B), true),
            Ev::Unlock,
        ]
    );
}

/// The caller's index goes to the seam as it is, out of range included.
#[test]
fn order_get_source_index_unchecked() {
    let tr = Tr::new(Fields::new(Some(A), Some(B)));
    for idx in [2, 7, usize::MAX] {
        tr.events.borrow_mut().clear();
        assert_eq!(get_source(Some(&tr), idx), None);
        assert_eq!(
            tr.events(),
            [
                Ev::Valid(GET_SOURCE_LOG_NAME),
                Ev::Lock,
                Ev::Source(idx, true),
                Ev::GetRef(None, true),
                Ev::Unlock,
            ]
        );
    }
}

/// `transitioning_audio` first; `transitioning_video` only when it is
/// false (the C's `||`).
#[test]
fn order_get_active_source() {
    for (audio, video) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut f = Fields::new(Some(A), Some(B));
        f.audio = audio;
        f.video = video;
        let tr = Tr::new(f);
        let idx = usize::from(audio || video);
        let child = [A, B][idx];
        assert_eq!(get_active_source(Some(&tr)), Some(child));
        let mut want = vec![
            Ev::Valid(GET_ACTIVE_SOURCE_LOG_NAME),
            Ev::Lock,
            Ev::Audio(true),
        ];
        if !audio {
            want.push(Ev::Video(true));
        }
        want.extend([
            Ev::Source(idx, true),
            Ev::GetRef(Some(child), true),
            Ev::Unlock,
        ]);
        assert_eq!(tr.events(), want, "{audio} {video}");
    }
}

#[test]
fn order_set_manual_torque() {
    let tr = Tr::new(Fields::new(Some(A), Some(B)));
    set_manual_torque(Some(&tr), 0.75, 2.0);
    assert_eq!(
        tr.events(),
        [
            Ev::Valid(SET_MANUAL_TORQUE_LOG_NAME),
            Ev::Lock,
            Ev::Torque(0.75f32.to_bits(), true),
            Ev::Clamp(2.0f32.to_bits(), true),
            Ev::Unlock,
        ]
    );
}

/// The target and the mode under the lock; slot A read after the unlock,
/// only for the reset.
#[test]
fn order_set_manual_time() {
    for (mode, t, reset) in [
        (MANUAL, 0.0f32, true),
        (MANUAL, 0.5, false),
        (AUTO, 0.0, false),
    ] {
        let mut f = Fields::new(Some(A), Some(B));
        f.mode = mode;
        let tr = Tr::new(f);
        set_manual_time(Some(&tr), t);
        let mut want = vec![
            Ev::Valid(SET_MANUAL_TIME_LOG_NAME),
            Ev::Lock,
            Ev::Target(t.to_bits(), true),
            Ev::Mode(true),
            Ev::Unlock,
        ];
        if reset {
            want.extend([Ev::Source(RESET_SLOT, false), Ev::Set(Some(A), false)]);
        }
        assert_eq!(tr.events(), want, "{mode} {t}");
    }
    assert_eq!(RESET_SLOT, 0);
}

/// A child going away (`obs_source_get_ref()` gives NULL): no source, and
/// the lock is released.
#[test]
fn order_dying_child() {
    let tr = Tr::new(Fields::new(Some(A), Some(B)));
    tr.refs.borrow_mut().insert(A, 0);
    assert_eq!(get_source(Some(&tr), 0), None);
    assert_eq!(get_active_source(Some(&tr)), None);
    assert!(!tr.locked());
    assert_eq!(get_source(Some(&tr), 1), Some(B));
}

// ---------------------------------------------------------------------------
// The intentional difference: both setters check their argument

/// NULL: no crash, and the setter's own name logged.
#[test]
fn check_setters_null() {
    take_null_checks();
    set_manual_torque::<Tr>(None, 1.0, 1.0);
    assert_eq!(take_null_checks(), [c"obs_transition_set_manual_torque"]);
    set_manual_time::<Tr>(None, 0.0);
    assert_eq!(take_null_checks(), [c"obs_transition_set_manual_time"]);
    assert_eq!(
        SET_MANUAL_TORQUE_LOG_NAME,
        c"obs_transition_set_manual_torque"
    );
    assert_eq!(SET_MANUAL_TIME_LOG_NAME, c"obs_transition_set_manual_time");
}

/// A source that is not a transition: nothing written, no lock, no reset.
#[test]
fn check_setters_not_transition() {
    let mut f = Fields::new(Some(A), Some(B));
    f.mode = MANUAL;
    let input = Tr::not_transition(f);
    set_manual_torque(Some(&input), 9.0, 9.0);
    set_manual_time(Some(&input), 0.0);
    assert_eq!(input.fields(), f);
    assert_eq!(
        input.events(),
        [
            Ev::Valid(SET_MANUAL_TORQUE_LOG_NAME),
            Ev::Valid(SET_MANUAL_TIME_LOG_NAME),
        ]
    );
}

// ---------------------------------------------------------------------------
// Against a copy of the C

#[derive(Debug, Clone, Copy)]
enum Op {
    Get(usize),
    Active,
    Torque(f32, f32),
    Time(f32),
}

/// The calls the C makes on a valid transition (NULL and a source that is
/// not a transition are pinned above), with the setters' check added.
fn c_model(f: &Fields, op: Op) -> Vec<Ev> {
    match op {
        Op::Get(idx) => vec![
            Ev::Valid(c"obs_transition_get_source"),
            Ev::Lock,
            Ev::Source(idx, true),
            Ev::GetRef(f.sources.get(idx).copied().flatten(), true),
            Ev::Unlock,
        ],
        Op::Active => {
            let mut v = vec![
                Ev::Valid(c"obs_transition_get_source"),
                Ev::Lock,
                Ev::Audio(true),
            ];
            if !f.audio {
                v.push(Ev::Video(true));
            }
            let idx = usize::from(f.audio || f.video);
            v.extend([
                Ev::Source(idx, true),
                Ev::GetRef(f.sources[idx], true),
                Ev::Unlock,
            ]);
            v
        }
        Op::Torque(torque, clamp) => vec![
            Ev::Valid(SET_MANUAL_TORQUE_LOG_NAME),
            Ev::Lock,
            Ev::Torque(torque.to_bits(), true),
            Ev::Clamp(clamp.to_bits(), true),
            Ev::Unlock,
        ],
        Op::Time(t) => {
            let mut v = vec![
                Ev::Valid(SET_MANUAL_TIME_LOG_NAME),
                Ev::Lock,
                Ev::Target(t.to_bits(), true),
                Ev::Mode(true),
                Ev::Unlock,
            ];
            if f.mode == MANUAL && t == 0.0 {
                v.extend([Ev::Source(0, false), Ev::Set(f.sources[0], false)]);
            }
            v
        }
    }
}

fn gen_child() -> impl Strategy<Value = Option<Id>> {
    proptest::option::of(A..=B + 1)
}

fn gen_time() -> impl Strategy<Value = f32> {
    prop_oneof![
        Just(0.0f32),
        Just(-0.0f32),
        Just(f32::NAN),
        Just(1.0f32),
        -2.0f32..2.0,
    ]
}

fn gen_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0usize..4).prop_map(Op::Get),
        Just(Op::Active),
        (gen_time(), gen_time()).prop_map(|(a, b)| Op::Torque(a, b)),
        gen_time().prop_map(Op::Time),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(256)))]

    /// Any slots, flags, mode, child going away and call: the C's calls in
    /// the C's order, and the source, references and fields after.
    #[test]
    fn any_transition_and_call(
        a in gen_child(),
        b in gen_child(),
        video in any::<bool>(),
        audio in any::<bool>(),
        mode in 0i32..3,
        dying in proptest::option::of(A..=B + 1),
        op in gen_op(),
    ) {
        let mut f = Fields::new(a, b);
        f.video = video;
        f.audio = audio;
        f.mode = mode;
        let tr = Tr::new(f);
        if let Some(d) = dying {
            tr.refs.borrow_mut().insert(d, 0);
        }
        let refs_before = tr.refs.borrow().clone();
        let got = match op {
            Op::Get(idx) => get_source(Some(&tr), idx),
            Op::Active => get_active_source(Some(&tr)),
            Op::Torque(torque, clamp) => {
                set_manual_torque(Some(&tr), torque, clamp);
                None
            }
            Op::Time(t) => {
                set_manual_time(Some(&tr), t);
                None
            }
        };
        prop_assert_eq!(tr.events(), c_model(&f, op));
        prop_assert!(!tr.locked());

        // The reference the getters took, and the fields the setters wrote.
        let mut refs_after = refs_before;
        let mut fields_after = f;
        let read = match op {
            Op::Get(idx) => f.sources.get(idx).copied().flatten(),
            Op::Active => f.sources[usize::from(f.audio || f.video)],
            Op::Torque(torque, clamp) => {
                fields_after.torque = torque;
                fields_after.clamp = clamp;
                None
            }
            Op::Time(t) => {
                fields_after.target = t;
                None
            }
        };
        let mut want_got = None;
        if let Some(id) = read {
            let r = refs_after.entry(id).or_insert(0);
            if *r > 0 {
                *r += 1;
                want_got = Some(id);
            }
        }
        prop_assert_eq!(got, want_got);
        prop_assert_eq!(&*tr.refs.borrow(), &refs_after);
        let now = tr.fields();
        prop_assert_eq!(now.sources, fields_after.sources);
        prop_assert_eq!(now.mode, fields_after.mode);
        prop_assert_eq!(now.torque.to_bits(), fields_after.torque.to_bits());
        prop_assert_eq!(now.clamp.to_bits(), fields_after.clamp.to_bits());
        prop_assert_eq!(now.target.to_bits(), fields_after.target.to_bits());
    }
}
