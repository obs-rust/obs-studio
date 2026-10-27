//! Tier 1: `obs_source::transition_load`, the load, scale type and swap
//! checks of `transition_valid()`, `obs_transition_set_scale_type()`,
//! `obs_transition_load()`, `copy_source_state()` and
//! `obs_transition_swap_begin()` / `_end()`.
//!
//! The `steps_*` cases port `test/cmocka/test_transition_load_steps.c` 1:1:
//! its 10 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones and the
//! copies of the callers on a stand-in transition included. The C calls the
//! hidden `obs_source_transition_load_*()` functions; here the safe core is
//! called directly.
//!
//! The `edge_*` cases are new: every decision over every combination of its
//! flags against models of the original C expressions, the type check over
//! -100,000..100,000 and the ends of `i32`, the scale type at every power of
//! two, its neighbours and negations, a stride over `i64` and the ends, and
//! proptests: full-range values, and operation sequences (loads, swaps, the
//! setter and the getters) through the stand-in against a copy of the
//! callers with origin/master's expressions inline.

use obs_source::transition_load::{
    SCALE_ASPECT, SCALE_MAX_ONLY, SCALE_STRETCH, Source, TYPE_TRANSITION, copy_stores,
    is_transition, looks_up, scale_type, source, swaps,
};
use obs_source::transition_start::child_active;
use proptest::prelude::*;

const TYPE_INPUT: i32 = 0;
const TYPE_FILTER: i32 = 1;
const TYPE_SCENE: i32 = 3;
const ALIGN_LEFT_TOP: u32 = 1 | 4; // OBS_ALIGN_LEFT | OBS_ALIGN_TOP
const ALIGN_CENTER: u32 = 0;
const ALIGN_RIGHT: u32 = 2;

fn bit(bits: u32, n: u32) -> bool {
    bits & (1 << n) != 0
}

/// origin/master's `valid_scale_type()`.
fn inline_scale_type(value: i64) -> i32 {
    match value {
        0..=2 => i32::try_from(value).unwrap(),
        _ => SCALE_MAX_ONLY,
    }
}

#[test]
fn steps_constants() {
    // the header's mirror of libobs
    assert_eq!(TYPE_TRANSITION, 2);
    assert_eq!(SCALE_MAX_ONLY, 0);
    assert_eq!(SCALE_ASPECT, 1);
    assert_eq!(SCALE_STRETCH, 2);
    assert_eq!(SCALE_MAX_ONLY, obs_source::transition_clock::SCALE_MAX_ONLY);
    assert_eq!(SCALE_ASPECT, obs_source::transition_clock::SCALE_ASPECT);
    assert_eq!(SCALE_STRETCH, obs_source::transition_clock::SCALE_STRETCH);
    assert_eq!(Source::Keep as i32, 0);
    assert_eq!(Source::NotFound as i32, 1);
    assert_eq!(Source::Refuse as i32, 2);
}

#[test]
fn steps_is_transition() {
    assert!(is_transition(TYPE_TRANSITION));
    assert!(!is_transition(TYPE_INPUT));
    assert!(!is_transition(TYPE_FILTER));
    assert!(!is_transition(TYPE_SCENE));

    for t in -1000..=1000 {
        assert_eq!(is_transition(t), t == 2, "is_transition({t})");
    }
    assert!(!is_transition(i32::MIN));
    assert!(!is_transition(i32::MAX));
    assert!(!is_transition(2 + 256));
}

#[test]
fn steps_scale_type() {
    assert_eq!(scale_type(0), SCALE_MAX_ONLY);
    assert_eq!(scale_type(1), SCALE_ASPECT);
    assert_eq!(scale_type(2), SCALE_STRETCH);

    assert_eq!(scale_type(3), SCALE_MAX_ONLY);
    assert_eq!(scale_type(-1), SCALE_MAX_ONLY);
    assert_eq!(scale_type(i64::MAX), SCALE_MAX_ONLY);
    assert_eq!(scale_type(i64::MIN), SCALE_MAX_ONLY);
    assert_eq!(scale_type(i64::from(i32::MAX)), SCALE_MAX_ONLY);
    assert_eq!(scale_type(i64::from(i32::MIN)), SCALE_MAX_ONLY);

    // compared in 64 bits
    assert_eq!(scale_type((1_i64 << 32) + 1), SCALE_MAX_ONLY);
    assert_eq!(scale_type((1_i64 << 32) + 2), SCALE_MAX_ONLY);
    assert_eq!(scale_type(-(1_i64 << 32) + 1), SCALE_MAX_ONLY);
    assert_eq!(scale_type(i64::from(u32::MAX) + 3), SCALE_MAX_ONLY);

    for v in -1000_i64..=1000 {
        let want = if v == 1 || v == 2 {
            i32::try_from(v).unwrap()
        } else {
            SCALE_MAX_ONLY
        };
        assert_eq!(scale_type(v), want, "scale_type({v})");
    }
}

