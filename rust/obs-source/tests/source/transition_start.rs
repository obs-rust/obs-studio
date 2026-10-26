//! Tier 1: `obs_source::transition_start`, the start checks and child
//! handover of `obs_transition_start()`, `set_source()`,
//! `activate_transition()`, `obs_transition_get_active_source()`,
//! `obs_transition_save()`, `obs_transition_set_manual_time()`,
//! `obs_transition_set()` and `copy_source_state()`.
//!
//! The `steps_*` cases port `test/cmocka/test_transition_start_steps.c`
//! 1:1: its 17 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones and the
//! copies of the callers on a stand-in transition included. The C calls the
//! hidden `obs_source_transition_start_*()` functions; here the safe core is
//! called directly.
//!
//! The `edge_*` cases are new: every decision over every combination of its
//! flags against models of the original C expressions, the modes from
//! -70,000 to 70,000 and the ends of `i32`, the durations at every power of
//! two and its neighbours and a stride over `u32`, the manual reset over
//! every special `f32` and a stride over all bit patterns, and proptests:
//! full-range values, and operation sequences through the stand-in against a
//! copy of the callers with origin/master's expressions inline.

use obs_source::transition_start::{
    Admission, MODE_MANUAL, activates, active_index, adds, admit, calls_back, child_active,
    duration_ns, is_unsized, manual_reset, null_dest, refused, removes, restart, walks,
};
use proptest::prelude::*;

const AUTO: i32 = 0; // OBS_TRANSITION_MODE_AUTO
const MANUAL: i32 = MODE_MANUAL;
const MODES: [i32; 6] = [AUTO, MANUAL, 2, -1, i32::MIN, i32::MAX];

fn bit(bits: u32, n: u32) -> bool {
    bits & (1 << n) != 0
}

#[test]
fn steps_constants() {
    // the header's mirror of the core, and the enum's values
    assert_eq!(MODE_MANUAL, 1);
    assert_eq!(MODE_MANUAL, obs_source::transition_clock::MODE_MANUAL);
    assert_eq!(Admission::Go as i32, 0);
    assert_eq!(Admission::Refuse as i32, 1);
    assert_eq!(Admission::Done as i32, 2);
}

#[test]
fn steps_refused_and_walks() {
    assert!(refused(true, false));
    assert!(!refused(true, true));
    assert!(!refused(false, false));
    assert!(!refused(false, true));

    assert!(walks(true, false));
    assert!(!walks(true, true));
    assert!(!walks(false, false));
    assert!(!walks(false, true));
}

/// `obs_transition_start()`'s two returns after its second read, as
/// origin/master wrote them inline.
fn model_admit(ss: bool, sd: bool, same_mode: bool, mode: i32, active: bool) -> Admission {
    if ss && !active {
        Admission::Refuse
    } else if active && mode == 1 && same_mode && sd {
        Admission::Done
    } else {
        Admission::Go
    }
}

#[test]
fn steps_admit_every_combination() {
    for bits in 0..16 {
        let (ss, sd, same_mode, active) = (bit(bits, 0), bit(bits, 1), bit(bits, 2), bit(bits, 3));
        for mode in MODES {
            assert_eq!(
                admit(ss, sd, same_mode, mode, active),
                model_admit(ss, sd, same_mode, mode, active),
                "admit({ss}, {sd}, {same_mode}, {mode}, {active})"
            );
        }
    }

    assert_eq!(admit(true, true, true, MANUAL, false), Admission::Refuse);
    assert_eq!(admit(true, true, true, MANUAL, true), Admission::Done);
    assert_eq!(admit(false, true, true, AUTO, true), Admission::Go);
    assert_eq!(admit(false, true, false, MANUAL, true), Admission::Go);
    assert_eq!(admit(false, false, true, MANUAL, true), Admission::Go);
}

#[test]
fn steps_duration() {
    assert_eq!(duration_ns(0), 0);
    assert_eq!(duration_ns(1), 1_000_000);
    assert_eq!(duration_ns(300), 300_000_000);
    assert_eq!(duration_ns(4294), 4_294_000_000);
    // past 32 bits: the product is taken in 64 bits
    assert_eq!(duration_ns(4295), 4_295_000_000);
    assert_eq!(duration_ns(u32::MAX), 4_294_967_295_000_000);
}

#[test]
fn steps_set_source_rules() {
    for bits in 0..8 {
        let (a, b, c) = (bit(bits, 0), bit(bits, 1), bit(bits, 2));
        assert_eq!(restart(a, b, c), !a || (!b && !c));
        assert_eq!(removes(a, b, c), a && b && c);
        assert_eq!(null_dest(a, b, c), !a && b && !c);
    }
    for bits in 0..4 {
        let (a, b) = (bit(bits, 0), bit(bits, 1));
        assert_eq!(adds(a, b), a && b);
        assert_eq!(calls_back(a, b), a && b);
        assert_eq!(child_active(a, b), a && b);
    }

    assert!(restart(true, false, false));
    assert!(!restart(true, true, false));
    assert!(!restart(true, false, true));
    assert!(restart(false, true, true));

    assert!(is_unsized(0));
    assert!(!is_unsized(1));
    assert!(!is_unsized(1920));
    assert!(!is_unsized(u32::MAX));

    assert!(activates(false));
    assert!(!activates(true));

    assert_eq!(active_index(false), 0);
    assert_eq!(active_index(true), 1);
}

