//! Tier 3 for `obs_source_seam::ffi::source_transition_size` against the
//! original `libobs/obs-source-transition-size.c.h`
//! (`obs_c_oracle::source_transition_size`, renamed `oracle_*`).
//!
//! Both sides run on the same transition, built afresh for each: a zeroed
//! `struct obs_source` with the transition type and both transition
//! mutexes, its slots holding opaque children with a width and a height,
//! and its sizes, scale type, alignment, mode, manual fields, render
//! targets and matrices set through the seam. The C calls the oracle's
//! recording stand-ins for `obs_source_get_width()`,
//! `obs_source_get_height()` and `gs_texrender_reset()`; the Rust shims are
//! run with [`Oracle`], whose `Calls` are those same stand-ins. Both reach
//! the fields and locks through the same C transition seam, and both call
//! the same `add_alignment()`, `calc_torquef()` and matrix functions, as in
//! the hybrid libobs.
//!
//! After every call the two must agree on what it returned, the actual
//! size, the manual value, both matrices bit for bit (a slot with no matrix
//! keeps the sentinel it started with), that neither mutex is left held,
//! and the calls in order. The inputs are fixed cases (each scale type and
//! alignment, an empty slot, a child with no size, the unsized transition,
//! a set size, each manual step, a held texture lock) and a proptest over
//! random sizes, scale types, alignments, modes, manual fields, tick times
//! and sequences of calls with children resized between them.
//!
//! One intentional difference: the `(int)` of the room left for a child
//! saturates past `int`, where the C is undefined (x86-64 gives `INT_MIN`,
//! arm64 saturates); only a transition size of about 2^31 or more reaches
//! it. Sizes here stay below 2^24, where both agree, and the Tier 1 tests
//! pin the saturation. The order of the lock calls is not visible here (the
//! seam calls pthread itself); the Tier 1 tests pin it.

use std::ffi::{c_int, c_void};
use std::ptr;
use std::sync::{Mutex, MutexGuard, PoisonError};

use obs_c_oracle::parity::cases;
use obs_c_oracle::source_transition_size::{self as c, Matrix4, seam};
use obs_source_seam::ffi::source_transition_size::{self as rs, Calls};
use proptest::prelude::*;

/// The oracle graph is process-wide; the load's parity test
/// (`transition_restore_parity`), which uses it too, takes this lock.
static GRAPH: Mutex<()> = Mutex::new(());

pub(super) fn graph_lock() -> MutexGuard<'static, ()> {
    GRAPH.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Opaque children and render targets: addresses never dereferenced.
static CHILDREN: [u8; 3] = [0; 3];
static TEXRENDERS: [u8; 2] = [0; 2];

fn child(i: usize) -> *mut c_void {
    ptr::from_ref(&CHILDREN[i]).cast_mut().cast()
}

fn texrender(i: usize) -> *mut c_void {
    ptr::from_ref(&TEXRENDERS[i]).cast_mut().cast()
}

/// What a slot with no matrix keeps.
const SENTINEL: Matrix4 = Matrix4([7.0; 16]);

/// The oracle's stand-ins, as the Rust shims' libobs calls.
enum Oracle {}

// SAFETY: each is the stand-in the C oracle calls under the same name.
unsafe impl Calls for Oracle {
    unsafe fn get_width(source: *mut c_void) -> u32 {
        // SAFETY: see the trait.
        unsafe { c::oracle_size_get_width(source) }
    }
    unsafe fn get_height(source: *mut c_void) -> u32 {
        // SAFETY: see the trait.
        unsafe { c::oracle_size_get_height(source) }
    }
    unsafe fn texrender_reset(texrender: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_size_texrender_reset(texrender) }
    }
}

/// How a transition starts.
#[derive(Debug, Clone, PartialEq)]
struct Init {
    /// Which child each slot holds (0, 1 or 2), if any.
    slots: [Option<usize>; 2],
    /// Each child's width and height.
    sizes: [(u32, u32); 3],
    cx: u32,
    cy: u32,
    actual: (u32, u32),
    scale_type: c_int,
    alignment: u32,
    mode: c_int,
    val: f32,
    target: f32,
    torque: f32,
    clamp: f32,
    /// The texture lock is held through the calls.
    tex_busy: bool,
}