#[test]
fn steps_load_source_steps() {
    assert!(looks_up(true));
    assert!(!looks_up(false));

    assert_eq!(source(false, false), Source::NotFound);
    assert_eq!(source(false, true), Source::NotFound);
    assert_eq!(source(true, false), Source::Refuse);
    assert_eq!(source(true, true), Source::Keep);
}

#[test]
fn steps_swap_and_copy_steps() {
    assert!(swaps(false));
    assert!(!swaps(true));

    assert!(copy_stores(false, false));
    assert!(copy_stores(false, true));
    assert!(!copy_stores(true, false));
    assert!(copy_stores(true, true));
}

/// Which rules the stand-in asks: the core, or origin/master's expressions
/// inline.
#[derive(Debug, Clone, Copy)]
enum Rules {
    Core,
    Inline,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Child {
    name: &'static str,
    contains: bool,
    refs: i32,
    active_refs: i32,
}

type ChildId = usize;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Tr {
    source_type: i32,
    src: [Option<ChildId>; 2],
    slot_active: [bool; 2],
    alignment: u32,
    cx: u32,
    cy: u32,
    mode: i32,
    scale: i32,
    fixed: bool,
    warnings: i32,
    resizes: i32,
}

impl Tr {
    /// a transition as `obs_transition_init()` leaves it
    fn init() -> Self {
        Self {
            source_type: TYPE_TRANSITION,
            src: [None, None],
            slot_active: [true, false],
            alignment: ALIGN_LEFT_TOP,
            cx: 0,
            cy: 0,
            mode: 0,
            scale: SCALE_MAX_ONLY,
            fixed: false,
            warnings: 0,
            resizes: 0,
        }
    }
}

/// The values `obs_transition_load()` reads from its `obs_data`.
#[derive(Debug, Clone, Copy)]
struct Saved {
    name: Option<&'static str>,
    alignment: i64,
    mode: i64,
    scale: i64,
    cx: i64,
    cy: i64,
}

impl Saved {
    fn named(name: Option<&'static str>) -> Self {
        Self {
            name,
            alignment: 0,
            mode: 0,
            scale: 0,
            cx: 0,
            cy: 0,
        }
    }
}

/// A stand-in world: transitions and children that count their references
/// and active references, as in the C test.
#[derive(Debug, Clone, PartialEq, Eq)]
struct World {
    trs: Vec<Tr>,
    children: Vec<Child>,
}

/// the C's `(uint32_t)` of the loaded `int64_t`
const fn trunc32(v: i64) -> u32 {
    v as u32
}

/// the C's `(enum obs_transition_mode)` of the loaded `int64_t`
const fn trunc_mode(v: i64) -> i32 {
    v as i32
}

impl World {
    fn new(children: Vec<Child>) -> Self {
        Self {
            trs: Vec::new(),
            children,
        }
    }

    fn lookup(&mut self, name: &str) -> Option<ChildId> {
        let id = self.children.iter().position(|c| c.name == name)?;
        self.children[id].refs += 1;
        Some(id)
    }

    fn get_ref(&mut self, c: Option<ChildId>) -> Option<ChildId> {
        if let Some(id) = c {
            self.children[id].refs += 1;
        }
        c
    }

    fn release(&mut self, c: Option<ChildId>) {
        if let Some(id) = c {
            self.children[id].refs -= 1;
        }
    }

    fn add_active(&mut self, c: ChildId) -> bool {
        if self.children[c].contains {
            return false;
        }
        self.children[c].active_refs += 1;
        true
    }

    fn remove_active(&mut self, c: Option<ChildId>) {
        if let Some(id) = c {
            self.children[id].active_refs -= 1;
        }
    }