#[test]
fn steps_manual_reset() {
    let tiny = f32::from_bits(1); // the smallest denormal

    assert!(manual_reset(MANUAL, 0.0));
    // -0.0 compares equal to 0.0: it resets
    assert!(manual_reset(MANUAL, -0.0));
    // NaN compares unequal: it does not
    assert!(!manual_reset(MANUAL, f32::NAN));
    assert!(!manual_reset(MANUAL, 1.0));
    assert!(!manual_reset(MANUAL, tiny));
    assert!(!manual_reset(MANUAL, -tiny));
    assert!(!manual_reset(MANUAL, -1.0));
    assert!(!manual_reset(MANUAL, f32::INFINITY));

    for mode in MODES {
        if mode == MANUAL {
            continue;
        }
        assert!(!manual_reset(mode, 0.0));
        assert!(!manual_reset(mode, -0.0));
        assert!(!manual_reset(mode, f32::NAN));
    }
}

// ---- the callers, copied onto a stand-in transition ----

/// A stand-in child: its references and active references.
#[derive(Debug, Clone, Default, PartialEq)]
struct Child {
    refs: i32,
    active_refs: i32,
    /// `obs_source_add_active_child()` fails (a cycle)
    fail_add: bool,
    /// the destination is the transition itself
    is_self: bool,
    /// the destination's tree contains the transition
    contains: bool,
}

type ChildId = usize;

/// A stand-in transition.
#[derive(Debug, Clone, Default, PartialEq)]
struct Tr {
    src: [Option<ChildId>; 2],
    slot_active: [bool; 2],
    audio: bool,
    video: bool,
    mode: i32,
    val: f32,
    target: f32,
    start_time: u64,
    duration: u64,
    use_fixed: bool,
    fixed_ms: u32,
    cx: u32,
    cy: u32,
    now: u64,
    resizes: i32,
    start_signals: i32,
    stop_signals: i32,
    warnings: i32,
    plugin_calls: i32,
    /// the plugin's `transition_start` starts this transition again, once
    reenters: bool,
    nested_mode: i32,
    nested_ms: u32,
    nested_dest: Option<ChildId>,
    nested_result: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct World {
    children: Vec<Child>,
    trs: Vec<Tr>,
}

/// The C's choices where the stand-in copies of the callers differ between
/// the port (each decision asked from the core) and origin/master (the
/// expressions inline): the proptest runs both.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Rules {
    Core,
    Inline,
}

#[derive(Debug, Clone, Copy)]
struct StartClock {
    duration: u64,
    same_as_source: bool,
    same_as_dest: bool,
    restart: bool,
}

impl World {
    fn child(&mut self, c: Child) -> ChildId {
        self.children.push(c);
        self.children.len() - 1
    }

    fn get_ref(&mut self, c: Option<ChildId>) -> Option<ChildId> {
        if let Some(c) = c {
            self.children[c].refs += 1;
        }
        c
    }

    fn release(&mut self, c: Option<ChildId>) {
        if let Some(c) = c {
            self.children[c].refs -= 1;
        }
    }

    /// `obs_source_add_active_child()`: false for NULL and for a cycle
    fn add_active(&mut self, c: Option<ChildId>) -> bool {
        match c {
            Some(c) if !self.children[c].fail_add => {
                self.children[c].active_refs += 1;
                true
            }
            _ => false,
        }
    }

    fn remove_active(&mut self, c: Option<ChildId>) {
        if let Some(c) = c {
            self.children[c].active_refs -= 1;
        }
    }

    fn tr_active(&self, t: usize) -> bool {
        self.trs[t].audio || self.trs[t].video
    }

    fn write_clock(&mut self, t: usize, clock: &StartClock) {
        if clock.restart {
            let tr = &mut self.trs[t];
            tr.now += 1;
            tr.start_time = tr.now;
            tr.duration = clock.duration;
        }
    }

