//! Tier 1 tests for `obs_source::transition_size`, the safe core of
//! `libobs/obs-source-transition-size.c.h`.
//!
//! The functions run on a fake transition: its slots, the set and actual
//! sizes, the scale type, the alignment, the mode and manual fields, both
//! render targets and both locks, with children that have a width and a
//! height. The fake records each call into the rest of libobs (the locks,
//! `obs_source_get_width()` and `_height()`, `add_alignment()`, the matrix,
//! `calc_torquef()`, `gs_texrender_reset()`) and each write of a field the
//! tick writes, in order, and panics when a slot, a child's size, the mode or
//! a manual field is read without `transition_mutex` (the C holds it there)
//! or a lock is taken twice. `add_alignment()` is the C's, and `calc_torquef()` the
//! `obs-graphics` port, so the positions and the manual values are the ones
//! libobs computes.
//!
//! The `c_*` cases follow `test_source_transition_size.c`: the actual size
//! over zero, one and two children; the size a transition is drawn at; each
//! slot's matrix for its scale type and alignment; the manual value with a
//! torque of 0, a torque, and a clamp. The other cases pin the plan: an
//! empty slot and a child with no size, a scale type outside the enum, the
//! unsized transition, the hold outside manual mode, the texture trylock,
//! and the order of every lock and call, with a proptest against a copy of
//! the C.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use obs_c_oracle::parity::cases;
use obs_graphics::math_extra::calc_torquef;
use obs_source::transition_clock::{MODE_MANUAL, SCALE_ASPECT, SCALE_MAX_ONLY, SCALE_STRETCH};
use obs_source::transition_size::{
    SLOTS, Transition, get_cx, get_cy, recalculate_matrices, recalculate_matrix, recalculate_size,
    tick,
};
use proptest::prelude::*;

const MODE_AUTO: i32 = 0;
const ALIGN_CENTER: u32 = 0;
const ALIGN_LEFT: u32 = 1;
const ALIGN_RIGHT: u32 = 2;
const ALIGN_TOP: u32 = 4;
const ALIGN_BOTTOM: u32 = 8;
/// `EBUSY` on Linux, what a held `transition_tex_mutex` returns.
const EBUSY: i32 = 16;

/// A child of the fake: an id.
type Id = u32;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Ev {
    Lock,
    Unlock,
    TryLockTex(i32),
    UnlockTex,
    Width(Id),
    Height(Id),
    Align(u32, i32, i32),
    Matrix(usize, (f32, f32), (f32, f32)),
    Torque(f32, f32, f32, f32, f32),
    Reset(u32),
    SetActualCx(u32),
    SetActualCy(u32),
    SetManualVal(f32),
}

/// A stored matrix: the scale and the position it was built from.
type Matrix = ((f32, f32), (f32, f32));

#[derive(Debug, Clone, Copy, PartialEq)]
struct Fields {
    sources: [Option<Id>; 2],
    cx: u32,
    cy: u32,
    actual_cx: u32,
    actual_cy: u32,
    scale_type: i32,
    alignment: u32,
    mode: i32,
    manual_val: f32,
    manual_target: f32,
    manual_torque: f32,
    manual_clamp: f32,
    matrices: [Option<Matrix>; 2],
}

impl Default for Fields {
    fn default() -> Self {
        Self {
            sources: [None; 2],
            cx: 0,
            cy: 0,
            actual_cx: 0,
            actual_cy: 0,
            scale_type: SCALE_MAX_ONLY,
            alignment: ALIGN_CENTER,
            mode: MODE_AUTO,
            manual_val: 0.0,
            manual_target: 0.0,
            manual_torque: 0.0,
            manual_clamp: 0.0,
            matrices: [None; 2],
        }
    }
}

#[derive(Default)]
struct Tr {
    f: RefCell<Fields>,
    sizes: RefCell<BTreeMap<Id, (u32, u32)>>,
    locked: Cell<bool>,
    tex_locked: Cell<bool>,
    /// What the texture trylock returns: 0 takes the lock.
    tex_busy: Cell<i32>,
    events: RefCell<Vec<Ev>>,
}