    /// `transition_valid()` past `obs_ptr_valid()`
    fn valid(&self, r: Rules, t: Option<usize>) -> bool {
        t.is_some_and(|t| {
            let ty = self.trs[t].source_type;
            match r {
                Rules::Core => is_transition(ty),
                Rules::Inline => ty == 2,
            }
        })
    }

    fn scale(r: Rules, v: i64) -> i32 {
        match r {
            Rules::Core => scale_type(v),
            Rules::Inline => inline_scale_type(v),
        }
    }

    /// `obs_transition_load()`
    fn load(&mut self, r: Rules, t: usize, saved: Saved) {
        let mut src = None;
        let look = match r {
            Rules::Core => looks_up(saved.name.is_some()),
            Rules::Inline => saved.name.is_some(),
        };
        if look {
            src = self.lookup(saved.name.unwrap_or(""));
            match r {
                Rules::Core => {
                    let found = src.is_some();
                    let add_ok = found && src.is_some_and(|c| self.add_active(c));
                    match source(found, add_ok) {
                        Source::Refuse => {
                            self.trs[t].warnings += 1;
                            self.release(src);
                            src = None;
                        }
                        Source::NotFound => self.trs[t].warnings += 1,
                        Source::Keep => {}
                    }
                }
                Rules::Inline => {
                    if let Some(c) = src {
                        if !self.add_active(c) {
                            self.trs[t].warnings += 1;
                            self.release(src);
                            src = None;
                        }
                    } else {
                        self.trs[t].warnings += 1;
                    }
                }
            }
        }

        let tr = &mut self.trs[t];
        let old = tr.src[0];
        let old_active = tr.slot_active[0];
        tr.src[0] = src;
        tr.slot_active[0] = true;
        tr.alignment = trunc32(saved.alignment);
        tr.mode = trunc_mode(saved.mode);
        tr.scale = Self::scale(r, saved.scale);
        tr.cx = trunc32(saved.cx);
        tr.cy = trunc32(saved.cy);

        let out = match r {
            Rules::Core => child_active(old.is_some(), old_active),
            Rules::Inline => old.is_some() && old_active,
        };
        if out {
            self.remove_active(old);
        }
        self.release(old);
        self.trs[t].resizes += 1;
    }

    /// `copy_source_state()`
    fn copy_state(&mut self, r: Rules, dest: usize, src_t: usize, idx: usize) -> Option<ChildId> {
        let old_child = self.trs[dest].src[idx];
        let new_child = self.get_ref(self.trs[src_t].src[idx]);
        let mut active = self.trs[src_t].slot_active[idx];

        if old_child.is_some() && self.trs[dest].slot_active[idx] {
            self.remove_active(old_child);
        }

        let adds = child_active(new_child.is_some(), active);
        let stores = match r {
            Rules::Core => {
                let add_ok = adds && new_child.is_some_and(|c| self.add_active(c));
                copy_stores(adds, add_ok)
            }
            Rules::Inline => !(adds && !new_child.is_some_and(|c| self.add_active(c))),
        };
        let mut stored = new_child;
        if !stores {
            self.trs[dest].warnings += 1;
            self.release(new_child);
            stored = None;
            active = false;
        }
        self.trs[dest].src[idx] = stored;
        self.trs[dest].slot_active[idx] = active;
        old_child
    }

    fn gate(r: Rules, same: bool) -> bool {
        match r {
            Rules::Core => swaps(same),
            Rules::Inline => !same,
        }
    }

    /// `obs_transition_swap_begin()`
    fn swap_begin(&mut self, r: Rules, dest: usize, src_t: usize) {
        if !Self::gate(r, dest == src_t) {
            return;
        }
        let old = [
            self.copy_state(r, dest, src_t, 0),
            self.copy_state(r, dest, src_t, 1),
        ];
        self.trs[dest].mode = self.trs[src_t].mode;
        for o in old {
            self.release(o);
        }
    }

    /// `obs_transition_swap_end()`, whose `obs_transition_clear()` empties
    /// the source transition
    fn swap_end(&mut self, r: Rules, dest: usize, src_t: usize) {
        if !Self::gate(r, dest == src_t) {
            return;
        }
        for i in 0..2 {
            let s = self.trs[src_t].src[i];
            let active = self.trs[src_t].slot_active[i];
            self.trs[src_t].src[i] = None;
            self.trs[src_t].slot_active[i] = false;
            if s.is_some() && active {
                self.remove_active(s);
            }
            self.release(s);
        }
    }