    fn set_source(
        &mut self,
        r: Rules,
        t: usize,
        idx: usize,
        new_child: Option<ChildId>,
        clock: &mut StartClock,
    ) -> bool {
        let mut add_success = true;
        let new_child = self.get_ref(new_child);
        let old_child = self.trs[t].src[idx];

        clock.same_as_source = new_child == self.trs[t].src[0];
        clock.same_as_dest = new_child == old_child;
        let active = self.tr_active(t);
        clock.restart = match r {
            Rules::Core => restart(active, clock.same_as_dest, clock.same_as_source),
            Rules::Inline => !active || (!clock.same_as_dest && !clock.same_as_source),
        };

        if new_child == old_child {
            self.write_clock(t, clock);
            self.release(new_child);
            return true;
        }

        let already_active = self.trs[t].slot_active[idx];
        match r {
            Rules::Core => {
                if adds(already_active, new_child.is_some()) {
                    add_success = self.add_active(new_child);
                }
                if removes(already_active, old_child.is_some(), add_success) {
                    self.remove_active(old_child);
                }
                // the callback is always activate_transition()
                if calls_back(true, add_success) {
                    add_success = self.activate(r, t, idx, new_child);
                }
            }
            Rules::Inline => {
                if already_active {
                    if new_child.is_some() {
                        add_success = self.add_active(new_child);
                    }
                    if old_child.is_some() && add_success {
                        self.remove_active(old_child);
                    }
                }
                if add_success {
                    add_success = self.activate(r, t, idx, new_child);
                }
            }
        }
        if add_success {
            self.write_clock(t, clock);
            self.trs[t].src[idx] = new_child;
        }
        if !add_success {
            self.release(new_child);
            return false;
        }
        let (cx, cy) = (self.trs[t].cx, self.trs[t].cy);
        let resize = match r {
            Rules::Core => is_unsized(cx) || is_unsized(cy),
            Rules::Inline => cx == 0 || cy == 0,
        };
        if resize {
            self.trs[t].resizes += 1;
        }
        self.release(old_child);
        true
    }

    fn activate(&mut self, r: Rules, t: usize, idx: usize, child: Option<ChildId>) -> bool {
        let slot = self.trs[t].slot_active[idx];
        let add = match r {
            Rules::Core => activates(slot),
            Rules::Inline => !slot,
        };
        if add {
            if !self.add_active(child) {
                return false;
            }
            self.trs[t].slot_active[idx] = true;
        }
        self.trs[t].video = true;
        self.trs[t].audio = true;
        true
    }

    fn set(&mut self, r: Rules, t: usize, source: Option<ChildId>) {
        let source = self.get_ref(source);
        if self.tr_active(t) {
            self.trs[t].stop_signals += 1;
        }
        let tr = &mut self.trs[t];
        let s = tr.src;
        let active = tr.slot_active;
        tr.src = [None, None];
        tr.slot_active = [true, false];
        tr.src[0] = source;
        tr.video = false;
        tr.audio = false;
        tr.val = 0.0;
        tr.target = 0.0;
        for i in 0..2 {
            let out = match r {
                Rules::Core => child_active(s[i].is_some(), active[i]),
                Rules::Inline => s[i].is_some() && active[i],
            };
            if out {
                self.remove_active(s[i]);
            }
            self.release(s[i]);
        }
        if source.is_some() {
            self.add_active(source);
        }
    }

    fn start(
        &mut self,
        r: Rules,
        t: usize,
        mode: i32,
        mut duration_ms: u32,
        dest: Option<ChildId>,
    ) -> bool {
        let same_as_source = dest == self.trs[t].src[0];
        let active = self.tr_active(t);
        let first_refused = match r {
            Rules::Core => refused(same_as_source, active),
            Rules::Inline => same_as_source && !active,
        };
        if first_refused {
            return false;
        }

        let mut recursive = dest.is_some_and(|d| self.children[d].is_self);
        let walk = match r {
            Rules::Core => walks(dest.is_some(), recursive),
            Rules::Inline => dest.is_some() && !recursive,
        };
        if walk {
            recursive = dest.is_some_and(|d| self.children[d].contains);
        }
        if recursive {
            self.trs[t].warnings += 1;
            return false;
        }

        if self.tr_active(t) {
            let b = self.trs[t].src[1];
            self.set(r, t, b);
        }

        let same_as_source = dest == self.trs[t].src[0];
        let same_as_dest = dest == self.trs[t].src[1];
        let same_mode = mode == self.trs[t].mode;
        let active = self.tr_active(t);
        let admission = match r {
            Rules::Core => admit(same_as_source, same_as_dest, same_mode, mode, active),
            Rules::Inline => model_admit(same_as_source, same_as_dest, same_mode, mode, active),
        };
        match admission {
            Admission::Refuse => return false,
            Admission::Done => return true,
            Admission::Go => {}
        }

        let tr = &mut self.trs[t];
        tr.mode = mode;
        tr.val = 0.0;
        tr.target = 0.0;

        if self.trs[t].reenters {
            self.reenter(r, t);
        }

        if self.trs[t].use_fixed {
            duration_ms = self.trs[t].fixed_ms;
        }

        let mut clock = StartClock {
            duration: match r {
                Rules::Core => duration_ns(duration_ms),
                Rules::Inline => u64::from(duration_ms) * 1_000_000,
            },
            same_as_source: false,
            same_as_dest: false,
            restart: false,
        };
        if !self.set_source(r, t, 1, dest, &mut clock) {
            self.trs[t].warnings += 1;
            return false;
        }
        let flags = match r {
            Rules::Core => null_dest(dest.is_some(), clock.same_as_dest, clock.same_as_source),
            Rules::Inline => dest.is_none() && clock.same_as_dest && !clock.same_as_source,
        };
        if flags {
            self.trs[t].video = true;
            self.trs[t].audio = true;
        }
        self.trs[t].start_signals += 1;
        self.trs[t].resizes += 1;
        true
    }