impl Tr {
    fn new() -> Self {
        Self::default()
    }
    fn with(f: Fields) -> Self {
        let tr = Self::new();
        *tr.f.borrow_mut() = f;
        tr
    }
    fn child(&self, idx: usize, id: Id, cx: u32, cy: u32) {
        self.sizes.borrow_mut().insert(id, (cx, cy));
        self.f.borrow_mut().sources[idx] = Some(id);
    }
    fn ev(&self, ev: Ev) {
        self.events.borrow_mut().push(ev);
    }
    fn need_lock(&self, what: &str) {
        assert!(self.locked.get(), "{what} without transition_mutex");
    }
    fn fields(&self) -> Fields {
        *self.f.borrow()
    }
    fn events(&self) -> Vec<Ev> {
        std::mem::take(&mut self.events.borrow_mut())
    }
    fn assert_unlocked(&self) {
        assert!(!self.locked.get(), "transition_mutex left held");
        assert!(!self.tex_locked.get(), "transition_tex_mutex left held");
    }
}

/// libobs's `add_alignment()` on a zeroed vec2.
fn add_alignment(align: u32, cx: i32, cy: i32) -> (f32, f32) {
    let mut x = 0.0f32;
    let mut y = 0.0f32;
    if align & ALIGN_RIGHT != 0 {
        x += cx as f32;
    } else if align & ALIGN_LEFT == 0 {
        x += (cx / 2) as f32;
    }
    if align & ALIGN_BOTTOM != 0 {
        y += cy as f32;
    } else if align & ALIGN_TOP == 0 {
        y += (cy / 2) as f32;
    }
    (x, y)
}

impl Transition for Tr {
    type Child = Id;
    type Texrender = u32;

    fn source(&self, idx: usize) -> Option<Id> {
        self.need_lock("transition_sources");
        self.f.borrow().sources[idx]
    }
    fn cx(&self) -> u32 {
        self.f.borrow().cx
    }
    fn cy(&self) -> u32 {
        self.f.borrow().cy
    }
    fn actual_cx(&self) -> u32 {
        self.f.borrow().actual_cx
    }
    fn set_actual_cx(&self, value: u32) {
        self.ev(Ev::SetActualCx(value));
        self.f.borrow_mut().actual_cx = value;
    }
    fn actual_cy(&self) -> u32 {
        self.f.borrow().actual_cy
    }
    fn set_actual_cy(&self, value: u32) {
        self.ev(Ev::SetActualCy(value));
        self.f.borrow_mut().actual_cy = value;
    }
    fn scale_type(&self) -> i32 {
        self.f.borrow().scale_type
    }
    fn alignment(&self) -> u32 {
        self.f.borrow().alignment
    }
    fn mode(&self) -> i32 {
        self.need_lock("transition_mode");
        self.f.borrow().mode
    }
    fn manual_val(&self) -> f32 {
        self.need_lock("transition_manual_val");
        self.f.borrow().manual_val
    }
    fn set_manual_val(&self, value: f32) {
        self.need_lock("transition_manual_val");
        self.ev(Ev::SetManualVal(value));
        self.f.borrow_mut().manual_val = value;
    }
    fn manual_target(&self) -> f32 {
        self.need_lock("transition_manual_target");
        self.f.borrow().manual_target
    }
    fn manual_torque(&self) -> f32 {
        self.need_lock("transition_manual_torque");
        self.f.borrow().manual_torque
    }
    fn manual_clamp(&self) -> f32 {
        self.need_lock("transition_manual_clamp");
        self.f.borrow().manual_clamp
    }
    fn texrender(&self, idx: usize) -> u32 {
        assert!(
            self.tex_locked.get(),
            "transition_texrender without transition_tex_mutex"
        );
        [11, 12][idx]
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
        assert!(!self.tex_locked.get(), "transition_tex_mutex taken twice");
        let r = self.tex_busy.get();
        if r == 0 {
            self.tex_locked.set(true);
        }
        self.ev(Ev::TryLockTex(r));
        r
    }
    fn unlock_textures(&self) {
        assert!(
            self.tex_locked.get(),
            "transition_tex_mutex unlocked unheld"
        );
        self.tex_locked.set(false);
        self.ev(Ev::UnlockTex);
    }
    fn child_width(&self, child: Id) -> u32 {
        self.need_lock("obs_source_get_width");
        self.ev(Ev::Width(child));
        self.sizes.borrow()[&child].0
    }
    fn child_height(&self, child: Id) -> u32 {
        self.need_lock("obs_source_get_height");
        self.ev(Ev::Height(child));
        self.sizes.borrow()[&child].1
    }
    fn alignment_position(&self, align: u32, cx: i32, cy: i32) -> (f32, f32) {
        assert!(!self.locked.get(), "add_alignment under transition_mutex");
        self.ev(Ev::Align(align, cx, cy));
        add_alignment(align, cx, cy)
    }
    fn set_matrix(&self, idx: usize, scale: (f32, f32), pos: (f32, f32)) {
        assert!(self.locked.get(), "matrix without transition_mutex");
        self.ev(Ev::Matrix(idx, scale, pos));
        self.f.borrow_mut().matrices[idx] = Some((scale, pos));
    }
    fn calc_torquef(&self, val1: f32, val2: f32, torque: f32, min_adjust: f32, t: f32) -> f32 {
        self.ev(Ev::Torque(val1, val2, torque, min_adjust, t));
        calc_torquef(val1, val2, torque, min_adjust, t)
    }
    fn texrender_reset(&self, texrender: u32) {
        assert!(
            self.tex_locked.get(),
            "gs_texrender_reset without transition_tex_mutex"
        );
        self.ev(Ev::Reset(texrender));
    }
}

