//! Tier 1 tests for `obs_source::transition_slots`, the safe core of
//! `libobs/obs-source-transition-slots.c.h`.
//!
//! The functions run on a fake source graph: every source (child or
//! transition) has a reference count and a set of active-child links, and a
//! transition has its slots, flags, clock fields, render targets and both
//! locks. The fake records each call into the rest of libobs (the check, the
//! locks, `obs_source_get_ref()`, `obs_source_release()`, the active-child
//! add and remove, the recursion warning, the enum callback) in order, and
//! panics when a field is read or written without the lock the C holds
//! there (`transition_mutex` for the slots, flags and clock,
//! `transition_tex_mutex` for the render targets) or a lock is taken twice.
//!
//! The `c_*` cases follow `test_source_transition_slots.c` (same names and
//! the same steps), with what the real libobs shows through weak references
//! and `obs_transition_get_source()` read off the fake. The other cases pin
//! the hand-over plan: which children are released, added or kept, what the
//! destination stores when the add fails or the child is going away, which
//! clock fields move, and the order of every lock and call.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::CStr;
use std::rc::Rc;

use obs_c_oracle::parity::cases;
use obs_source::transition_slots::{
    CLEAR_LOG_NAME, RECURSION_WARNING, SLOTS, Transition, clear, copy_source_state, enum_sources,
    swap_begin, swap_end,
};
use proptest::prelude::*;

/// A source of the fake graph.
type Id = usize;

