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
#[expect(
    clippy::cognitive_complexity,
    reason = "scenario table: one block per ported step"
)]
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

/// Tier 1 for `obs_source::transition_load::load`, `obs_transition_load()`
/// of `libobs/obs-source-transition-restore.c.h`.
///
/// The load runs on a fake transition over a fake source graph: saved data
/// (the string the name key reads, and the ints of the other keys, 0 when
/// missing, as `obs_data_get_int()` gives them), sources found by name with
/// a reference count, the children that refuse the active-child add, and
/// slot A with its active flag. The fake records every `obs_data` read,
/// lookup, active-child call, release, warning, lock and field write in
/// order, and panics on a field written or slot A read without
/// `transition_mutex`. The size and matrix recalculation after the unlock
/// are the sizing port's, and show as their lock and child-size calls.
///
/// The `c_*` cases follow `test_source_transition_restore.c`; the others
/// pin the casts, each source outcome, the old slot A's hand-back and the
/// order of every call, with a proptest against a copy of the C.
mod load {
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeMap;
    use std::ffi::CStr;

    use obs_c_oracle::parity::cases;
    use obs_source::transition_load::{
        self as core, KEY_ALIGNMENT, KEY_CX, KEY_CY, KEY_MODE, KEY_SCALE_TYPE, KEY_SOURCE_A,
        Transition, load,
    };
    use obs_source::transition_size;
    use proptest::prelude::*;