fn sized(cx: u32, cy: u32) -> Fields {
    Fields {
        cx,
        cy,
        ..Fields::default()
    }
}

#[test]
fn constants() {
    assert_eq!(SLOTS, [0, 1]);
}

#[test]
fn c_actual_size() {
    // none: 0x0
    let tr = Tr::new();
    recalculate_size(&tr);
    assert_eq!((tr.fields().actual_cx, tr.fields().actual_cy), (0, 0));
    assert_eq!(
        tr.events(),
        [Ev::Lock, Ev::Unlock, Ev::SetActualCx(0), Ev::SetActualCy(0)]
    );

    // one child: its size, in either slot
    for idx in SLOTS {
        let tr = Tr::new();
        tr.child(idx, 7, 320, 240);
        recalculate_size(&tr);
        assert_eq!((tr.fields().actual_cx, tr.fields().actual_cy), (320, 240));
        assert_eq!(
            tr.events(),
            [
                Ev::Lock,
                Ev::Width(7),
                Ev::Height(7),
                Ev::Unlock,
                Ev::SetActualCx(320),
                Ev::SetActualCy(240)
            ]
        );
    }

    // two: the larger width and the larger height, apart
    let tr = Tr::new();
    tr.child(0, 1, 100, 50);
    tr.child(1, 2, 80, 90);
    recalculate_size(&tr);
    assert_eq!((tr.fields().actual_cx, tr.fields().actual_cy), (100, 90));
    assert_eq!(
        tr.events(),
        [
            Ev::Lock,
            Ev::Width(1),
            Ev::Height(1),
            Ev::Width(2),
            Ev::Height(2),
            Ev::Unlock,
            Ev::SetActualCx(100),
            Ev::SetActualCy(90)
        ]
    );
    tr.assert_unlocked();

    // the fold starts from 0, not from the last actual size
    let tr = Tr::with(Fields {
        actual_cx: 999,
        actual_cy: 999,
        ..Fields::default()
    });
    tr.child(1, 3, 10, 20);
    recalculate_size(&tr);
    assert_eq!((tr.fields().actual_cx, tr.fields().actual_cy), (10, 20));
}

#[test]
fn c_draw_size() {
    // a set size wins, one dimension at a time
    let tr = Tr::with(Fields {
        cx: 640,
        cy: 0,
        actual_cx: 100,
        actual_cy: 200,
        ..Fields::default()
    });
    assert_eq!(get_cx(&tr), 640);
    assert_eq!(get_cy(&tr), 200);

    let tr = Tr::with(Fields {
        cx: 0,
        cy: 480,
        actual_cx: 100,
        actual_cy: 200,
        ..Fields::default()
    });
    assert_eq!(get_cx(&tr), 100);
    assert_eq!(get_cy(&tr), 480);

    let tr = Tr::new();
    assert_eq!((get_cx(&tr), get_cy(&tr)), (0, 0));
    // no lock, no call
    assert_eq!(tr.events(), []);
}