    /// The re-entering plugin: on its first call it starts the transition
    /// again with the nested arguments.
    fn reenter(&mut self, r: Rules, t: usize) {
        self.trs[t].plugin_calls += 1;
        if self.trs[t].plugin_calls == 1 {
            let tr = &self.trs[t];
            let (mode, ms, dest) = (tr.nested_mode, tr.nested_ms, tr.nested_dest);
            let res = self.start(r, t, mode, ms, dest);
            self.trs[t].nested_result = res;
        }
    }

    fn set_manual_time(&mut self, r: Rules, t: usize, f: f32) {
        self.trs[t].target = f;
        let mode = self.trs[t].mode;
        #[allow(clippy::float_cmp)]
        let reset = match r {
            Rules::Core => manual_reset(mode, f),
            Rules::Inline => mode == 1 && f == 0.0,
        };
        if reset {
            let a = self.trs[t].src[0];
            self.set(r, t, a);
        }
    }

    fn get_active(&self, r: Rules, t: usize) -> Option<ChildId> {
        let active = self.tr_active(t);
        match r {
            Rules::Core => self.trs[t].src[active_index(active)],
            Rules::Inline => {
                if active {
                    self.trs[t].src[1]
                } else {
                    self.trs[t].src[0]
                }
            }
        }
    }

    fn copy_state(&mut self, r: Rules, dest: usize, source: usize, idx: usize) -> Option<ChildId> {
        let old_child = self.trs[dest].src[idx];
        let new_child = self.get_ref(self.trs[source].src[idx]);
        let active = self.trs[source].slot_active[idx];
        if old_child.is_some() && self.trs[dest].slot_active[idx] {
            self.remove_active(old_child);
        }
        self.trs[dest].src[idx] = new_child;
        self.trs[dest].slot_active[idx] = active;
        let comes_in = match r {
            Rules::Core => child_active(new_child.is_some(), active),
            Rules::Inline => active && new_child.is_some(),
        };
        if comes_in {
            self.add_active(new_child);
        }
        old_child
    }

    /// `obs_transition_swap_begin()` and `_end()`; `_end()` clears the
    /// source.
    fn swap(&mut self, r: Rules, dest: usize, source: usize) {
        if dest == source {
            return;
        }
        let old = [
            self.copy_state(r, dest, source, 0),
            self.copy_state(r, dest, source, 1),
        ];
        let s = self.trs[source].clone();
        let d = &mut self.trs[dest];
        d.video = s.video;
        d.audio = s.audio;
        d.start_time = s.start_time;
        d.duration = s.duration;
        d.mode = s.mode;
        d.val = s.val;
        d.target = s.target;
        for c in old {
            self.release(c);
        }
        for i in 0..2 {
            let c = self.trs[source].src[i];
            let active = self.trs[source].slot_active[i];
            self.trs[source].src[i] = None;
            self.trs[source].slot_active[i] = false;
            if c.is_some() && active {
                self.remove_active(c);
            }
            self.release(c);
        }
        self.trs[source].video = false;
        self.trs[source].audio = false;
    }

    /// A transition showing `a`, idle, as `obs_transition_init()` and a set
    /// leave it.
    fn tr(&mut self, a: Option<ChildId>) -> usize {
        self.trs.push(Tr {
            slot_active: [true, false],
            cx: 1920,
            cy: 1080,
            mode: AUTO,
            ..Tr::default()
        });
        let t = self.trs.len() - 1;
        if a.is_some() {
            self.set(Rules::Core, t, a);
        }
        t
    }
}

const C: Rules = Rules::Core;

#[test]
fn steps_start_from_idle() {
    let mut w = World::default();
    let a = w.child(Child::default());
    let b = w.child(Child::default());
    let t = w.tr(Some(a));
    assert_eq!(w.children[a].refs, 1);
    assert_eq!(w.children[a].active_refs, 1);

    assert!(w.start(C, t, AUTO, 300, Some(b)));
    assert_eq!(w.trs[t].src[1], Some(b));
    assert!(w.trs[t].slot_active[1]);
    assert!(w.trs[t].audio && w.trs[t].video);
    assert_eq!(w.trs[t].duration, 300_000_000);
    assert_eq!(w.trs[t].start_time, 1);
    assert_eq!(w.children[b].refs, 1);
    assert_eq!(w.children[b].active_refs, 1);
    assert_eq!(w.trs[t].start_signals, 1);
    assert_eq!(w.trs[t].resizes, 1);
    assert_eq!(w.get_active(C, t), Some(b));

    let c = w.child(Child::default());
    let u = w.tr(Some(a));
    w.trs[u].cy = 0;
    assert!(w.start(C, u, AUTO, 300, Some(c)));
    assert_eq!(w.trs[u].resizes, 2);
}