thread_local! {
    /// The names `transition_valid()` was asked about a NULL source with.
    static NULL_CHECKS: RefCell<Vec<&'static CStr>> = const { RefCell::new(Vec::new()) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ev {
    Valid(&'static CStr),
    Lock(Id),
    Unlock(Id),
    LockTex(Id),
    UnlockTex(Id),
    GetRef(Option<Id>),
    Release(Option<Id>),
    Add(Id, Id, bool),
    Remove(Id, Id),
    Warn(Id, Id),
    Callback(Id, Id),
}

#[derive(Debug, Default)]
struct World {
    /// References held on each source.
    refs: BTreeMap<Id, i64>,
    /// Active-child links, parent to child, with their count.
    links: BTreeMap<(Id, Id), i64>,
    /// (child, parent): the child's tree contains the parent, so adding it
    /// to the parent as an active child fails.
    contains: Vec<(Id, Id)>,
    events: Vec<Ev>,
}

impl World {
    fn refs(&self, id: Id) -> i64 {
        self.refs.get(&id).copied().unwrap_or(0)
    }
    fn link(&self, parent: Id, child: Id) -> i64 {
        self.links.get(&(parent, child)).copied().unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Fields {
    sources: [Option<Id>; 2],
    active: [bool; 2],
    video: bool,
    audio: bool,
    start_time: u64,
    duration: u64,
    mode: i32,
    manual_val: f32,
    manual_target: f32,
    texrender: [u32; 2],
}

impl Fields {
    fn empty(id: Id) -> Self {
        let tex = u32::try_from(id).unwrap_or(0).wrapping_mul(10);
        Self {
            sources: [None; 2],
            active: [false; 2],
            video: false,
            audio: false,
            start_time: 0,
            duration: 0,
            mode: 0,
            manual_val: 0.0,
            manual_target: 0.0,
            texrender: [tex.wrapping_add(1), tex.wrapping_add(2)],
        }
    }
}

struct Tr {
    id: Id,
    is_transition: bool,
    world: Rc<RefCell<World>>,
    f: RefCell<Fields>,
    locked: RefCell<bool>,
    tex_locked: RefCell<bool>,
}

impl Tr {
    fn new(world: &Rc<RefCell<World>>, id: Id) -> Self {
        world.borrow_mut().refs.insert(id, 1);
        Self {
            id,
            is_transition: true,
            world: Rc::clone(world),
            f: RefCell::new(Fields::empty(id)),
            locked: RefCell::new(false),
            tex_locked: RefCell::new(false),
        }
    }
    fn ev(&self, ev: Ev) {
        self.world.borrow_mut().events.push(ev);
    }
    fn locked(&self) -> bool {
        *self.locked.borrow()
    }
    fn tex_locked(&self) -> bool {
        *self.tex_locked.borrow()
    }
    fn need_lock(&self) {
        assert!(
            self.locked(),
            "transition {} field without its lock",
            self.id
        );
    }
    fn need_tex_lock(&self) {
        assert!(
            self.tex_locked(),
            "transition {} render target without its texture lock",
            self.id
        );
    }
    fn fields(&self) -> Fields {
        *self.f.borrow()
    }
    /// What `obs_transition_set()` and `obs_transition_start()` leave: a
    /// child in `idx`, with the transition's own reference and, when
    /// `active`, its active-child link.
    fn hold(&self, idx: usize, child: Id, active: bool) {
        let mut w = self.world.borrow_mut();
        *w.refs.entry(child).or_insert(0) += 1;
        if active {
            *w.links.entry((self.id, child)).or_insert(0) += 1;
        }
        let mut f = self.f.borrow_mut();
        f.sources[idx] = Some(child);
        f.active[idx] = active;
    }
    /// The events so far, emptied.
    fn events(&self) -> Vec<Ev> {
        std::mem::take(&mut self.world.borrow_mut().events)
    }
}

impl Transition for Tr {
    type Child = Id;
    type Texrender = u32;

    fn same(&self, other: &Self) -> bool {
        self.id == other.id
    }
    fn valid(source: Option<&Self>, func: &'static CStr) -> bool {
        match source {
            None => {
                NULL_CHECKS.with(|n| n.borrow_mut().push(func));
                false
            }
            Some(s) => {
                s.ev(Ev::Valid(func));
                s.is_transition
            }
        }
    }
    fn source(&self, idx: usize) -> Option<Id> {
        self.need_lock();
        self.f.borrow().sources[idx]
    }
    fn set_source(&self, idx: usize, value: Option<Id>) {
        self.need_lock();
        self.f.borrow_mut().sources[idx] = value;
    }
    fn source_active(&self, idx: usize) -> bool {
        self.need_lock();
        self.f.borrow().active[idx]
    }
    fn set_source_active(&self, idx: usize, value: bool) {
        self.need_lock();
        self.f.borrow_mut().active[idx] = value;
    }
    fn transitioning_video(&self) -> bool {
        self.need_lock();
        self.f.borrow().video
    }
    fn set_transitioning_video(&self, value: bool) {
        self.need_lock();
        self.f.borrow_mut().video = value;
    }
    fn transitioning_audio(&self) -> bool {
        self.need_lock();
        self.f.borrow().audio
    }
    fn set_transitioning_audio(&self, value: bool) {
        self.need_lock();
        self.f.borrow_mut().audio = value;
    }
    fn start_time(&self) -> u64 {
        self.need_lock();
        self.f.borrow().start_time
    }
    fn set_start_time(&self, value: u64) {
        self.need_lock();
        self.f.borrow_mut().start_time = value;
    }
    fn duration(&self) -> u64 {
        self.need_lock();
        self.f.borrow().duration
    }
    fn set_duration(&self, value: u64) {
        self.need_lock();
        self.f.borrow_mut().duration = value;
    }
    fn mode(&self) -> i32 {
        self.need_lock();
        self.f.borrow().mode
    }
    fn set_mode(&self, value: i32) {
        self.need_lock();
        self.f.borrow_mut().mode = value;
    }
    fn manual_val(&self) -> f32 {
        self.need_lock();
        self.f.borrow().manual_val
    }
    fn set_manual_val(&self, value: f32) {
        self.need_lock();
        self.f.borrow_mut().manual_val = value;
    }
    fn manual_target(&self) -> f32 {
        self.need_lock();
        self.f.borrow().manual_target
    }
    fn set_manual_target(&self, value: f32) {
        self.need_lock();
        self.f.borrow_mut().manual_target = value;
    }
    fn texrender(&self, idx: usize) -> u32 {
        self.need_tex_lock();
        self.f.borrow().texrender[idx]
    }
    fn set_texrender(&self, idx: usize, value: u32) {
        self.need_tex_lock();
        self.f.borrow_mut().texrender[idx] = value;
    }
    fn lock(&self) {
        assert!(!self.locked(), "transition {} locked twice", self.id);
        *self.locked.borrow_mut() = true;
        self.ev(Ev::Lock(self.id));
    }
    fn unlock(&self) {
        assert!(self.locked(), "transition {} unlocked while free", self.id);
        *self.locked.borrow_mut() = false;
        self.ev(Ev::Unlock(self.id));
    }
    fn lock_textures(&self) {
        assert!(!self.tex_locked(), "textures {} locked twice", self.id);
        *self.tex_locked.borrow_mut() = true;
        self.ev(Ev::LockTex(self.id));
    }
    fn unlock_textures(&self) {
        assert!(
            self.tex_locked(),
            "textures {} unlocked while free",
            self.id
        );
        *self.tex_locked.borrow_mut() = false;
        self.ev(Ev::UnlockTex(self.id));
    }
    fn get_ref(&self, child: Option<Id>) -> Option<Id> {
        self.ev(Ev::GetRef(child));
        let mut w = self.world.borrow_mut();
        let c = child?;
        let refs = w.refs.entry(c).or_insert(0);
        if *refs > 0 {
            *refs += 1;
            Some(c)
        } else {
            None
        }
    }
    fn release(&self, child: Option<Id>) {
        self.ev(Ev::Release(child));
        if let Some(c) = child {
            let mut w = self.world.borrow_mut();
            let refs = w.refs.entry(c).or_insert(0);
            assert!(*refs > 0, "source {c} released past its last reference");
            *refs -= 1;
        }
    }
    fn add_active_child(&self, child: Id) -> bool {
        let ok = child != self.id && !self.world.borrow().contains.contains(&(child, self.id));
        self.ev(Ev::Add(self.id, child, ok));
        if ok {
            *self
                .world
                .borrow_mut()
                .links
                .entry((self.id, child))
                .or_insert(0) += 1;
        }
        ok
    }
    fn remove_active_child(&self, child: Id) {
        self.ev(Ev::Remove(self.id, child));
        let mut w = self.world.borrow_mut();
        let link = w.links.entry((self.id, child)).or_insert(0);
        assert!(*link > 0, "link {} -> {child} removed twice", self.id);
        *link -= 1;
    }
    fn warn_recursion(&self, child: Id) {
        self.ev(Ev::Warn(self.id, child));
    }
}

fn world() -> Rc<RefCell<World>> {
    Rc::new(RefCell::new(World::default()))
}

/// A child of the graph with one reference, its owner's.
fn child(w: &Rc<RefCell<World>>, id: Id) -> Id {
    w.borrow_mut().refs.insert(id, 1);
    id
}

/// Drops the owner's reference on `id`, as the C test's `drop_child()`.
fn drop_child(w: &Rc<RefCell<World>>, id: Id) {
    *w.borrow_mut().refs.entry(id).or_insert(0) -= 1;
}

/// A transition holding `a` in slot A and `b` in slot B, running towards
/// `b` in auto mode (the C test's `make_running()`).
fn make_running(w: &Rc<RefCell<World>>, id: Id, a: Id, b: Id) -> Tr {
    let tr = Tr::new(w, id);
    tr.hold(0, a, true);
    tr.hold(1, b, true);
    {
        let mut f = tr.f.borrow_mut();
        f.video = true;
        f.audio = true;
        f.start_time = 1_000;
        f.duration = 60_000_000_000;
    }
    tr
}

/// `obs_transition_is_active()` of the fake.
fn is_active(tr: &Tr) -> bool {
    let f = tr.fields();
    f.video || f.audio
}

const A: Id = 100;
const B: Id = 101;
const C: Id = 102;
const SCENE: Id = 200;
const OLD: Id = 1;
const NEW: Id = 2;

#[test]
fn constants_are_the_c_literals() {
    assert_eq!(CLEAR_LOG_NAME, c"obs_transition_clear");
    assert_eq!(
        RECURSION_WARNING,
        c"obs_transition_swap_begin: cannot give transition '%s' source '%s' due to infinite recursion"
    );
    assert_eq!(SLOTS, [0, 1]);
}

#[test]
fn c_clear_empties_both_slots() {
    let w = world();
    let (a, b) = (child(&w, A), child(&w, B));
    let tr = make_running(&w, OLD, a, b);
    drop_child(&w, a);
    drop_child(&w, b);
    assert_eq!(tr.fields().sources, [Some(a), Some(b)]);
    tr.events();

    clear(Some(&tr));
    assert_eq!(tr.fields().sources, [None, None]);
    assert_eq!(tr.fields().active, [false, false]);
    assert!(!is_active(&tr));
    // Both references dropped.
    assert_eq!(w.borrow().refs(a), 0);
    assert_eq!(w.borrow().refs(b), 0);
    assert_eq!(w.borrow().link(OLD, a), 0);
    assert_eq!(w.borrow().link(OLD, b), 0);
    assert_eq!(
        tr.events(),
        [
            Ev::Valid(CLEAR_LOG_NAME),
            Ev::Lock(OLD),
            Ev::Unlock(OLD),
            Ev::Remove(OLD, a),
            Ev::Release(Some(a)),
            Ev::Remove(OLD, b),
            Ev::Release(Some(b)),
        ]
    );

    // Clearing an empty transition releases two NULLs and removes nothing.
    clear(Some(&tr));
    assert_eq!(
        tr.events(),
        [
            Ev::Valid(CLEAR_LOG_NAME),
            Ev::Lock(OLD),
            Ev::Unlock(OLD),
            Ev::Release(None),
            Ev::Release(None),
        ]
    );

    // NULL, and a source that is not a transition: only the check.
    // NULL is checked too: transition_valid() logs it.
    NULL_CHECKS.with(|n| n.borrow_mut().clear());
    clear::<Tr>(None);
    assert_eq!(tr.events(), []);
    assert_eq!(NULL_CHECKS.with(|n| n.borrow().clone()), [CLEAR_LOG_NAME]);
    let mut input = Tr::new(&w, 50);
    input.is_transition = false;
    input.hold(0, child(&w, C), true);
    input.events();
    clear(Some(&input));
    assert_eq!(input.events(), [Ev::Valid(CLEAR_LOG_NAME)]);
    assert_eq!(input.fields().sources[0], Some(C));
}

#[test]
fn clear_keeps_no_link_for_an_inactive_slot() {
    let w = world();
    let tr = Tr::new(&w, OLD);
    tr.hold(0, child(&w, A), false);
    tr.hold(1, child(&w, B), true);
    tr.f.borrow_mut().video = true;
    tr.events();

    clear(Some(&tr));
    assert_eq!(
        tr.events(),
        [
            Ev::Valid(CLEAR_LOG_NAME),
            Ev::Lock(OLD),
            Ev::Unlock(OLD),
            Ev::Release(Some(A)),
            Ev::Remove(OLD, B),
            Ev::Release(Some(B)),
        ]
    );
    assert_eq!(w.borrow().refs(A), 1);
    assert_eq!(w.borrow().refs(B), 1);
    assert!(!is_active(&tr));
}

#[test]
fn clear_stops_audio_and_video_only() {
    for (video, audio) in [(true, false), (false, true), (true, true)] {
        let w = world();
        let tr = Tr::new(&w, OLD);
        {
            let mut f = tr.f.borrow_mut();
            f.video = video;
            f.audio = audio;
            f.start_time = 9;
            f.duration = 8;
            f.mode = 1;
            f.manual_val = 0.5;
            f.manual_target = 0.5;
        }
        let before = tr.fields();
        clear(Some(&tr));
        let after = tr.fields();
        assert!(!after.video && !after.audio);
        // The clock and render targets stay.
        assert_eq!(
            Fields {
                video,
                audio,
                ..after
            },
            before
        );
    }
}

fn enumerate(tr: &Tr) -> Vec<Id> {
    let mut seen = Vec::new();
    enum_sources(tr, |c| {
        assert!(tr.locked(), "the callback runs under the transition lock");
        tr.ev(Ev::Callback(tr.id, c));
        seen.push(c);
    });
    seen
}

#[test]
fn c_enum_visits_a_then_b() {
    let w = world();
    let tr = Tr::new(&w, OLD);
    assert!(enumerate(&tr).is_empty());
    assert_eq!(tr.events(), [Ev::Lock(OLD), Ev::Unlock(OLD)]);

    tr.hold(0, child(&w, A), true);
    assert_eq!(enumerate(&tr), [A]);
    tr.hold(1, child(&w, B), true);
    tr.events();
    assert_eq!(enumerate(&tr), [A, B]);
    assert_eq!(
        tr.events(),
        [
            Ev::Lock(OLD),
            Ev::Callback(OLD, A),
            Ev::Callback(OLD, B),
            Ev::Unlock(OLD),
        ]
    );

    // Slot A empty, slot B filled: only B.
    tr.f.borrow_mut().sources[0] = None;
    assert_eq!(enumerate(&tr), [B]);
    // An inactive slot is visited too: the enum reads only the child.
    tr.f.borrow_mut().active[1] = false;
    assert_eq!(enumerate(&tr), [B]);
    // No reference is taken or dropped.
    assert_eq!(w.borrow().refs(B), 2);
}

#[test]
fn c_swap_hands_over_both_slots() {
    let w = world();
    let (a, b) = (child(&w, A), child(&w, B));
    let old = make_running(&w, OLD, a, b);
    let new = Tr::new(&w, NEW);
    drop_child(&w, a);
    drop_child(&w, b);
    old.events();

    swap_begin(&new, &old);
    assert_eq!(new.fields().sources, [Some(a), Some(b)]);
    assert_eq!(new.fields().active, [true, true]);
    assert!(is_active(&new));
    assert_eq!(old.fields().sources, [Some(a), Some(b)]);
    assert!(is_active(&old));
    assert_eq!(w.borrow().refs(a), 2);
    assert_eq!(w.borrow().link(NEW, a), 1);
    assert_eq!(w.borrow().link(OLD, a), 1);
    // The texture locks stay held until the end.
    assert!(new.tex_locked() && old.tex_locked());
    assert!(!new.locked() && !old.locked());
    assert_eq!(
        old.events(),
        [
            Ev::LockTex(OLD),
            Ev::LockTex(NEW),
            Ev::Lock(OLD),
            Ev::Lock(NEW),
            Ev::GetRef(Some(a)),
            Ev::Add(NEW, a, true),
            Ev::GetRef(Some(b)),
            Ev::Add(NEW, b, true),
            Ev::Unlock(NEW),
            Ev::Unlock(OLD),
            Ev::Release(None),
            Ev::Release(None),
        ]
    );

    swap_end(&new, &old);
    assert_eq!(old.fields().sources, [None, None]);
    assert!(!is_active(&old));
    assert_eq!(new.fields().sources, [Some(a), Some(b)]);
    assert!(is_active(&new));
    assert!(!new.tex_locked() && !old.tex_locked());
    assert_eq!(
        old.events(),
        [
            Ev::Valid(CLEAR_LOG_NAME),
            Ev::Lock(OLD),
            Ev::Unlock(OLD),
            Ev::Remove(OLD, a),
            Ev::Release(Some(a)),
            Ev::Remove(OLD, b),
            Ev::Release(Some(b)),
            Ev::UnlockTex(NEW),
            Ev::UnlockTex(OLD),
        ]
    );

    // The new transition holds the only references.
    clear(Some(&new));
    assert_eq!(w.borrow().refs(a), 0);
    assert_eq!(w.borrow().refs(b), 0);
}

#[test]
fn c_swap_drops_what_the_new_one_held() {
    let w = world();
    let (a, b, c) = (child(&w, A), child(&w, B), child(&w, C));
    let old = Tr::new(&w, OLD);
    let new = Tr::new(&w, NEW);
    old.hold(0, a, true);
    new.hold(0, c, true);
    new.hold(1, b, true);
    new.f.borrow_mut().video = true;
    new.f.borrow_mut().audio = true;
    drop_child(&w, a);
    drop_child(&w, b);
    drop_child(&w, c);
    old.events();

    swap_begin(&new, &old);
    assert_eq!(
        old.events(),
        [
            Ev::LockTex(OLD),
            Ev::LockTex(NEW),
            Ev::Lock(OLD),
            Ev::Lock(NEW),
            Ev::GetRef(Some(a)),
            Ev::Remove(NEW, c),
            Ev::Add(NEW, a, true),
            Ev::GetRef(None),
            Ev::Remove(NEW, b),
            Ev::Unlock(NEW),
            Ev::Unlock(OLD),
            Ev::Release(Some(c)),
            Ev::Release(Some(b)),
        ]
    );
    swap_end(&new, &old);

    assert_eq!(new.fields().sources, [Some(a), None]);
    // The old one's empty slot B was inactive: so is the new one's.
    assert_eq!(new.fields().active, [true, false]);
    // The old one was not transitioning, so neither is the new one now.
    assert!(!is_active(&new));
    assert_eq!(w.borrow().refs(b), 0);
    assert_eq!(w.borrow().refs(c), 0);
    assert_eq!(w.borrow().link(NEW, b), 0);
    assert_eq!(w.borrow().link(NEW, c), 0);
    assert_eq!(old.fields().sources, [None, None]);
}

#[test]
fn c_swap_hands_over_the_textures() {
    let w = world();
    let old = make_running(&w, OLD, child(&w, A), child(&w, B));
    let new = Tr::new(&w, NEW);
    let (before_old, before_new) = (old.fields().texrender, new.fields().texrender);
    assert_ne!(before_old[0], before_old[1]);

    swap_begin(&new, &old);
    // Not yet: the render targets move at the end.
    assert_eq!(new.fields().texrender, before_new);
    swap_end(&new, &old);
    assert_eq!(new.fields().texrender, before_old);
    assert_eq!(old.fields().texrender, before_new);
}

#[test]
fn c_swap_hands_over_the_auto_clock() {
    let w = world();
    let old = make_running(&w, OLD, child(&w, A), child(&w, B));
    old.f.borrow_mut().start_time = 123_456_789;
    let new = Tr::new(&w, NEW);
    new.f.borrow_mut().start_time = 5;
    new.f.borrow_mut().duration = 7;

    swap_begin(&new, &old);
    let f = new.fields();
    assert_eq!(f.start_time, 123_456_789);
    assert_eq!(f.duration, 60_000_000_000);
    assert!(f.video && f.audio);
    swap_end(&new, &old);
    assert!(is_active(&new));
}

#[test]
fn c_swap_hands_over_the_manual_state() {
    let w = world();
    let old = Tr::new(&w, OLD);
    old.hold(0, child(&w, A), true);
    old.hold(1, child(&w, B), true);
    {
        let mut f = old.f.borrow_mut();
        f.video = true;
        f.audio = true;
        f.mode = 1; // OBS_TRANSITION_MODE_MANUAL
        f.manual_val = 0.25;
        f.manual_target = 0.25;
    }
    let new = Tr::new(&w, NEW);

    swap_begin(&new, &old);
    swap_end(&new, &old);
    let f = new.fields();
    assert_eq!(f.mode, 1);
    assert_eq!(f.manual_val.to_bits(), 0.25f32.to_bits());
    assert_eq!(f.manual_target.to_bits(), 0.25f32.to_bits());
    assert!(is_active(&new));
}

#[test]
fn c_swap_with_itself_does_nothing() {
    let w = world();
    let tr = make_running(&w, OLD, child(&w, A), child(&w, B));
    let before = tr.fields();
    tr.events();

    swap_begin(&tr, &tr);
    swap_end(&tr, &tr);
    assert_eq!(tr.fields(), before);
    assert_eq!(tr.events(), []);
    assert!(!tr.tex_locked());
}

#[test]
fn c_swap_does_not_store_the_new_transition() {
    let w = world();
    let old = Tr::new(&w, OLD);
    let new = Tr::new(&w, NEW);
    old.hold(0, NEW, true);
    old.events();

    swap_begin(&new, &old);
    assert_eq!(
        old.events(),
        [
            Ev::LockTex(OLD),
            Ev::LockTex(NEW),
            Ev::Lock(OLD),
            Ev::Lock(NEW),
            Ev::GetRef(Some(NEW)),
            Ev::Add(NEW, NEW, false),
            Ev::Warn(NEW, NEW),
            Ev::Release(Some(NEW)),
            Ev::GetRef(None),
            Ev::Unlock(NEW),
            Ev::Unlock(OLD),
            Ev::Release(None),
            Ev::Release(None),
        ]
    );
    assert_eq!(new.fields().sources, [None, None]);
    assert_eq!(new.fields().active, [false, false]);
    // Only the owner's and the old transition's references are left.
    assert_eq!(w.borrow().refs(NEW), 2);
    swap_end(&new, &old);
    assert_eq!(new.fields().sources, [None, None]);
    assert_eq!(old.fields().sources, [None, None]);
    assert_eq!(w.borrow().refs(NEW), 1);
}

#[test]
fn copy_refuses_a_child_that_contains_the_destination() {
    let w = world();
    let old = Tr::new(&w, OLD);
    let new = Tr::new(&w, NEW);
    // A scene holding the new transition, in the old one's slot B, and
    // something the new one held there before.
    let scene = child(&w, SCENE);
    w.borrow_mut().contains.push((scene, NEW));
    old.hold(1, scene, true);
    new.hold(1, child(&w, B), true);
    old.lock();
    new.lock();
    old.events();

    // copy_source_state() runs under both transition locks.
    assert_eq!(copy_source_state(&new, &old, 1), Some(B));
    assert_eq!(
        old.events(),
        [
            Ev::GetRef(Some(scene)),
            Ev::Remove(NEW, B),
            Ev::Add(NEW, scene, false),
            Ev::Warn(NEW, scene),
            Ev::Release(Some(scene)),
        ]
    );
    new.unlock();
    old.unlock();
    assert_eq!(new.fields().sources[1], None);
    assert!(!new.fields().active[1]);
    // The old one's slot is not touched; its reference stays.
    assert_eq!(old.fields().sources[1], Some(scene));
    assert_eq!(w.borrow().refs(scene), 2);
    // What the new one held stays referenced until the caller releases it.
    assert_eq!(w.borrow().refs(B), 2);
    assert_eq!(w.borrow().link(NEW, B), 0);
}

#[test]
fn copy_returns_what_the_destination_held() {
    let w = world();
    let old = Tr::new(&w, OLD);
    let new = Tr::new(&w, NEW);
    old.hold(0, child(&w, A), true);
    new.hold(0, child(&w, C), false);
    old.lock();
    new.lock();
    old.events();

    assert_eq!(copy_source_state(&new, &old, 0), Some(C));
    // C's slot was inactive: no remove.
    assert_eq!(old.events(), [Ev::GetRef(Some(A)), Ev::Add(NEW, A, true)]);
    assert_eq!(copy_source_state(&new, &old, 1), None);
    assert_eq!(old.events(), [Ev::GetRef(None)]);
    new.unlock();
    old.unlock();
}

#[test]
fn swap_stores_an_inactive_slot_without_adding_it() {
    let w = world();
    let old = Tr::new(&w, OLD);
    let new = Tr::new(&w, NEW);
    old.hold(0, child(&w, A), false);
    // An inactive slot holding a source that contains the destination is
    // stored: the add that would fail is never tried.
    let scene = child(&w, SCENE);
    w.borrow_mut().contains.push((scene, NEW));
    old.hold(1, scene, false);
    old.events();

    swap_begin(&new, &old);
    let ev = old.events();
    assert!(!ev.iter().any(|e| matches!(e, Ev::Add(..) | Ev::Warn(..))));
    assert_eq!(new.fields().sources, [Some(A), Some(scene)]);
    assert_eq!(new.fields().active, [false, false]);
    // The owner's, the old one's and the new one's.
    assert_eq!(w.borrow().refs(A), 3);
    swap_end(&new, &old);
    assert_eq!(w.borrow().refs(A), 2);
}

#[test]
fn swap_of_a_child_going_away_stores_null_with_its_active_flag() {
    let w = world();
    let old = Tr::new(&w, OLD);
    let new = Tr::new(&w, NEW);
    // A child whose last reference is gone: obs_source_get_ref() gives NULL.
    old.f.borrow_mut().sources[0] = Some(A);
    old.f.borrow_mut().active[0] = true;
    w.borrow_mut().refs.insert(A, 0);
    old.events();

    swap_begin(&new, &old);
    let ev = old.events();
    assert!(ev.contains(&Ev::GetRef(Some(A))));
    assert!(!ev.iter().any(|e| matches!(e, Ev::Add(..) | Ev::Warn(..))));
    // NULL is stored, and the slot keeps the old one's active flag, as the
    // C does: only a refused add clears it.
    assert_eq!(new.fields().sources[0], None);
    assert!(new.fields().active[0]);
    // The fake's old slot holds no reference of its own for the end's clear
    // to drop: empty it first.
    old.f.borrow_mut().sources[0] = None;
    swap_end(&new, &old);
}

#[test]
fn swap_moves_only_the_running_state() {
    let w = world();
    let old = Tr::new(&w, OLD);
    let new = Tr::new(&w, NEW);
    {
        let mut f = old.f.borrow_mut();
        f.video = true;
        f.audio = false;
        f.start_time = 11;
        f.duration = 22;
        f.mode = 1;
        f.manual_val = 0.5;
        f.manual_target = 0.75;
    }
    let old_before = old.fields();
    let new_tex = new.fields().texrender;

    swap_begin(&new, &old);
    let f = new.fields();
    assert!(f.video && !f.audio);
    assert_eq!((f.start_time, f.duration, f.mode), (11, 22, 1));
    assert_eq!(f.manual_val.to_bits(), 0.5f32.to_bits());
    assert_eq!(f.manual_target.to_bits(), 0.75f32.to_bits());
    assert_eq!(f.texrender, new_tex);
    // The begin leaves the old one as it was.
    assert_eq!(old.fields(), old_before);
    swap_end(&new, &old);
    // The end stops the old one; its clock fields stay.
    let f = old.fields();
    assert!(!f.video && !f.audio);
    assert_eq!((f.start_time, f.duration, f.mode), (11, 22, 1));
}

/// One slot of a generated transition: 0 empty, 1..=3 a plain child, 4 the
/// destination itself, 5 a scene that contains the destination.
#[derive(Debug, Clone, Copy)]
struct GenSlot {
    kind: u8,
    active: bool,
}

fn gen_slot() -> impl Strategy<Value = GenSlot> {
    (0u8..6, any::<bool>()).prop_map(|(kind, active)| GenSlot { kind, active })
}

fn slot_child(kind: u8) -> Option<Id> {
    match kind {
        0 => None,
        1..=3 => Some(A + usize::from(kind) - 1),
        4 => Some(NEW),
        _ => Some(SCENE),
    }
}

/// Whether `obs_transition_set()` and the start refuse to store `s` in the
/// destination: an active child that is the destination or contains it.
fn refused_in_new(s: GenSlot) -> bool {
    matches!(s.kind, 4 | 5) && s.active
}

/// How many slots of `sources` hold `id`.
fn held(sources: [Option<Id>; 2], id: Id) -> i64 {
    let n = sources.iter().filter(|c| **c == Some(id)).count();
    i64::try_from(n).unwrap_or(0)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(256)))]

    /// Any two transitions: after the begin and the end, the old one is
    /// empty and stopped, the new one holds the old one's children (an
    /// empty, inactive slot where the add was refused) and its running
    /// state, the render targets are swapped, every reference and active
    /// link is balanced, and both locks of both are free.
    #[test]
    fn swap_any_pair(
        old_slots in proptest::array::uniform2(gen_slot()),
        new_slots in proptest::array::uniform2(gen_slot()),
        video in any::<bool>(),
        audio in any::<bool>(),
        start_time in any::<u64>(),
        duration in any::<u64>(),
        mode in 0i32..2,
    ) {
        let w = world();
        for id in [A, A + 1, A + 2, SCENE] {
            child(&w, id);
        }
        w.borrow_mut().contains.push((SCENE, NEW));
        let old = Tr::new(&w, OLD);
        let new = Tr::new(&w, NEW);
        for (idx, s) in old_slots.iter().enumerate() {
            if let Some(c) = slot_child(s.kind) {
                old.hold(idx, c, s.active);
            }
        }
        let mut new_held = [None; 2];
        for (idx, s) in new_slots.iter().enumerate() {
            if let (Some(c), false) = (slot_child(s.kind), refused_in_new(*s)) {
                new.hold(idx, c, s.active);
                new_held[idx] = Some(c);
            }
        }
        {
            let mut f = old.f.borrow_mut();
            f.video = video;
            f.audio = audio;
            f.start_time = start_time;
            f.duration = duration;
            f.mode = mode;
        }
        let old_before = old.fields();
        let refs_before = w.borrow().refs.clone();
        let tex = (old.fields().texrender, new.fields().texrender);

        swap_begin(&new, &old);
        swap_end(&new, &old);

        let (of, nf) = (old.fields(), new.fields());
        prop_assert_eq!(of.sources, [None, None]);
        prop_assert_eq!(of.active, [false, false]);
        prop_assert!(!of.video && !of.audio);
        prop_assert_eq!((nf.video, nf.audio), (video, audio));
        prop_assert_eq!((nf.start_time, nf.duration, nf.mode), (start_time, duration, mode));
        prop_assert_eq!((nf.texrender, of.texrender), tex);
        prop_assert!(!old.locked() && !new.locked());
        prop_assert!(!old.tex_locked() && !new.tex_locked());

        for idx in SLOTS {
            let refused = old_before.active[idx]
                && matches!(old_before.sources[idx], Some(NEW | SCENE));
            let want = if refused { None } else { old_before.sources[idx] };
            prop_assert_eq!(nf.sources[idx], want);
            prop_assert_eq!(nf.active[idx], old_before.active[idx] && !refused);
        }

        // References: the old one's and the new one's earlier holds are
        // gone, the new one's current holds are counted.
        let w = w.borrow();
        for (&id, &before) in &refs_before {
            let expect = before - held(old_before.sources, id) - held(new_held, id)
                + held(nf.sources, id);
            prop_assert_eq!(w.refs(id), expect, "source {}", id);
        }
        // Active links: the old one has none, the new one one per active
        // slot.
        for (&(parent, child_id), &n) in &w.links {
            let want = if parent == NEW {
                let n = SLOTS
                    .iter()
                    .filter(|&&i| nf.sources[i] == Some(child_id) && nf.active[i])
                    .count();
                i64::try_from(n).unwrap_or(0)
            } else {
                0
            };
            prop_assert_eq!(n, want, "link {} -> {}", parent, child_id);
        }
    }
}