/// The matrix of slot 0 for a child of `cx` by `cy` in a 200x100
/// transition with `scale_type` and `align`.
fn matrix_of(scale_type: i32, align: u32, cx: u32, cy: u32) -> (Vec<Ev>, Fields) {
    let tr = Tr::with(Fields {
        scale_type,
        alignment: align,
        ..sized(200, 100)
    });
    tr.child(0, 5, cx, cy);
    recalculate_matrix(&tr, 0);
    tr.assert_unlocked();
    (tr.events(), tr.fields())
}

#[test]
fn c_matrix_scale_types() {
    // max only, a child that fits: its own size, centered
    let (ev, f) = matrix_of(SCALE_MAX_ONLY, ALIGN_CENTER, 100, 100);
    assert_eq!(
        ev,
        [
            Ev::Lock,
            Ev::Width(5),
            Ev::Height(5),
            Ev::Unlock,
            Ev::Align(ALIGN_CENTER, 100, 0),
            Ev::Lock,
            Ev::Matrix(0, (1.0, 1.0), (50.0, 0.0)),
            Ev::Unlock,
        ]
    );
    assert_eq!(f.matrices, [Some(((1.0, 1.0), (50.0, 0.0))), None]);

    // max only, a larger child: fitted by its aspect
    let (ev, _) = matrix_of(SCALE_MAX_ONLY, ALIGN_CENTER, 400, 100);
    assert_eq!(
        ev[4..],
        [
            Ev::Align(ALIGN_CENTER, 0, 50),
            Ev::Lock,
            Ev::Matrix(0, (0.5, 0.5), (0.0, 25.0)),
            Ev::Unlock,
        ]
    );

    // aspect: the smaller ratio on both sides
    let (ev, _) = matrix_of(SCALE_ASPECT, ALIGN_CENTER, 100, 100);
    assert_eq!(
        ev[4..],
        [
            Ev::Align(ALIGN_CENTER, 100, 0),
            Ev::Lock,
            Ev::Matrix(0, (1.0, 1.0), (50.0, 0.0)),
            Ev::Unlock,
        ]
    );
    let (ev, _) = matrix_of(SCALE_ASPECT, ALIGN_CENTER, 50, 50);
    assert_eq!(
        ev[4..],
        [
            Ev::Align(ALIGN_CENTER, 100, 0),
            Ev::Lock,
            Ev::Matrix(0, (2.0, 2.0), (50.0, 0.0)),
            Ev::Unlock,
        ]
    );
    let (ev, _) = matrix_of(SCALE_ASPECT, ALIGN_CENTER, 400, 100);
    assert_eq!(
        ev[4..],
        [
            Ev::Align(ALIGN_CENTER, 0, 50),
            Ev::Lock,
            Ev::Matrix(0, (0.5, 0.5), (0.0, 25.0)),
            Ev::Unlock,
        ]
    );

    // stretch: each side's ratio
    let (ev, _) = matrix_of(SCALE_STRETCH, ALIGN_CENTER, 100, 50);
    assert_eq!(
        ev[4..],
        [
            Ev::Align(ALIGN_CENTER, 0, 0),
            Ev::Lock,
            Ev::Matrix(0, (2.0, 2.0), (0.0, 0.0)),
            Ev::Unlock,
        ]
    );
    let (ev, _) = matrix_of(SCALE_STRETCH, ALIGN_CENTER, 400, 25);
    assert_eq!(
        ev[4..],
        [
            Ev::Align(ALIGN_CENTER, 0, 0),
            Ev::Lock,
            Ev::Matrix(0, (0.5, 4.0), (0.0, 0.0)),
            Ev::Unlock,
        ]
    );
}