#[test]
fn steps_start_to_source_a_refused() {
    let mut w = World::default();
    let a = w.child(Child::default());
    let t = w.tr(Some(a));

    assert!(!w.start(C, t, AUTO, 300, Some(a)));
    assert_eq!(w.trs[t].src[1], None);
    assert!(!(w.trs[t].audio || w.trs[t].video));
    assert_eq!(w.trs[t].start_signals, 0);
    assert_eq!(w.trs[t].mode, AUTO);

    let me = w.child(Child {
        is_self: true,
        ..Child::default()
    });
    let holder = w.child(Child {
        contains: true,
        ..Child::default()
    });
    assert!(!w.start(C, t, MANUAL, 300, Some(me)));
    assert!(!w.start(C, t, MANUAL, 300, Some(holder)));
    assert_eq!(w.trs[t].warnings, 2);
    assert_eq!(w.trs[t].mode, AUTO);
    assert_eq!(w.children[me].refs + w.children[holder].refs, 0);
}

#[test]
fn steps_start_while_running() {
    let mut w = World::default();
    let a = w.child(Child::default());
    let b = w.child(Child::default());
    let c = w.child(Child::default());
    let t = w.tr(Some(a));
    assert!(w.start(C, t, AUTO, 300, Some(b)));

    assert!(w.start(C, t, AUTO, 500, Some(c)));
    assert_eq!(w.trs[t].stop_signals, 1);
    assert_eq!(w.trs[t].src, [Some(b), Some(c)]);
    assert_eq!(w.children[a].refs, 0);
    assert_eq!(w.children[a].active_refs, 0);
    assert_eq!(w.children[b].refs, 1);
    assert_eq!(w.children[b].active_refs, 1);
    assert_eq!(w.children[c].active_refs, 1);
    assert_eq!(w.trs[t].duration, 500_000_000);

    assert!(w.start(C, t, AUTO, 700, Some(b)));
    assert_eq!(w.trs[t].src, [Some(c), Some(b)]);
    assert_eq!(w.children[b].refs, 1);
    assert_eq!(w.children[b].active_refs, 1);
}

#[test]
fn steps_null_destination() {
    let mut w = World::default();
    let a = w.child(Child::default());

    let t = w.tr(Some(a));
    assert!(w.start(C, t, AUTO, 300, None));
    assert!(w.trs[t].audio && w.trs[t].video);
    assert_eq!(w.trs[t].src[1], None);
    assert!(!w.trs[t].slot_active[1]);
    assert_eq!(w.trs[t].duration, 300_000_000);
    assert_eq!(w.get_active(C, t), None);

    let e = w.tr(None);
    assert!(!w.start(C, e, AUTO, 300, None));
    assert!(!(w.trs[e].audio || w.trs[e].video));

    // characterized, not endorsed: a source B in an inactive slot
    let b = w.child(Child::default());
    let f = w.tr(Some(a));
    w.trs[f].src[1] = w.get_ref(Some(b));
    w.trs[f].slot_active[1] = false;
    assert!(!w.start(C, f, AUTO, 300, None));
    assert_eq!(w.trs[f].warnings, 1);
    assert_eq!(w.trs[f].src[1], Some(b));
    assert!(!(w.trs[f].audio || w.trs[f].video));
}

#[test]
fn steps_manual_same_target() {
    let mut w = World::default();
    let a = w.child(Child::default());
    let b = w.child(Child::default());
    let t = w.tr(Some(a));
    assert!(w.start(C, t, MANUAL, 300, Some(b)));
    w.trs[t].val = 0.25;
    w.trs[t].target = 0.5;

    // characterized, not endorsed: the finish step runs first
    assert!(!w.start(C, t, MANUAL, 300, Some(b)));
    assert_eq!(w.trs[t].src, [Some(b), None]);
    assert!(!(w.trs[t].audio || w.trs[t].video));
    assert_eq!(w.children[a].active_refs, 0);
    assert_eq!(w.children[b].active_refs, 1);
    assert_eq!(w.trs[t].stop_signals, 1);

    assert_eq!(admit(false, true, true, MANUAL, true), Admission::Done);
}

#[test]
fn steps_reentering_callback() {
    let mut w = World::default();
    let a = w.child(Child::default());
    let b = w.child(Child::default());
    let c = w.child(Child::default());
    let t = w.tr(Some(a));
    w.trs[t].reenters = true;
    w.trs[t].nested_mode = MANUAL;
    w.trs[t].nested_ms = 900;
    w.trs[t].nested_dest = Some(c);

    assert!(w.start(C, t, AUTO, 300, Some(b)));
    assert!(w.trs[t].nested_result);
    assert_eq!(w.trs[t].plugin_calls, 2);
    assert_eq!(w.trs[t].src[1], Some(b));
    assert_eq!(w.trs[t].duration, 300_000_000);
    // characterized, not endorsed: the nested start's mode stays
    assert_eq!(w.trs[t].mode, MANUAL);
    assert_eq!(w.children[c].refs, 0);
    assert_eq!(w.children[c].active_refs, 0);
    assert_eq!(w.children[b].active_refs, 1);
    assert_eq!(w.trs[t].start_signals, 2);

    let d = w.child(Child::default());
    let e = w.child(Child::default());
    let u = w.tr(Some(d));
    w.trs[u].reenters = true;
    w.trs[u].nested_mode = AUTO;
    w.trs[u].nested_ms = 900;
    w.trs[u].nested_dest = Some(e);
    assert!(w.start(C, u, AUTO, 300, Some(e)));
    assert!(w.trs[u].nested_result);
    assert_eq!(w.trs[u].duration, 900_000_000);
    assert_eq!(w.trs[u].start_time, 1);
    assert_eq!(w.children[e].refs, 1);
    assert_eq!(w.children[e].active_refs, 1);
    assert_eq!(w.trs[u].start_signals, 2);
}