    fn swap(&mut self, r: Rules, dest: usize, src_t: usize) {
        self.swap_begin(r, dest, src_t);
        self.swap_end(r, dest, src_t);
    }

    /// `obs_transition_set_scale_type()`
    fn set_scale_type(&mut self, r: Rules, t: Option<usize>, v: i32) {
        if !self.valid(r, t) {
            return;
        }
        if let Some(t) = t {
            self.trs[t].scale = Self::scale(r, i64::from(v));
        }
    }

    /// `obs_transition_get_scale_type()`, `_get_alignment()`, `_get_size()`
    /// and `_fixed()`
    fn getters(&self, r: Rules, t: Option<usize>) -> (i32, u32, (u32, u32), bool) {
        match t {
            Some(t) if self.valid(r, Some(t)) => {
                let tr = &self.trs[t];
                (tr.scale, tr.alignment, (tr.cx, tr.cy), tr.fixed)
            }
            _ => (SCALE_MAX_ONLY, 0, (0, 0), false),
        }
    }
}

fn child(name: &'static str, contains: bool) -> Child {
    Child {
        name,
        contains,
        refs: 1,
        active_refs: 0,
    }
}

#[test]
fn steps_load_fields() {
    let mut w = World::new(Vec::new());
    w.trs.push(Tr::init());
    let r = Rules::Core;

    w.load(
        r,
        0,
        Saved {
            name: None,
            alignment: i64::from(ALIGN_CENTER),
            mode: 1,
            scale: 2,
            cx: 1920,
            cy: 1080,
        },
    );
    let t = &w.trs[0];
    assert_eq!(t.src[0], None);
    assert!(t.slot_active[0]);
    assert_eq!(t.alignment, ALIGN_CENTER);
    assert_eq!(t.mode, 1);
    assert_eq!(t.scale, SCALE_STRETCH);
    assert_eq!((t.cx, t.cy), (1920, 1080));
    assert_eq!(t.warnings, 0);
    assert_eq!(t.resizes, 1);

    // characterized, not endorsed: truncated to the low 32 bits
    w.load(
        r,
        0,
        Saved {
            name: None,
            alignment: (1_i64 << 32) + 5,
            mode: 0,
            scale: 1,
            cx: (1_i64 << 32) + 640,
            cy: -1,
        },
    );
    let t = &w.trs[0];
    assert_eq!(t.alignment, 5);
    assert_eq!(t.cx, 640);
    assert_eq!(t.cy, u32::MAX);
    assert_eq!(t.scale, SCALE_ASPECT);
    w.load(
        r,
        0,
        Saved {
            name: None,
            alignment: -2,
            mode: 0,
            scale: 0,
            cx: i64::MAX,
            cy: i64::MIN,
        },
    );
    let t = &w.trs[0];
    assert_eq!(t.alignment, u32::MAX - 1);
    assert_eq!(t.cx, u32::MAX);
    assert_eq!(t.cy, 0);
    assert_eq!(t.scale, SCALE_MAX_ONLY);

    // characterized, not endorsed: a mode out of the enum is kept
    w.load(
        r,
        0,
        Saved {
            mode: 7,
            ..Saved::named(None)
        },
    );
    assert_eq!(w.trs[0].mode, 7);

    for v in [3, -1, i64::MAX, (1_i64 << 32) + 2] {
        w.load(
            r,
            0,
            Saved {
                scale: v,
                ..Saved::named(None)
            },
        );
        assert_eq!(w.trs[0].scale, SCALE_MAX_ONLY, "scale {v}");
    }
    assert_eq!(w.trs[0].warnings, 0);
}

#[test]
fn steps_load_sources() {
    const A: usize = 0;
    const B: usize = 1;
    const SELF: usize = 2;
    let mut w = World::new(vec![
        child("A", false),
        child("B", false),
        child("self", true),
    ]);
    w.trs.push(Tr::init());
    let r = Rules::Core;

    // a missing name: none, no warning
    w.load(r, 0, Saved::named(None));
    assert_eq!(w.trs[0].src[0], None);
    assert_eq!(w.trs[0].warnings, 0);

    // an unknown name: none and a warning
    w.load(r, 0, Saved::named(Some("nope")));
    assert_eq!(w.trs[0].src[0], None);
    assert!(w.trs[0].slot_active[0]);
    assert_eq!(w.trs[0].warnings, 1);

    // an empty name is still looked up
    w.load(r, 0, Saved::named(Some("")));
    assert_eq!(w.trs[0].src[0], None);
    assert_eq!(w.trs[0].warnings, 2);

    // a known source: kept, with a reference and an active reference
    w.load(r, 0, Saved::named(Some("A")));
    assert_eq!(w.trs[0].src[0], Some(A));
    assert_eq!(w.children[A].refs, 2);
    assert_eq!(w.children[A].active_refs, 1);
    assert_eq!(w.trs[0].warnings, 2);

    // a source that contains the transition: released, none and a
    // warning; the old source A still goes out
    w.load(r, 0, Saved::named(Some("self")));
    assert_eq!(w.trs[0].src[0], None);
    assert_eq!(w.children[SELF].refs, 1);
    assert_eq!(w.children[SELF].active_refs, 0);
    assert_eq!(w.children[A].refs, 1);
    assert_eq!(w.children[A].active_refs, 0);
    assert_eq!(w.trs[0].warnings, 3);

    // a load onto a slot A that holds a source takes it back
    w.load(r, 0, Saved::named(Some("A")));
    w.load(r, 0, Saved::named(Some("B")));
    assert_eq!(w.trs[0].src[0], Some(B));
    assert_eq!(w.children[A].refs, 1);
    assert_eq!(w.children[A].active_refs, 0);
    assert_eq!(w.children[B].refs, 2);
    assert_eq!(w.children[B].active_refs, 1);

    // the same source again: one reference and one active reference
    w.load(r, 0, Saved::named(Some("B")));
    assert_eq!(w.children[B].refs, 2);
    assert_eq!(w.children[B].active_refs, 1);

    // an inactive slot A: its source is released without a removal
    w.trs[0].slot_active[0] = false;
    w.children[B].active_refs -= 1;
    w.load(r, 0, Saved::named(None));
    assert_eq!(w.trs[0].src[0], None);
    assert!(w.trs[0].slot_active[0]);
    assert_eq!(w.children[B].refs, 1);
    assert_eq!(w.children[B].active_refs, 0);
}

#[test]
fn steps_swap_with_itself() {
    let mut w = World::new(vec![child("A", false)]);
    w.trs.push(Tr::init());
    w.trs[0].src[0] = w.get_ref(Some(0));
    assert!(w.add_active(0));
    w.trs[0].mode = 1;
    w.swap(Rules::Core, 0, 0);
    assert_eq!(w.trs[0].src[0], Some(0));
    assert!(w.trs[0].slot_active[0]);
    assert_eq!(w.children[0].refs, 2);
    assert_eq!(w.children[0].active_refs, 1);
    assert_eq!(w.trs[0].mode, 1);
}

#[test]
fn steps_swap_slot_states() {
    for bits in 0..32 {
        let has_old = bit(bits, 0);
        let old_active = bit(bits, 1);
        let has_new = bit(bits, 2);
        let new_active = bit(bits, 3);
        let contains = bit(bits, 4);
        const O: usize = 0;
        const N: usize = 1;
        let mut w = World::new(vec![child("O", false), child("N", contains)]);
        w.trs.push(Tr::init());
        w.trs.push(Tr::init());
        let (dest, src) = (0, 1);
        if has_old {
            w.trs[dest].src[1] = w.get_ref(Some(O));
            if old_active {
                assert!(w.add_active(O));
            }
        }
        w.trs[dest].slot_active[1] = old_active;
        if has_new {
            w.trs[src].src[1] = w.get_ref(Some(N));
            // the source's active slot holds an active reference
            if new_active {
                w.children[N].active_refs += 1;
            }
        }
        w.trs[src].slot_active[1] = new_active;

        w.swap(Rules::Core, dest, src);

        let refused = has_new && new_active && contains;
        assert_eq!(w.children[O].refs, 1, "bits {bits}");
        assert_eq!(w.children[O].active_refs, 0, "bits {bits}");
        assert_eq!(w.trs[src].src[1], None);
        assert!(!w.trs[src].slot_active[1]);
        if refused {
            assert_eq!(w.trs[dest].src[1], None);
            assert!(!w.trs[dest].slot_active[1]);
            assert_eq!(w.trs[dest].warnings, 1);
            assert_eq!(w.children[N].refs, 1);
            assert_eq!(w.children[N].active_refs, 0);
        } else {
            assert_eq!(w.trs[dest].src[1], has_new.then_some(N), "bits {bits}");
            assert_eq!(w.trs[dest].slot_active[1], new_active);
            assert_eq!(w.trs[dest].warnings, 0);
            assert_eq!(w.children[N].refs, if has_new { 2 } else { 1 });
            assert_eq!(
                w.children[N].active_refs,
                i32::from(has_new && new_active),
                "bits {bits}"
            );
        }
    }
}

#[test]
fn steps_getters() {
    let types = [TYPE_INPUT, TYPE_FILTER, TYPE_TRANSITION, TYPE_SCENE, -1, 99];
    let r = Rules::Core;
    for ty in types {
        let valid = ty == TYPE_TRANSITION;
        let mut w = World::new(Vec::new());
        let mut t = Tr::init();
        t.source_type = ty;
        t.scale = SCALE_STRETCH;
        t.alignment = ALIGN_RIGHT;
        t.cx = 640;
        t.cy = 480;
        t.fixed = true;
        w.trs.push(t);

        let (scale, alignment, size, fixed) = w.getters(r, Some(0));
        assert_eq!(
            scale,
            if valid { SCALE_STRETCH } else { SCALE_MAX_ONLY },
            "type {ty}"
        );
        assert_eq!(alignment, if valid { ALIGN_RIGHT } else { 0 });
        assert_eq!(size, if valid { (640, 480) } else { (0, 0) });
        assert_eq!(fixed, valid);

        // the setter writes only a transition, and only a valid value
        w.set_scale_type(r, Some(0), SCALE_ASPECT);
        assert_eq!(
            w.trs[0].scale,
            if valid { SCALE_ASPECT } else { SCALE_STRETCH }
        );
        w.set_scale_type(r, Some(0), 3);
        assert_eq!(
            w.trs[0].scale,
            if valid { SCALE_MAX_ONLY } else { SCALE_STRETCH }
        );
        w.set_scale_type(r, Some(0), -1);
        assert_eq!(
            w.trs[0].scale,
            if valid { SCALE_MAX_ONLY } else { SCALE_STRETCH }
        );
    }

    // no source at all
    let w = World::new(Vec::new());
    assert_eq!(w.getters(r, None), (SCALE_MAX_ONLY, 0, (0, 0), false));
}

#[test]
fn edge_flags_every_combination() {
    for bits in 0..4 {
        let (a, b) = (bit(bits, 0), bit(bits, 1));
        let want = if !a {
            Source::NotFound
        } else if !b {
            Source::Refuse
        } else {
            Source::Keep
        };
        assert_eq!(source(a, b), want, "source({a}, {b})");
        assert_eq!(copy_stores(a, b), !(a && !b), "copy_stores({a}, {b})");
    }
    for a in [false, true] {
        assert_eq!(looks_up(a), a);
        assert_eq!(swaps(a), !a);
    }
}

#[test]
fn edge_is_transition_range() {
    for t in -100_000..=100_000 {
        assert_eq!(is_transition(t), t == 2, "is_transition({t})");
    }
    for t in [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX] {
        assert!(!is_transition(t));
    }
    // 2 with any other bit set is not a transition
    for shift in (0..31).filter(|&s| s != 1) {
        assert!(!is_transition(2 | (1 << shift)), "2 | 1 << {shift}");
    }
}

#[test]
fn edge_scale_type_powers_and_stride() {
    let mut values = vec![i64::MIN, i64::MIN + 1, i64::MAX - 1, i64::MAX];
    for shift in 0..63 {
        let p = 1_i64 << shift;
        values.extend([p - 1, p, p + 1, p + 2, -p, -p + 1, -p + 2]);
    }
    let mut v = i64::MIN;
    while let Some(next) = v.checked_add(i64::MAX / 4093) {
        values.push(v);
        values.push(v.wrapping_add(1));
        values.push(v.wrapping_add(2));
        v = next;
    }
    for v in values {
        assert_eq!(scale_type(v), inline_scale_type(v), "scale_type({v})");
    }
}

/// One operation of a sequence through the stand-in.
#[derive(Debug, Clone)]
enum Op {
    Load(usize, Saved),
    Swap(usize, usize),
    SwapBegin(usize, usize),
    SwapEnd(usize, usize),
    SetScale(Option<usize>, i32),
    Get(Option<usize>),
}

static NAMES: [&str; 4] = ["c0", "c1", "c2", "nope"];

fn any_saved() -> impl Strategy<Value = Saved> {
    (
        prop::option::of(prop::sample::select(&NAMES[..])),
        any::<i64>(),
        prop_oneof![Just(0_i64), Just(1), Just(7), any::<i64>()],
        prop_oneof![-1_i64..4, any::<i64>()],
        any::<i64>(),
        any::<i64>(),
    )
        .prop_map(|(name, alignment, mode, scale, cx, cy)| Saved {
            name,
            alignment,
            mode,
            scale,
            cx,
            cy,
        })
}

fn any_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0_usize..3, any_saved()).prop_map(|(t, s)| Op::Load(t, s)),
        (0_usize..3, 0_usize..3).prop_map(|(a, b)| Op::Swap(a, b)),
        (0_usize..3, 0_usize..3).prop_map(|(a, b)| Op::SwapBegin(a, b)),
        (0_usize..3, 0_usize..3).prop_map(|(a, b)| Op::SwapEnd(a, b)),
        (
            prop::option::of(0_usize..3),
            prop_oneof![-1_i32..4, any::<i32>()]
        )
            .prop_map(|(t, v)| Op::SetScale(t, v)),
        prop::option::of(0_usize..3).prop_map(Op::Get),
    ]
}