#[test]
fn c_matrix_alignments() {
    // a 100x50 child in 200x100 leaves 100 by 50
    let cases = [
        (ALIGN_CENTER, (50.0, 25.0)),
        (ALIGN_LEFT | ALIGN_TOP, (0.0, 0.0)),
        (ALIGN_RIGHT | ALIGN_BOTTOM, (100.0, 50.0)),
        (ALIGN_LEFT, (0.0, 25.0)),
        (ALIGN_RIGHT, (100.0, 25.0)),
        (ALIGN_TOP, (50.0, 0.0)),
        (ALIGN_BOTTOM, (50.0, 50.0)),
        // right wins over left, bottom over top
        (
            ALIGN_LEFT | ALIGN_RIGHT | ALIGN_TOP | ALIGN_BOTTOM,
            (100.0, 50.0),
        ),
    ];
    for (align, pos) in cases {
        let (ev, f) = matrix_of(SCALE_MAX_ONLY, align, 100, 50);
        assert_eq!(ev[4], Ev::Align(align, 100, 50), "align {align}");
        assert_eq!(f.matrices[0], Some(((1.0, 1.0), pos)), "align {align}");
    }

    // an odd room is halved as an int
    let (_, f) = matrix_of(SCALE_MAX_ONLY, ALIGN_CENTER, 99, 49);
    assert_eq!(f.matrices[0], Some(((1.0, 1.0), (50.0, 25.0))));

    // a larger child, fitted by its width: the f32 room the scale leaves,
    // each cast to int (a sliver of -0.00002 is 0)
    let (ev, _) = matrix_of(SCALE_MAX_ONLY, ALIGN_RIGHT | ALIGN_BOTTOM, 300, 100);
    let s = 200.0f32 / 300.0;
    let room = (200.0f32 - 300.0 * s, 100.0f32 - 100.0 * s);
    assert_eq!((room.0 as i32, room.1 as i32), (0, 33));
    assert_eq!(
        ev[4..],
        [
            Ev::Align(ALIGN_RIGHT | ALIGN_BOTTOM, 0, 33),
            Ev::Lock,
            Ev::Matrix(0, (s, s), (0.0, 33.0)),
            Ev::Unlock,
        ]
    );

    // fitted by its height: the room is 100 wide, halved
    let (ev, _) = matrix_of(SCALE_ASPECT, ALIGN_CENTER, 300, 300);
    let s = 100.0f32 / 300.0;
    let room = (200.0f32 - 300.0 * s, 100.0f32 - 300.0 * s);
    let room = (room.0 as i32, room.1 as i32);
    assert_eq!(
        ev[4..],
        [
            Ev::Align(ALIGN_CENTER, room.0, room.1),
            Ev::Lock,
            Ev::Matrix(0, (s, s), ((room.0 / 2) as f32, (room.1 / 2) as f32)),
            Ev::Unlock,
        ]
    );
}

#[test]
fn matrix_each_slot() {
    let tr = Tr::with(sized(200, 100));
    tr.child(0, 1, 100, 100);
    tr.child(1, 2, 200, 50);
    recalculate_matrices(&tr);
    assert_eq!(
        tr.events(),
        [
            Ev::Lock,
            Ev::Width(1),
            Ev::Height(1),
            Ev::Unlock,
            Ev::Align(ALIGN_CENTER, 100, 0),
            Ev::Lock,
            Ev::Matrix(0, (1.0, 1.0), (50.0, 0.0)),
            Ev::Unlock,
            Ev::Lock,
            Ev::Width(2),
            Ev::Height(2),
            Ev::Unlock,
            Ev::Align(ALIGN_CENTER, 0, 50),
            Ev::Lock,
            Ev::Matrix(1, (1.0, 1.0), (0.0, 25.0)),
            Ev::Unlock,
        ]
    );
    tr.assert_unlocked();
}

#[test]
fn matrix_no_matrix() {
    // an empty slot: the lock, nothing else, and the old matrix kept
    let tr = Tr::with(Fields {
        matrices: [Some(((3.0, 3.0), (1.0, 1.0))), None],
        ..sized(200, 100)
    });
    recalculate_matrix(&tr, 0);
    assert_eq!(tr.events(), [Ev::Lock, Ev::Unlock]);
    assert_eq!(tr.fields().matrices[0], Some(((3.0, 3.0), (1.0, 1.0))));

    // a child with no width or no height: its size read, no matrix
    for (cx, cy) in [(0, 10), (10, 0), (0, 0)] {
        let tr = Tr::with(sized(200, 100));
        tr.child(1, 9, cx, cy);
        recalculate_matrix(&tr, 1);
        assert_eq!(
            tr.events(),
            [Ev::Lock, Ev::Width(9), Ev::Height(9), Ev::Unlock],
            "{cx}x{cy}"
        );
        assert_eq!(tr.fields().matrices, [None, None]);
    }
}