    type Id = u32;
    type Name = &'static str;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Ev {
        GetString(&'static CStr),
        GetInt(&'static CStr),
        ByName(Name),
        Add(Id, bool),
        Remove(Id),
        Release(Option<Id>),
        WarnRecursion(Name),
        WarnNotFound(Name),
        Lock,
        Unlock,
        SourceActive,
        SetSource(Option<Id>),
        SetSourceActive(bool),
        SetAlignment(u32),
        SetMode(i32),
        SetScaleType(i32),
        SetCx(u32),
        SetCy(u32),
        Width(Id),
        Height(Id),
        SetActual(u32, u32),
        Matrix(usize),
    }

    /// What the saved data reads give.
    #[derive(Debug, Clone, Default)]
    struct Data {
        name: Option<Name>,
        ints: BTreeMap<&'static CStr, i64>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    struct Fields {
        source: Option<Id>,
        active: bool,
        alignment: u32,
        mode: i32,
        scale_type: i32,
        cx: u32,
        cy: u32,
        actual: (u32, u32),
    }

    #[derive(Default)]
    struct Tr {
        f: RefCell<Fields>,
        /// Sources by name, each 100x50 times its id.
        names: BTreeMap<Name, Id>,
        refs: RefCell<BTreeMap<Id, i64>>,
        links: RefCell<BTreeMap<Id, i64>>,
        refuses: Vec<Id>,
        locked: Cell<bool>,
        events: RefCell<Vec<Ev>>,
    }

    impl Tr {
        fn new(names: &[(Name, Id)]) -> Self {
            let mut tr = Self::default();
            for &(n, id) in names {
                tr.names.insert(n, id);
                tr.refs.borrow_mut().insert(id, 1);
            }
            tr
        }
        /// Slot A holds `id` with the transition's reference and, when
        /// `active`, its active-child link.
        fn hold(&self, id: Id, active: bool) {
            *self.refs.borrow_mut().entry(id).or_insert(0) += 1;
            if active {
                *self.links.borrow_mut().entry(id).or_insert(0) += 1;
            }
            let mut f = self.f.borrow_mut();
            f.source = Some(id);
            f.active = active;
        }
        fn ev(&self, e: Ev) {
            self.events.borrow_mut().push(e);
        }
        fn need_lock(&self, what: &str) {
            assert!(self.locked.get(), "{what} without transition_mutex");
        }
        fn events(&self) -> Vec<Ev> {
            std::mem::take(&mut self.events.borrow_mut())
        }
        fn fields(&self) -> Fields {
            *self.f.borrow()
        }
        fn refs(&self, id: Id) -> i64 {
            self.refs.borrow().get(&id).copied().unwrap_or(0)
        }
        fn link(&self, id: Id) -> i64 {
            self.links.borrow().get(&id).copied().unwrap_or(0)
        }
    }

    impl transition_size::Transition for Tr {
        type Child = Id;
        type Texrender = ();

        fn source(&self, idx: usize) -> Option<Id> {
            self.need_lock("transition_sources");
            if idx == 0 {
                self.f.borrow().source
            } else {
                None
            }
        }
        fn cx(&self) -> u32 {
            self.f.borrow().cx
        }
        fn cy(&self) -> u32 {
            self.f.borrow().cy
        }
        fn actual_cx(&self) -> u32 {
            self.f.borrow().actual.0
        }
        fn set_actual_cx(&self, value: u32) {
            self.f.borrow_mut().actual.0 = value;
        }
        fn actual_cy(&self) -> u32 {
            self.f.borrow().actual.1
        }
        fn set_actual_cy(&self, value: u32) {
            self.f.borrow_mut().actual.1 = value;
            let a = self.f.borrow().actual;
            self.ev(Ev::SetActual(a.0, a.1));
        }
        fn scale_type(&self) -> i32 {
            self.f.borrow().scale_type
        }
        fn alignment(&self) -> u32 {
            self.f.borrow().alignment
        }
        fn mode(&self) -> i32 {
            self.f.borrow().mode
        }
        fn manual_val(&self) -> f32 {
            panic!("the load reads no manual value")
        }
        fn set_manual_val(&self, _: f32) {
            panic!("the load writes no manual value")
        }
        fn manual_target(&self) -> f32 {
            panic!("the load reads no manual target")
        }
        fn manual_torque(&self) -> f32 {
            panic!("the load reads no manual torque")
        }
        fn manual_clamp(&self) -> f32 {
            panic!("the load reads no manual clamp")
        }
        fn texrender(&self, _: usize) {
            panic!("the load touches no render target")
        }
        fn lock(&self) {
            assert!(!self.locked.get(), "transition_mutex taken twice");
            self.locked.set(true);
            self.ev(Ev::Lock);
        }
        fn unlock(&self) {
            assert!(self.locked.get(), "transition_mutex unlocked unheld");
            self.locked.set(false);
            self.ev(Ev::Unlock);
        }
        fn trylock_textures(&self) -> i32 {
            panic!("the load takes no texture lock")
        }
        fn unlock_textures(&self) {
            panic!("the load takes no texture lock")
        }
        fn child_width(&self, child: Id) -> u32 {
            self.ev(Ev::Width(child));
            100 * child
        }
        fn child_height(&self, child: Id) -> u32 {
            self.ev(Ev::Height(child));
            50 * child
        }
        fn alignment_position(&self, _: u32, cx: i32, cy: i32) -> (f32, f32) {
            (cx as f32, cy as f32)
        }
        fn set_matrix(&self, idx: usize, _: (f32, f32), _: (f32, f32)) {
            self.ev(Ev::Matrix(idx));
        }
        fn calc_torquef(&self, _: f32, _: f32, _: f32, _: f32, _: f32) -> f32 {
            panic!("the load calls no calc_torquef()")
        }
        fn texrender_reset(&self, _: ()) {
            panic!("the load resets no render target")
        }
    }

    impl Transition for Tr {
        type Data = Data;
        type Name = Name;

        fn get_string(&self, data: &Data, key: &'static CStr) -> Option<Name> {
            self.ev(Ev::GetString(key));
            assert_eq!(key, KEY_SOURCE_A);
            data.name
        }
        fn get_int(&self, data: &Data, key: &'static CStr) -> i64 {
            self.ev(Ev::GetInt(key));
            data.ints.get(key).copied().unwrap_or(0)
        }
        fn source_by_name(&self, name: Name) -> Option<Id> {
            assert!(!self.locked.get(), "lookup under transition_mutex");
            self.ev(Ev::ByName(name));
            let id = self.names.get(name).copied()?;
            *self.refs.borrow_mut().entry(id).or_insert(0) += 1;
            Some(id)
        }
        fn add_active_child(&self, child: Id) -> bool {
            assert!(!self.locked.get(), "add under transition_mutex");
            let ok = !self.refuses.contains(&child);
            if ok {
                *self.links.borrow_mut().entry(child).or_insert(0) += 1;
            }
            self.ev(Ev::Add(child, ok));
            ok
        }
        fn remove_active_child(&self, child: Id) {
            assert!(!self.locked.get(), "remove under transition_mutex");
            *self.links.borrow_mut().entry(child).or_insert(0) -= 1;
            self.ev(Ev::Remove(child));
        }
        fn release(&self, child: Option<Id>) {
            assert!(!self.locked.get(), "release under transition_mutex");
            if let Some(c) = child {
                *self.refs.borrow_mut().entry(c).or_insert(0) -= 1;
            }
            self.ev(Ev::Release(child));
        }
        fn warn_recursion(&self, name: Name) {
            self.ev(Ev::WarnRecursion(name));
        }
        fn warn_not_found(&self, name: Name) {
            self.ev(Ev::WarnNotFound(name));
        }
        fn set_source(&self, idx: usize, value: Option<Id>) {
            self.need_lock("set transition_sources");
            assert_eq!(idx, 0);
            self.ev(Ev::SetSource(value));
            self.f.borrow_mut().source = value;
        }
        fn source_active(&self, idx: usize) -> bool {
            self.need_lock("transition_source_active");
            assert_eq!(idx, 0);
            self.ev(Ev::SourceActive);
            self.f.borrow().active
        }
        fn set_source_active(&self, idx: usize, value: bool) {
            self.need_lock("set transition_source_active");
            assert_eq!(idx, 0);
            self.ev(Ev::SetSourceActive(value));
            self.f.borrow_mut().active = value;
        }
        fn set_alignment(&self, value: u32) {
            self.need_lock("set transition_alignment");
            self.ev(Ev::SetAlignment(value));
            self.f.borrow_mut().alignment = value;
        }
        fn set_mode(&self, value: i32) {
            self.need_lock("set transition_mode");
            self.ev(Ev::SetMode(value));
            self.f.borrow_mut().mode = value;
        }
        fn set_scale_type(&self, value: i32) {
            self.need_lock("set transition_scale_type");
            self.ev(Ev::SetScaleType(value));
            self.f.borrow_mut().scale_type = value;
        }
        fn set_cx(&self, value: u32) {
            self.need_lock("set transition_cx");
            self.ev(Ev::SetCx(value));
            self.f.borrow_mut().cx = value;
        }
        fn set_cy(&self, value: u32) {
            self.need_lock("set transition_cy");
            self.ev(Ev::SetCy(value));
            self.f.borrow_mut().cy = value;
        }
    }

    fn data(name: Option<Name>, ints: &[(&'static CStr, i64)]) -> Data {
        Data {
            name,
            ints: ints.iter().copied().collect(),
        }
    }

    const READS: [Ev; 6] = [
        Ev::GetString(KEY_SOURCE_A),
        Ev::GetInt(KEY_ALIGNMENT),
        Ev::GetInt(KEY_MODE),
        Ev::GetInt(KEY_SCALE_TYPE),
        Ev::GetInt(KEY_CX),
        Ev::GetInt(KEY_CY),
    ];

    #[test]
    fn constants() {
        assert_eq!(KEY_SOURCE_A, c"transition_source_a");
        assert_eq!(KEY_ALIGNMENT, c"transition_alignment");
        assert_eq!(KEY_MODE, c"transition_mode");
        assert_eq!(KEY_SCALE_TYPE, c"transition_scale_type");
        assert_eq!(KEY_CX, c"transition_cx");
        assert_eq!(KEY_CY, c"transition_cy");
        assert_eq!(
            core::RECURSION_WARNING,
            c"Cannot set transition '%s' to source '%s' due to infinite recursion"
        );
        assert_eq!(
            core::NOT_FOUND_WARNING,
            c"Failed to find source '%s' for transition '%s'"
        );
    }

    #[test]
    fn c_fields() {
        let tr = Tr::new(&[("child", 2)]);
        let d = data(
            Some("child"),
            &[
                (KEY_ALIGNMENT, 5),
                (KEY_MODE, 1),
                (KEY_SCALE_TYPE, 2),
                (KEY_CX, 1920),
                (KEY_CY, 1080),
            ],
        );
        load(&tr, &d);
        let mut want = READS.to_vec();
        want.extend([
            Ev::ByName("child"),
            Ev::Add(2, true),
            Ev::Lock,
            Ev::SourceActive,
            Ev::SetSource(Some(2)),
            Ev::SetSourceActive(true),
            Ev::SetAlignment(5),
            Ev::SetMode(1),
            Ev::SetScaleType(2),
            Ev::SetCx(1920),
            Ev::SetCy(1080),
            Ev::Unlock,
            Ev::Release(None),
            // the size, then each slot's matrix
            Ev::Lock,
            Ev::Width(2),
            Ev::Height(2),
            Ev::Unlock,
            Ev::SetActual(200, 100),
            Ev::Lock,
            Ev::Width(2),
            Ev::Height(2),
            Ev::Unlock,
            Ev::Matrix(0),
            Ev::Lock,
            Ev::Unlock,
        ]);
        assert_eq!(tr.events(), want);
        assert_eq!(
            tr.fields(),
            Fields {
                source: Some(2),
                active: true,
                alignment: 5,
                mode: 1,
                scale_type: 2,
                cx: 1920,
                cy: 1080,
                actual: (200, 100),
            }
        );
        // the lookup's reference is the slot's, and the link is held
        assert_eq!((tr.refs(2), tr.link(2)), (2, 1));
        assert!(!tr.locked.get());
    }

    #[test]
    fn c_missing_keys() {
        // obs_data gives "" and 0s: "" is looked up and not found, and the
        // alignment becomes 0 (OBS_ALIGN_CENTER)
        let tr = Tr::new(&[]);
        load(&tr, &data(Some(""), &[]));
        let ev = tr.events();
        assert_eq!(ev[..6], READS);
        assert_eq!(ev[6..8], [Ev::ByName(""), Ev::WarnNotFound("")]);
        assert_eq!(
            tr.fields(),
            Fields {
                active: true,
                ..Fields::default()
            }
        );
    }

    #[test]
    fn no_name_no_lookup() {
        let tr = Tr::new(&[("x", 1)]);
        load(&tr, &data(None, &[]));
        let ev = tr.events();
        assert_eq!(ev[6..9], [Ev::Lock, Ev::SourceActive, Ev::SetSource(None)]);
        assert!(
            !ev.iter()
                .any(|e| matches!(e, Ev::ByName(_) | Ev::WarnNotFound(_)))
        );
        // the slot is active with nothing in it
        assert_eq!((tr.fields().source, tr.fields().active), (None, true));
    }

    #[test]
    fn c_source_outcomes() {
        // found, refused: warned, released, none stored
        let mut tr = Tr::new(&[("loop", 3)]);
        tr.refuses.push(3);
        load(&tr, &data(Some("loop"), &[]));
        let ev = tr.events();
        assert_eq!(
            ev[6..11],
            [
                Ev::ByName("loop"),
                Ev::Add(3, false),
                Ev::WarnRecursion("loop"),
                Ev::Release(Some(3)),
                Ev::Lock
            ]
        );
        assert!(ev.contains(&Ev::SetSource(None)));
        assert_eq!((tr.refs(3), tr.link(3)), (1, 0));
        assert_eq!((tr.fields().source, tr.fields().active), (None, true));

        // not found: warned, no add
        let tr = Tr::new(&[("other", 4)]);
        load(&tr, &data(Some("gone"), &[]));
        let ev = tr.events();
        assert_eq!(
            ev[6..9],
            [Ev::ByName("gone"), Ev::WarnNotFound("gone"), Ev::Lock]
        );
        assert_eq!(tr.refs(4), 1);
    }

    #[test]
    fn c_old_slot_a() {
        // an active old child: its link and reference handed back after
        // the unlock
        let tr = Tr::new(&[("new", 2)]);
        tr.hold(1, true);
        load(&tr, &data(Some("new"), &[]));
        let ev = tr.events();
        let at = ev.iter().position(|e| *e == Ev::Unlock).unwrap();
        assert_eq!(ev[at + 1..at + 3], [Ev::Remove(1), Ev::Release(Some(1))]);
        assert_eq!((tr.refs(1), tr.link(1)), (0, 0));

        // an inactive one: only its reference
        let tr = Tr::new(&[]);
        tr.hold(1, false);
        load(&tr, &data(None, &[]));
        let ev = tr.events();
        let at = ev.iter().position(|e| *e == Ev::Unlock).unwrap();
        assert_eq!(ev[at + 1], Ev::Release(Some(1)));
        assert_eq!(tr.refs(1), 0);

        // the same source loaded again: added a second time, then the old
        // link and reference go
        let tr = Tr::new(&[("same", 5)]);
        tr.hold(5, true);
        load(&tr, &data(Some("same"), &[]));
        assert_eq!((tr.refs(5), tr.link(5)), (2, 1));
        assert_eq!(tr.fields().source, Some(5));
    }

    #[test]
    fn c_casts() {
        let cases: [(i64, u32); 6] = [
            (0, 0),
            (7, 7),
            (-1, u32::MAX),
            ((1 << 32) | 7, 7),
            (i64::MIN, 0),
            (i64::MAX, u32::MAX),
        ];
        for (v, low) in cases {
            let tr = Tr::new(&[]);
            load(
                &tr,
                &data(
                    None,
                    &[(KEY_ALIGNMENT, v), (KEY_MODE, v), (KEY_CX, v), (KEY_CY, v)],
                ),
            );
            let f = tr.fields();
            assert_eq!((f.alignment, f.cx, f.cy), (low, low, low), "{v}");
            assert_eq!(f.mode, low as i32, "{v}");
        }
        // the scale type by its rule, over the whole i64
        for (v, want) in [(0, 0), (1, 1), (2, 2), (3, 0), (-1, 0), ((1 << 32) | 1, 0)] {
            let tr = Tr::new(&[]);
            load(&tr, &data(None, &[(KEY_SCALE_TYPE, v)]));
            assert_eq!(tr.fields().scale_type, want, "{v}");
        }
    }

    /// A copy of the C: what `obs_transition_load()` calls, up to the
    /// release of the old slot A.
    fn model(tr: &Tr, d: &Data, old: Option<Id>, old_active: bool) -> Vec<Ev> {
        let mut ev = READS.to_vec();
        let i = |k: &'static CStr| d.ints.get(k).copied().unwrap_or(0);
        let mut source = None;
        if let Some(name) = d.name {
            ev.push(Ev::ByName(name));
            source = tr.names.get(name).copied();
            if let Some(s) = source {
                let ok = !tr.refuses.contains(&s);
                ev.push(Ev::Add(s, ok));
                if !ok {
                    ev.push(Ev::WarnRecursion(name));
                    ev.push(Ev::Release(Some(s)));
                    source = None;
                }
            } else {
                ev.push(Ev::WarnNotFound(name));
            }
        }
        ev.extend([
            Ev::Lock,
            Ev::SourceActive,
            Ev::SetSource(source),
            Ev::SetSourceActive(true),
            Ev::SetAlignment(i(KEY_ALIGNMENT) as u32),
            Ev::SetMode(i(KEY_MODE) as i32),
            Ev::SetScaleType(match i(KEY_SCALE_TYPE) {
                1 => 1,
                2 => 2,
                _ => 0,
            }),
            Ev::SetCx(i(KEY_CX) as u32),
            Ev::SetCy(i(KEY_CY) as u32),
            Ev::Unlock,
        ]);
        if let (Some(o), true) = (old, old_active) {
            ev.push(Ev::Remove(o));
        }
        ev.push(Ev::Release(old));
        ev
    }

    fn any_int() -> impl Strategy<Value = Option<i64>> {
        prop_oneof![
            Just(None),
            (-3i64..=3).prop_map(Some),
            any::<i64>().prop_map(Some),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(cases(512)))]

        #[test]
        fn load_matches_the_c(
            name in prop_oneof![Just(None), Just(Some("a")), Just(Some("b")), Just(Some("")), Just(Some("gone"))],
            ints in prop::array::uniform5(any_int()),
            refuse_a in any::<bool>(),
            old in prop_oneof![Just(None), Just(Some(1u32)), Just(Some(2u32)), Just(Some(9u32))],
            old_active in any::<bool>(),
        ) {
            let mut tr = Tr::new(&[("a", 1), ("b", 2)]);
            if refuse_a {
                tr.refuses.push(1);
            }
            if let Some(o) = old {
                tr.hold(o, old_active);
            }
            let keys = [KEY_ALIGNMENT, KEY_MODE, KEY_SCALE_TYPE, KEY_CX, KEY_CY];
            let d = Data {
                name,
                ints: keys
                    .into_iter()
                    .zip(ints)
                    .filter_map(|(k, v)| v.map(|v| (k, v)))
                    .collect(),
            };
            let want = model(&tr, &d, old, old_active && old.is_some());
            let refs_before: Vec<i64> = [1, 2, 9].iter().map(|&id| tr.refs(id)).collect();
            load(&tr, &d);
            let ev = tr.events();
            prop_assert_eq!(&ev[..want.len()], &want[..]);
            // then the size and matrix recalculation, and nothing else
            prop_assert!(ev[want.len()..].iter().all(|e| matches!(
                e,
                Ev::Lock | Ev::Unlock | Ev::Width(_) | Ev::Height(_) | Ev::SetActual(..) | Ev::Matrix(_)
            )));
            prop_assert!(!tr.locked.get());
            // the slot's reference is the lookup's, the old one handed back
            let stored = tr.fields().source;
            for (k, &id) in [1u32, 2, 9].iter().enumerate() {
                let mut want_refs = refs_before[k];
                if stored == Some(id) {
                    want_refs += 1;
                }
                if old == Some(id) {
                    want_refs -= 1;
                }
                prop_assert_eq!(tr.refs(id), want_refs, "source {}", id);
            }
        }
    }
}