fn world(contains: [bool; 3], types: [i32; 3]) -> World {
    let mut w = World::new(vec![
        child("c0", contains[0]),
        child("c1", contains[1]),
        child("c2", contains[2]),
    ]);
    for ty in types {
        let mut t = Tr::init();
        t.source_type = ty;
        w.trs.push(t);
    }
    w
}

fn run(w: &mut World, r: Rules, ops: &[Op]) -> Vec<(i32, u32, (u32, u32), bool)> {
    let mut seen = Vec::new();
    for op in ops {
        match *op {
            Op::Load(t, s) => w.load(r, t, s),
            Op::Swap(a, b) => w.swap(r, a, b),
            Op::SwapBegin(a, b) => w.swap_begin(r, a, b),
            Op::SwapEnd(a, b) => w.swap_end(r, a, b),
            Op::SetScale(t, v) => w.set_scale_type(r, t, v),
            Op::Get(t) => seen.push(w.getters(r, t)),
        }
    }
    seen
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn edge_full_range(t in any::<i32>(), v in any::<i64>(), a in any::<bool>(), b in any::<bool>()) {
        prop_assert_eq!(is_transition(t), t == 2);
        prop_assert_eq!(scale_type(v), inline_scale_type(v));
        prop_assert_eq!(looks_up(a), a);
        prop_assert_eq!(swaps(a), !a);
        prop_assert_eq!(copy_stores(a, b), !(a && !b));
        let want = if !a { Source::NotFound } else if b { Source::Keep } else { Source::Refuse };
        prop_assert_eq!(source(a, b), want);
    }

    #[test]
    fn edge_sequences_match_master(
        contains in any::<[bool; 3]>(),
        types in prop::array::uniform3(prop_oneof![Just(TYPE_TRANSITION), -1_i32..5]),
        ops in prop::collection::vec(any_op(), 1..12),
    ) {
        let mut core = world(contains, types);
        let mut inline = core.clone();
        let seen_core = run(&mut core, Rules::Core, &ops);
        let seen_inline = run(&mut inline, Rules::Inline, &ops);
        prop_assert_eq!(seen_core, seen_inline);
        prop_assert_eq!(&core, &inline);
        // every reference the transitions hold is counted once
        for (id, c) in core.children.iter().enumerate() {
            let held = core.trs.iter().flat_map(|t| t.src).filter(|s| *s == Some(id)).count();
            prop_assert_eq!(c.refs, 1 + i32::try_from(held).unwrap(), "child {}", id);
        }
    }
}