#[test]
fn matrix_scale_type_outside_enum() {
    // the scale is left at 1 by 1, and the matrix built
    for scale_type in [3, -1, 100, i32::MIN, i32::MAX] {
        let (ev, f) = matrix_of(scale_type, ALIGN_CENTER, 400, 50);
        assert_eq!(
            ev[4..],
            [
                Ev::Align(ALIGN_CENTER, -200, 50),
                Ev::Lock,
                Ev::Matrix(0, (1.0, 1.0), (-100.0, 25.0)),
                Ev::Unlock,
            ],
            "scale type {scale_type}"
        );
        assert_eq!(f.matrices[0], Some(((1.0, 1.0), (-100.0, 25.0))));
    }
}

#[test]
fn matrix_unsized_transition() {
    // no set size and no actual size: the child is scaled to 0 (its aspect
    // against NaN), with no room
    let tr = Tr::new();
    tr.child(0, 4, 100, 50);
    recalculate_matrix(&tr, 0);
    assert_eq!(
        tr.events()[4..],
        [
            Ev::Align(ALIGN_CENTER, 0, 0),
            Ev::Lock,
            Ev::Matrix(0, (0.0, 0.0), (0.0, 0.0)),
            Ev::Unlock,
        ]
    );

    // the actual size stands in for an unset one
    let tr = Tr::with(Fields {
        actual_cx: 200,
        actual_cy: 100,
        ..Fields::default()
    });
    tr.child(0, 4, 100, 50);
    recalculate_matrix(&tr, 0);
    assert_eq!(tr.fields().matrices[0], Some(((1.0, 1.0), (50.0, 25.0))));
}

#[test]
fn c_manual_value() {
    // a torque of 0: snapped to the target
    let tr = Tr::with(Fields {
        mode: MODE_MANUAL,
        manual_val: 0.25,
        manual_target: 0.75,
        ..Fields::default()
    });
    tick(&tr, 0.016);
    assert_eq!(tr.fields().manual_val, 0.75);
    assert!(tr.events().contains(&Ev::SetManualVal(0.75)));

    // -0 is 0
    let tr = Tr::with(Fields {
        mode: MODE_MANUAL,
        manual_val: 0.25,
        manual_target: 0.5,
        manual_torque: -0.0,
        ..Fields::default()
    });
    tick(&tr, 0.016);
    assert_eq!(tr.fields().manual_val, 0.5);

    // a torque: one calc_torquef() step
    let tr = Tr::with(Fields {
        mode: MODE_MANUAL,
        manual_val: 0.0,
        manual_target: 1.0,
        manual_torque: 2.0,
        manual_clamp: 0.01,
        ..Fields::default()
    });
    tick(&tr, 0.1);
    let step = calc_torquef(0.0, 1.0, 2.0, 0.01, 0.1);
    assert_eq!(tr.fields().manual_val, step);
    let ev = tr.events();
    let at = ev
        .iter()
        .position(|e| *e == Ev::Torque(0.0, 1.0, 2.0, 0.01, 0.1))
        .unwrap();
    assert_eq!(ev[at + 1], Ev::SetManualVal(step));

    // at the target in one tick when the clamp is larger than the step
    let tr = Tr::with(Fields {
        mode: MODE_MANUAL,
        manual_val: 0.0,
        manual_target: 0.1,
        manual_torque: 1.0,
        manual_clamp: 5.0,
        ..Fields::default()
    });
    tick(&tr, 1.0);
    assert_eq!(tr.fields().manual_val, 0.1);

    // NaN is not 0: a step
    let tr = Tr::with(Fields {
        mode: MODE_MANUAL,
        manual_torque: f32::NAN,
        manual_target: 1.0,
        ..Fields::default()
    });
    tick(&tr, 1.0);
    assert!(tr.events().iter().any(|e| matches!(e, Ev::Torque(..))));
}

#[test]
fn manual_hold_outside_manual_mode() {
    for mode in [MODE_AUTO, 2, -1, i32::MAX] {
        for torque in [0.0, 1.0] {
            let tr = Tr::with(Fields {
                mode,
                manual_val: 0.25,
                manual_target: 0.75,
                manual_torque: torque,
                ..Fields::default()
            });
            tick(&tr, 0.5);
            assert_eq!(tr.fields().manual_val, 0.25, "mode {mode}");
            assert!(
                !tr.events()
                    .iter()
                    .any(|e| matches!(e, Ev::SetManualVal(_) | Ev::Torque(..))),
                "mode {mode}"
            );
        }
    }
}