#[test]
fn steps_failed_add_keeps_nested_clock() {
    let mut w = World::default();
    let a = w.child(Child::default());
    let b = w.child(Child {
        fail_add: true,
        ..Child::default()
    });
    let c = w.child(Child::default());
    let t = w.tr(Some(a));
    w.trs[t].reenters = true;
    w.trs[t].nested_mode = AUTO;
    w.trs[t].nested_ms = 900;
    w.trs[t].nested_dest = Some(c);

    assert!(!w.start(C, t, AUTO, 300, Some(b)));
    assert!(w.trs[t].nested_result);
    assert_eq!(w.trs[t].warnings, 1);
    assert_eq!(w.trs[t].src[1], Some(c));
    assert_eq!(w.trs[t].duration, 900_000_000);
    assert_eq!(w.trs[t].start_time, 1);
    assert_eq!(w.children[b].refs, 0);
    assert_eq!(w.children[b].active_refs, 0);
    assert_eq!(w.children[c].active_refs, 1);
    assert!(w.trs[t].slot_active[1]);
}

#[test]
fn steps_fixed_duration() {
    const MS: [u32; 4] = [0, 1, 300, u32::MAX];
    for f in MS {
        for r in MS {
            let mut w = World::default();
            let a = w.child(Child::default());
            let b = w.child(Child::default());

            let t = w.tr(Some(a));
            w.trs[t].use_fixed = true;
            w.trs[t].fixed_ms = f;
            assert!(w.start(C, t, AUTO, r, Some(b)));
            assert_eq!(w.trs[t].duration, u64::from(f) * 1_000_000);

            let t = w.tr(Some(a));
            w.trs[t].use_fixed = false;
            w.trs[t].fixed_ms = f;
            assert!(w.start(C, t, AUTO, r, Some(b)));
            assert_eq!(w.trs[t].duration, u64::from(r) * 1_000_000);
        }
    }
}

#[test]
fn steps_manual_times() {
    let times = [0.0f32, -0.0, f32::NAN, 1.0];
    let resets = [true, true, false, false];
    for (time, reset) in times.into_iter().zip(resets) {
        let mut w = World::default();
        let a = w.child(Child::default());
        let b = w.child(Child::default());
        let t = w.tr(Some(a));
        assert!(w.start(C, t, MANUAL, 300, Some(b)));
        w.trs[t].val = 0.5;

        w.set_manual_time(C, t, time);
        if reset {
            assert!(!(w.trs[t].audio || w.trs[t].video));
            assert_eq!(w.trs[t].src, [Some(a), None]);
            assert_eq!(w.children[b].refs, 0);
            assert_eq!(w.children[b].active_refs, 0);
            assert_eq!(w.children[a].active_refs, 1);
            assert_eq!(w.trs[t].val.to_bits(), 0.0f32.to_bits());
        } else {
            assert!(w.trs[t].audio && w.trs[t].video);
            assert_eq!(w.trs[t].src[1], Some(b));
            assert_eq!(w.trs[t].val.to_bits(), 0.5f32.to_bits());
        }

        let u = w.tr(Some(a));
        assert!(w.start(C, u, AUTO, 300, Some(b)));
        w.set_manual_time(C, u, time);
        assert!(w.trs[u].audio && w.trs[u].video);
    }
}

#[test]
fn steps_swap() {
    let mut w = World::default();
    let a = w.child(Child::default());
    let b = w.child(Child::default());

    let t = w.tr(Some(a));
    assert!(w.start(C, t, AUTO, 300, Some(b)));
    w.swap(C, t, t);
    assert_eq!(w.trs[t].src[1], Some(b));
    assert_eq!(w.children[b].active_refs, 1);
    assert!(w.trs[t].audio);

    let n = w.tr(None);
    w.swap(C, n, t);
    assert_eq!(w.trs[n].src, [Some(a), Some(b)]);
    assert_eq!(w.trs[n].slot_active, [true, true]);
    assert!(w.trs[n].audio && w.trs[n].video);
    assert_eq!(w.trs[n].duration, 300_000_000);
    assert_eq!(w.children[a].refs, 1);
    assert_eq!(w.children[a].active_refs, 1);
    assert_eq!(w.children[b].refs, 1);
    assert_eq!(w.children[b].active_refs, 1);
    assert_eq!(w.trs[t].src[0], None);
    assert!(!w.trs[t].audio);

    let c = w.child(Child::default());
    let d = w.child(Child::default());
    let p = w.tr(None);
    let q = w.tr(None);
    w.trs[p].src[1] = w.get_ref(Some(c));
    w.trs[p].slot_active[1] = false;
    w.trs[q].src[1] = w.get_ref(Some(d));
    w.trs[q].slot_active[1] = false;
    w.swap(C, q, p);
    assert_eq!(w.trs[q].src[1], Some(c));
    assert!(!w.trs[q].slot_active[1]);
    assert_eq!(w.children[c].refs, 1);
    assert_eq!(w.children[c].active_refs, 0);
    assert_eq!(w.children[d].refs, 0);
    assert_eq!(w.children[d].active_refs, 0);
}