impl Default for Init {
    fn default() -> Self {
        Self {
            slots: [None; 2],
            sizes: [(0, 0); 3],
            cx: 0,
            cy: 0,
            actual: (0, 0),
            scale_type: 0,
            alignment: 0,
            mode: 0,
            val: 0.0,
            target: 0.0,
            torque: 0.0,
            clamp: 0.0,
            tex_busy: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Tick(f32),
    Size,
    Matrices,
    Cx,
    Cy,
    /// Child `.0` reports a new size.
    Resize(usize, u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    C,
    Rust,
}

/// What a side shows after a call.
#[derive(Debug, Clone, PartialEq, Eq)]
struct State {
    ret: Option<u32>,
    actual: (u32, u32),
    manual_val: u32,
    matrices: [[u32; 16]; 2],
    unlocked: bool,
}

fn bits(m: &Matrix4) -> [u32; 16] {
    m.0.map(f32::to_bits)
}

/// Reads a side's state.
///
/// # Safety
/// `tr` is a live test transition, the texture lock held only when
/// `tex_busy`.
unsafe fn observe(tr: *mut c_void, ret: Option<u32>, tex_busy: bool) -> State {
    // SAFETY: the caller's contract; the seam reads one field each.
    unsafe {
        let mut m = [Matrix4::default(); 2];
        for (i, out) in m.iter_mut().enumerate() {
            seam::obs_source_transition_seam_get_matrix(tr, i, out);
        }
        let free = seam::obs_source_transition_seam_trylock(tr) == 0;
        if free {
            seam::obs_source_transition_seam_unlock(tr);
        }
        let tex_free = tex_busy || seam::obs_source_transition_seam_trylock_textures(tr) == 0;
        if tex_free && !tex_busy {
            seam::obs_source_transition_seam_unlock_textures(tr);
        }
        State {
            ret,
            actual: (
                seam::obs_source_transition_seam_get_actual_cx(tr),
                seam::obs_source_transition_seam_get_actual_cy(tr),
            ),
            manual_val: seam::obs_source_transition_seam_get_manual_val(tr).to_bits(),
            matrices: [bits(&m[0]), bits(&m[1])],
            unlocked: free && tex_free,
        }
    }
}

/// Runs `ops` on a fresh transition from `init` with one side's functions:
/// the state after each call, and the calls the stand-ins saw.
fn run_side(side: Side, init: &Init, ops: &[Op]) -> (Vec<State>, Vec<(c_int, usize)>) {
    // SAFETY: the graph is held by the caller; every pointer is a live test
    // transition or an opaque child or render target.
    unsafe {
        c::oracle_size_reset();
        let tr = c::oracle_size_transition_new(c"transition".as_ptr());
        assert!(!tr.is_null());
        for (i, &(cx, cy)) in init.sizes.iter().enumerate() {
            assert!(c::oracle_size_set_child(child(i), cx, cy));
        }
        for (i, s) in init.slots.iter().enumerate() {
            seam::obs_source_transition_seam_set_sources(tr, i, s.map_or(ptr::null_mut(), child));
            seam::obs_source_transition_seam_set_texrender(tr, i, texrender(i));
            seam::obs_source_transition_seam_set_matrix(tr, i, &SENTINEL);
        }
        seam::obs_source_transition_seam_set_cx(tr, init.cx);
        seam::obs_source_transition_seam_set_cy(tr, init.cy);
        seam::obs_source_transition_seam_set_actual_cx(tr, init.actual.0);
        seam::obs_source_transition_seam_set_actual_cy(tr, init.actual.1);
        seam::obs_source_transition_seam_set_scale_type(tr, init.scale_type);
        seam::obs_source_transition_seam_set_alignment(tr, init.alignment);
        seam::obs_source_transition_seam_set_mode(tr, init.mode);
        seam::obs_source_transition_seam_set_manual_val(tr, init.val);
        seam::obs_source_transition_seam_set_manual_target(tr, init.target);
        seam::obs_source_transition_seam_set_manual_torque(tr, init.torque);
        seam::obs_source_transition_seam_set_manual_clamp(tr, init.clamp);
        if init.tex_busy {
            seam::obs_source_transition_seam_lock_textures(tr);
        }

        let mut states = Vec::new();
        for &op in ops {
            let ret = match (side, op) {
                (Side::C, Op::Tick(t)) => {
                    c::oracle_obs_transition_tick(tr, t);
                    None
                }
                (Side::Rust, Op::Tick(t)) => {
                    rs::tick::<Oracle>(tr, t);
                    None
                }
                (Side::C, Op::Size) => {
                    c::oracle_transition_recalculate_size(tr);
                    None
                }
                (Side::Rust, Op::Size) => {
                    rs::recalculate_size::<Oracle>(tr);
                    None
                }
                (Side::C, Op::Matrices) => {
                    c::oracle_transition_recalculate_matrices(tr);
                    None
                }
                (Side::Rust, Op::Matrices) => {
                    rs::recalculate_matrices::<Oracle>(tr);
                    None
                }
                (Side::C, Op::Cx) => Some(c::oracle_transition_get_cx(tr)),
                (Side::Rust, Op::Cx) => Some(rs::get_cx::<Oracle>(tr)),
                (Side::C, Op::Cy) => Some(c::oracle_transition_get_cy(tr)),
                (Side::Rust, Op::Cy) => Some(rs::get_cy::<Oracle>(tr)),
                (_, Op::Resize(i, cx, cy)) => {
                    assert!(c::oracle_size_set_child(child(i), cx, cy));
                    None
                }
            };
            states.push(observe(tr, ret, init.tex_busy));
        }

        if init.tex_busy {
            seam::obs_source_transition_seam_unlock_textures(tr);
        }
        let mut trace = Vec::new();
        for i in 0..c::oracle_size_traced() {
            let mut kind = 0;
            let mut a = ptr::null_mut();
            c::oracle_size_event(i, &raw mut kind, &raw mut a);
            let which = if kind == c::RESET {
                (0..2).find(|&j| texrender(j) == a)
            } else {
                (0..3).find(|&j| child(j) == a)
            };
            trace.push((kind, which.unwrap_or(usize::MAX)));
        }
        c::oracle_size_reset();
        (states, trace)
    }
}

fn check(init: &Init, ops: &[Op]) {
    let _g = graph_lock();
    let want = run_side(Side::C, init, ops);
    let got = run_side(Side::Rust, init, ops);
    assert_eq!(got, want, "{init:?} {ops:?}");
}

const ALL: [Op; 5] = [Op::Cx, Op::Cy, Op::Size, Op::Matrices, Op::Tick(0.25)];

#[test]
fn scale_types_and_alignments() {
    for scale_type in [-1, 0, 1, 2, 3] {
        for alignment in [0, 1, 2, 4, 5, 8, 10, 15] {
            for sizes in [
                (100, 100),
                (400, 100),
                (100, 50),
                (300, 300),
                (99, 49),
                (1, 3),
            ] {
                let init = Init {
                    slots: [Some(0), Some(1)],
                    sizes: [sizes, (sizes.1, sizes.0), (0, 0)],
                    cx: 200,
                    cy: 100,
                    scale_type,
                    alignment,
                    ..Init::default()
                };
                check(&init, &ALL);
            }
        }
    }
}

#[test]
fn slots_and_sizes() {
    let cases = [
        // empty
        Init::default(),
        // one child, either slot; the same child in both
        Init {
            slots: [Some(0), None],
            sizes: [(320, 240), (0, 0), (0, 0)],
            ..Init::default()
        },
        Init {
            slots: [None, Some(1)],
            sizes: [(0, 0), (320, 240), (0, 0)],
            ..Init::default()
        },
        Init {
            slots: [Some(2), Some(2)],
            sizes: [(0, 0), (0, 0), (640, 360)],
            ..Init::default()
        },
        // a child with no width, no height
        Init {
            slots: [Some(0), Some(1)],
            sizes: [(0, 50), (50, 0), (0, 0)],
            cx: 100,
            cy: 100,
            ..Init::default()
        },
        // the actual size stands in, a set size wins, each on its own
        Init {
            slots: [Some(0), Some(1)],
            sizes: [(100, 50), (80, 90), (0, 0)],
            actual: (999, 999),
            ..Init::default()
        },
        Init {
            slots: [Some(0), Some(1)],
            sizes: [(100, 50), (80, 90), (0, 0)],
            cx: 1920,
            ..Init::default()
        },
        Init {
            slots: [Some(0), Some(1)],
            sizes: [(100, 50), (80, 90), (0, 0)],
            cy: 1080,
            scale_type: 2,
            ..Init::default()
        },
    ];
    for init in cases {
        check(&init, &ALL);
        check(
            &init,
            &[Op::Matrices, Op::Size, Op::Matrices, Op::Tick(0.0)],
        );
        check(
            &init,
            &[
                Op::Tick(0.1),
                Op::Resize(0, 7, 9),
                Op::Tick(0.1),
                Op::Resize(1, 0, 0),
                Op::Tick(0.1),
            ],
        );
    }
}

#[test]
fn manual_steps_and_texture_lock() {
    for mode in [0, 1, 2, -1] {
        for torque in [0.0, -0.0, 0.5, 2.0, -3.0, f32::NAN] {
            for (val, target, clamp) in [(0.0, 1.0, 0.01), (0.9, 0.1, 0.5), (0.5, 0.5, 0.0)] {
                for tex_busy in [false, true] {
                    let init = Init {
                        slots: [Some(0), None],
                        sizes: [(10, 10), (0, 0), (0, 0)],
                        mode,
                        val,
                        target,
                        torque,
                        clamp,
                        tex_busy,
                        ..Init::default()
                    };
                    check(&init, &[Op::Tick(0.016), Op::Tick(0.5), Op::Tick(1.0)]);
                }
            }
        }
    }
}

fn any_size() -> impl Strategy<Value = u32> {
    prop_oneof![Just(0u32), 1u32..=4, 1u32..=16_384, 1u32..(1 << 24)]
}

fn any_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (-1.0f32..2.0).prop_map(Op::Tick),
        1 => Just(Op::Size),
        1 => Just(Op::Matrices),
        1 => Just(Op::Cx),
        1 => Just(Op::Cy),
        2 => (0usize..3, any_size(), any_size()).prop_map(|(i, cx, cy)| Op::Resize(i, cx, cy)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(256)))]

    #[test]
    fn calls_match_the_c(
        slots in prop::array::uniform2(prop_oneof![Just(None), (0usize..3).prop_map(Some)]),
        sizes in prop::array::uniform3((any_size(), any_size())),
        cx in any_size(),
        cy in any_size(),
        actual in (any_size(), any_size()),
        scale_type in -1i32..=3,
        alignment in 0u32..16,
        mode in -1i32..=2,
        val in -2.0f32..2.0,
        target in -2.0f32..2.0,
        torque in prop_oneof![Just(0.0f32), -4.0f32..4.0],
        clamp in 0.0f32..1.0,
        tex_busy in any::<bool>(),
        ops in prop::collection::vec(any_op(), 1..8),
    ) {
        let init = Init {
            slots,
            sizes,
            cx,
            cy,
            actual,
            scale_type,
            alignment,
            mode,
            val,
            target,
            torque,
            clamp,
            tex_busy,
        };
        let _g = graph_lock();
        let want = run_side(Side::C, &init, &ops);
        let got = run_side(Side::Rust, &init, &ops);
        prop_assert_eq!(got, want);
    }
}