#[test]
fn tick_order() {
    let tr = Tr::with(Fields {
        mode: MODE_MANUAL,
        manual_val: 0.5,
        manual_target: 1.0,
        ..sized(200, 100)
    });
    tr.child(0, 1, 100, 100);
    tr.child(1, 2, 400, 50);
    tick(&tr, 0.25);
    assert_eq!(
        tr.events(),
        [
            // the size
            Ev::Lock,
            Ev::Width(1),
            Ev::Height(1),
            Ev::Width(2),
            Ev::Height(2),
            Ev::Unlock,
            Ev::SetActualCx(400),
            Ev::SetActualCy(100),
            // slot A's matrix
            Ev::Lock,
            Ev::Width(1),
            Ev::Height(1),
            Ev::Unlock,
            Ev::Align(ALIGN_CENTER, 100, 0),
            Ev::Lock,
            Ev::Matrix(0, (1.0, 1.0), (50.0, 0.0)),
            Ev::Unlock,
            // slot B's
            Ev::Lock,
            Ev::Width(2),
            Ev::Height(2),
            Ev::Unlock,
            Ev::Align(ALIGN_CENTER, 0, 75),
            Ev::Lock,
            Ev::Matrix(1, (0.5, 0.5), (0.0, 37.0)),
            Ev::Unlock,
            // the snap, under the lock (#3570)
            Ev::Lock,
            Ev::SetManualVal(1.0),
            Ev::Unlock,
            // the render targets
            Ev::TryLockTex(0),
            Ev::Reset(11),
            Ev::Reset(12),
            Ev::UnlockTex,
        ]
    );
    tr.assert_unlocked();
}

#[test]
fn tick_texture_lock_busy() {
    for busy in [EBUSY, 1, -1] {
        let tr = Tr::new();
        tr.tex_busy.set(busy);
        tick(&tr, 0.0);
        let ev = tr.events();
        assert_eq!(ev.last(), Some(&Ev::TryLockTex(busy)), "busy {busy}");
        assert!(!ev.iter().any(|e| matches!(e, Ev::Reset(_) | Ev::UnlockTex)));
        tr.assert_unlocked();
    }
}

/// A copy of the C: what `obs_transition_tick()` calls on `f` with children
/// of `sizes`, in order.
fn model_tick(f: &Fields, sizes: &BTreeMap<Id, (u32, u32)>, t: f32, busy: i32) -> Vec<Ev> {
    let mut ev = Vec::new();
    let mut cx = 0u32;
    let mut cy = 0u32;
    ev.push(Ev::Lock);
    for i in SLOTS {
        if let Some(c) = f.sources[i] {
            ev.push(Ev::Width(c));
            ev.push(Ev::Height(c));
            cx = cx.max(sizes[&c].0);
            cy = cy.max(sizes[&c].1);
        }
    }
    ev.push(Ev::Unlock);
    ev.push(Ev::SetActualCx(cx));
    ev.push(Ev::SetActualCy(cy));

    let tr_cx = (if f.cx != 0 { f.cx } else { cx }) as f32;
    let tr_cy = (if f.cy != 0 { f.cy } else { cy }) as f32;
    for i in SLOTS {
        ev.push(Ev::Lock);
        let Some(c) = f.sources[i] else {
            ev.push(Ev::Unlock);
            continue;
        };
        ev.push(Ev::Width(c));
        ev.push(Ev::Height(c));
        ev.push(Ev::Unlock);
        let source_cx = sizes[&c].0 as f32;
        let source_cy = sizes[&c].1 as f32;
        // origin/master's fit, inline
        let tr_aspect = tr_cx / tr_cy;
        if source_cx == 0.0 || source_cy == 0.0 {
            continue;
        }
        let source_aspect = source_cx / source_cy;
        let mut scale_type = f.scale_type;
        let mut scale = (1.0f32, 1.0f32);
        if scale_type == SCALE_MAX_ONLY && (source_cx > tr_cx || source_cy > tr_cy) {
            scale_type = SCALE_ASPECT;
        }
        if scale_type == SCALE_ASPECT {
            let s = if tr_aspect < source_aspect {
                tr_cx / source_cx
            } else {
                tr_cy / source_cy
            };
            scale = (s, s);
        } else if scale_type == SCALE_STRETCH {
            scale = (tr_cx / source_cx, tr_cy / source_cy);
        }
        let ox = tr_cx - source_cx * scale.0;
        let oy = tr_cy - source_cy * scale.1;
        ev.push(Ev::Align(f.alignment, ox as i32, oy as i32));
        ev.push(Ev::Lock);
        ev.push(Ev::Matrix(
            i,
            scale,
            add_alignment(f.alignment, ox as i32, oy as i32),
        ));
        ev.push(Ev::Unlock);
    }

    ev.push(Ev::Lock);
    if f.mode == MODE_MANUAL {
        if f.manual_torque == 0.0 {
            ev.push(Ev::SetManualVal(f.manual_target));
        } else {
            ev.push(Ev::Torque(
                f.manual_val,
                f.manual_target,
                f.manual_torque,
                f.manual_clamp,
                t,
            ));
            ev.push(Ev::SetManualVal(calc_torquef(
                f.manual_val,
                f.manual_target,
                f.manual_torque,
                f.manual_clamp,
                t,
            )));
        }
    }
    ev.push(Ev::Unlock);

    ev.push(Ev::TryLockTex(busy));
    if busy == 0 {
        ev.push(Ev::Reset(11));
        ev.push(Ev::Reset(12));
        ev.push(Ev::UnlockTex);
    }
    ev
}