#[test]
fn steps_set_active_and_idle() {
    let mut w = World::default();
    let a = w.child(Child::default());
    let b = w.child(Child::default());
    let c = w.child(Child::default());

    let t = w.tr(Some(a));
    assert!(w.start(C, t, AUTO, 300, Some(b)));
    w.set(C, t, Some(c));
    assert_eq!(w.trs[t].stop_signals, 1);
    assert_eq!(w.children[a].active_refs, 0);
    assert_eq!(w.children[b].active_refs, 0);
    assert_eq!(w.children[c].active_refs, 1);
    assert_eq!(w.children[a].refs + w.children[b].refs, 0);
    assert_eq!(w.trs[t].src[0], Some(c));
    assert_eq!(w.get_active(C, t), Some(c));

    w.set(C, t, Some(a));
    assert_eq!(w.trs[t].stop_signals, 1);
    assert_eq!(w.children[c].active_refs, 0);
    assert_eq!(w.children[a].active_refs, 1);

    w.set(C, t, None);
    assert!(w.trs[t].slot_active[0]);
    assert_eq!(w.trs[t].src[0], None);
    assert_eq!(w.children[a].active_refs, 0);

    // characterized, not endorsed: the failed add still marks slot A active
    let e = w.child(Child {
        fail_add: true,
        ..Child::default()
    });
    w.set(C, t, Some(e));
    assert!(w.trs[t].slot_active[0]);
    assert_eq!(w.children[e].active_refs, 0);
    w.set(C, t, None);
    assert_eq!(w.children[e].active_refs, -1);
}

// ---- edge cases ----

#[test]
fn edge_every_flag_combination() {
    for bits in 0..32u32 {
        let f = |n| bit(bits, n);
        assert_eq!(refused(f(0), f(1)), f(0) && !f(1));
        assert_eq!(walks(f(0), f(1)), f(0) && !f(1));
        assert_eq!(restart(f(0), f(1), f(2)), !(f(0)) || (!f(1) && !f(2)));
        assert_eq!(adds(f(0), f(1)), f(0) && f(1));
        assert_eq!(removes(f(0), f(1), f(2)), f(0) && f(1) && f(2));
        assert_eq!(calls_back(f(0), f(1)), f(0) && f(1));
        assert_eq!(null_dest(f(0), f(1), f(2)), !f(0) && f(1) && !f(2));
        assert_eq!(activates(f(0)), !f(0));
        assert_eq!(active_index(f(0)), usize::from(f(0)));
        assert_eq!(child_active(f(0), f(1)), f(0) && f(1));
        assert_eq!(
            admit(f(0), f(1), f(2), MANUAL, f(3)),
            model_admit(f(0), f(1), f(2), 1, f(3))
        );
    }
}

#[test]
fn edge_modes() {
    let mut modes: Vec<i32> = (-70_000..=70_000).collect();
    modes.extend([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX]);
    for mode in modes {
        for bits in 0..16 {
            let (ss, sd, same_mode, active) =
                (bit(bits, 0), bit(bits, 1), bit(bits, 2), bit(bits, 3));
            assert_eq!(
                admit(ss, sd, same_mode, mode, active),
                model_admit(ss, sd, same_mode, mode, active)
            );
        }
        assert_eq!(manual_reset(mode, 0.0), mode == 1);
        assert_eq!(manual_reset(mode, -0.0), mode == 1);
        assert!(!manual_reset(mode, f32::NAN));
    }
}

#[test]
fn edge_durations() {
    let mut ms: Vec<u32> = Vec::new();
    for p in 0..32 {
        let v = 1u32 << p;
        ms.extend([v - 1, v, v.wrapping_add(1)]);
    }
    ms.extend((0..=u32::MAX).step_by(65_521));
    ms.extend([u32::MAX - 1, u32::MAX, 4294, 4295]);
    for m in ms {
        assert_eq!(u128::from(duration_ns(m)), u128::from(m) * 1_000_000, "{m}");
    }
}

#[test]
fn edge_manual_times() {
    let special = [
        0.0f32,
        -0.0,
        f32::NAN,
        -f32::NAN,
        f32::from_bits(0x7fc0_0001),
        f32::from_bits(0xff80_0001),
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::from_bits(0x8000_0001),
        f32::MAX,
        f32::MIN,
        f32::EPSILON,
        1.0,
        -1.0,
        0.5,
    ];
    for t in special {
        for mode in MODES {
            #[allow(clippy::float_cmp)]
            let want = mode == 1 && t == 0.0;
            assert_eq!(manual_reset(mode, t), want, "{mode} {:#x}", t.to_bits());
        }
    }
    // a stride over every bit pattern: only the two zeros reset
    for b in (0..=u32::MAX).step_by(4099).chain([0, 0x8000_0000]) {
        let t = f32::from_bits(b);
        assert_eq!(
            manual_reset(MANUAL, t),
            b == 0 || b == 0x8000_0000,
            "{b:#x}"
        );
    }
}

// ---- proptests ----

#[derive(Debug, Clone)]
enum Op {
    Start {
        t: usize,
        mode: i32,
        ms: u32,
        dest: Option<usize>,
    },
    Set {
        t: usize,
        source: Option<usize>,
    },
    ManualTime {
        t: usize,
        f: f32,
    },
    GetActive {
        t: usize,
    },
    Swap {
        dest: usize,
        source: usize,
    },
}

const CHILDREN: usize = 5;

fn op() -> impl Strategy<Value = Op> {
    let child = prop::option::of(0..CHILDREN);
    let mode = prop_oneof![Just(AUTO), Just(MANUAL), any::<i32>()];
    let ms = prop_oneof![Just(0u32), Just(300), Just(u32::MAX), any::<u32>()];
    let f = prop_oneof![
        Just(0.0f32),
        Just(-0.0),
        Just(f32::NAN),
        Just(1.0),
        any::<u32>().prop_map(f32::from_bits)
    ];
    prop_oneof![
        3 => (0..2usize, mode, ms, child.clone()).prop_map(|(t, mode, ms, dest)| Op::Start { t, mode, ms, dest }),
        1 => (0..2usize, child).prop_map(|(t, source)| Op::Set { t, source }),
        1 => (0..2usize, f).prop_map(|(t, f)| Op::ManualTime { t, f }),
        1 => (0..2usize).prop_map(|t| Op::GetActive { t }),
        1 => (0..2usize, 0..2usize).prop_map(|(dest, source)| Op::Swap { dest, source }),
    ]
}

fn world(flags: u64) -> World {
    let mut w = World::default();
    for i in 0..CHILDREN {
        let f = |n: usize| flags & (1 << (i * 3 + n)) != 0;
        // child 4 is the transition itself, child 3 holds it; some fail
        w.child(Child {
            fail_add: f(0) && f(1),
            is_self: i == 4,
            contains: i == 3 && f(2),
            ..Child::default()
        });
    }
    let t0 = w.tr(Some(0));
    let t1 = w.tr(None);
    w.trs[t0].reenters = flags & (1 << 40) != 0;
    w.trs[t0].nested_mode = if flags & (1 << 41) != 0 { MANUAL } else { AUTO };
    w.trs[t0].nested_dest = Some(usize::try_from((flags >> 42) % 3).unwrap());
    w.trs[t0].nested_ms = 900;
    w.trs[t0].use_fixed = flags & (1 << 45) != 0;
    w.trs[t0].fixed_ms = 77;
    w.trs[t1].cx = if flags & (1 << 46) != 0 { 0 } else { 1920 };
    w.trs[t1].cy = if flags & (1 << 47) != 0 { 0 } else { 1080 };
    w
}

fn run(w: &mut World, r: Rules, ops: &[Op]) -> Vec<String> {
    let mut out = Vec::new();
    for op in ops {
        let res = match *op {
            Op::Start { t, mode, ms, dest } => format!("{}", w.start(r, t, mode, ms, dest)),
            Op::Set { t, source } => {
                w.set(r, t, source);
                String::new()
            }
            Op::ManualTime { t, f } => {
                w.set_manual_time(r, t, f);
                String::new()
            }
            Op::GetActive { t } => format!("{:?}", w.get_active(r, t)),
            Op::Swap { dest, source } => {
                w.swap(r, dest, source);
                String::new()
            }
        };
        out.push(res);
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn edge_proptest_values(
        flags in any::<u8>(),
        mode in any::<i32>(),
        ms in any::<u32>(),
        size in any::<u32>(),
        bits in any::<u32>(),
    ) {
        let f = |n: u32| flags & (1 << n) != 0;
        prop_assert_eq!(admit(f(0), f(1), f(2), mode, f(3)), model_admit(f(0), f(1), f(2), mode, f(3)));
        prop_assert_eq!(u128::from(duration_ns(ms)), u128::from(ms) * 1_000_000);
        prop_assert_eq!(is_unsized(size), size == 0);
        let t = f32::from_bits(bits);
        #[allow(clippy::float_cmp)]
        let want = mode == 1 && t == 0.0;
        prop_assert_eq!(manual_reset(mode, t), want);
        prop_assert_eq!(manual_reset(MANUAL, t), bits & 0x7fff_ffff == 0);
    }

    #[test]
    fn edge_proptest_sequences(flags in any::<u64>(), ops in prop::collection::vec(op(), 1..12)) {
        let mut core = world(flags);
        let mut inline = world(flags);
        let a = run(&mut core, Rules::Core, &ops);
        let b = run(&mut inline, Rules::Inline, &ops);
        prop_assert_eq!(a, b);
        // by the debug form: a NaN manual target is equal to itself there
        prop_assert_eq!(format!("{core:?}"), format!("{inline:?}"));
    }
}