fn slot() -> impl Strategy<Value = Option<(u32, u32)>> {
    prop_oneof![
        1 => Just(None),
        1 => (0u32..=2, 0u32..=2).prop_map(Some),
        4 => (0u32..=8192, 0u32..=8192).prop_map(Some),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(512)))]

    #[test]
    fn tick_matches_the_c(
        a in slot(),
        b in slot(),
        cx in prop_oneof![Just(0u32), 1u32..=8192],
        cy in prop_oneof![Just(0u32), 1u32..=8192],
        scale_type in prop_oneof![Just(SCALE_MAX_ONLY), Just(SCALE_ASPECT), Just(SCALE_STRETCH), -2i32..=5],
        alignment in 0u32..16,
        mode in prop_oneof![Just(MODE_AUTO), Just(MODE_MANUAL), -1i32..=3],
        val in -2.0f32..2.0,
        target in -2.0f32..2.0,
        torque in prop_oneof![Just(0.0f32), -4.0f32..4.0],
        clamp in 0.0f32..1.0,
        t in 0.0f32..1.0,
        busy in prop_oneof![Just(0), Just(EBUSY)],
    ) {
        let f = Fields {
            cx,
            cy,
            scale_type,
            alignment,
            mode,
            manual_val: val,
            manual_target: target,
            manual_torque: torque,
            manual_clamp: clamp,
            ..Fields::default()
        };
        let tr = Tr::with(f);
        tr.tex_busy.set(busy);
        if let Some((w, h)) = a {
            tr.child(0, 1, w, h);
        }
        if let Some((w, h)) = b {
            tr.child(1, 2, w, h);
        }
        let want = model_tick(&tr.fields(), &tr.sizes.borrow(), t, busy);
        tick(&tr, t);
        prop_assert_eq!(tr.events(), want);
        tr.assert_unlocked();
    }
}

#[test]
fn matrix_room_past_int_saturates() {
    // The intentional difference: a room past int is undefined in the C's
    // (int) cast (x86-64 gives INT_MIN, arm64 saturates); here it saturates
    // on every host.
    let tr = Tr::with(sized(u32::MAX, 100));
    tr.child(0, 1, 100, 100);
    recalculate_matrix(&tr, 0);
    assert_eq!(tr.events()[4], Ev::Align(ALIGN_CENTER, i32::MAX, 0));

    let tr = Tr::with(Fields {
        scale_type: 7,
        ..sized(200, 100)
    });
    tr.child(0, 1, u32::MAX, 100);
    recalculate_matrix(&tr, 0);
    assert_eq!(tr.events()[4], Ev::Align(ALIGN_CENTER, i32::MIN, 0));
}
