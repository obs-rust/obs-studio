//! Tier 3 for `obs_source_seam::ffi::source_transition_restore` against
//! the original `libobs/obs-source-transition-restore.c.h`
//! (`obs_c_oracle::source_transition_restore`, renamed `oracle_*`).
//!
//! Both sides load the same saved data (a real `obs_data_t` of the obs-data
//! port, with any of its six keys missing) into the same transition, built
//! afresh for each: a zeroed `struct obs_source` with the transition type,
//! a name and both transition mutexes, its slots holding test sources and
//! its fields set through the seam. A test source is an opaque pointer with
//! a name, a width and a height, a reference count, an active-child link
//! count and whether the add refuses it. The C calls the oracle's recording
//! stand-ins for `obs_get_source_by_name()`, the active-child add and
//! remove, `obs_source_release()`, `obs_source_get_width()` and
//! `obs_source_get_height()`; the Rust shim is run with [`Oracle`], whose
//! `Calls` are those same stand-ins.
//!
//! After the load the two must agree on both slots, slot A's active flag,
//! the alignment, mode, scale type and size written, the actual size and
//! both matrices the recalculation leaves (bit for bit), every reference
//! and link count, that the lock is not left held, and the calls in order.
//! The inputs are fixed cases (each source outcome, an old slot A active or
//! not, the same source loaded again, missing keys, out-of-range ints) and
//! a proptest over random saved data and transitions. In a child process,
//! each warning matches the C's line and level.
//!
//! One intentional difference, the sizing's: a loaded width or height of
//! 2^31 or more (a saved -1 is 2^32 - 1) leaves a child a room past `int`,
//! whose `(int)` cast is undefined in C (x86-64 gives `INT_MIN`, arm64
//! saturates); the Rust saturates on every host. The matrix of a slot whose
//! own room is past `int` is not compared; every other matrix, and
//! everything else, is. The order of the lock calls is not
//! visible here (the seam calls pthread itself); the Tier 1 tests pin it.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

use obs_c_oracle::parity::cases;
use obs_c_oracle::source_transition_restore::{self as c, seam as seam_r};
use obs_c_oracle::source_transition_size::{self as cs, Matrix4, seam};
use obs_source::transition_clock as clock;
use obs_source::transition_render as render;
use obs_source_seam::ffi::source_transition_restore::{self as rs, Calls};
use obs_source_seam::ffi::source_transition_size::Calls as SizeCalls;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

/// The oracle graphs are process-wide, and the sizing's is shared with its
/// parity test: hold its lock.
use super::transition_size_parity::graph_lock;

const SOURCES: usize = 4;
/// The names of the test sources; the last has the empty name.
const NAMES: [&CStr; SOURCES] = [c"a", c"b", c"c", c""];

/// Opaque test sources: addresses never dereferenced.
static SOURCE_BYTES: [u8; SOURCES] = [0; SOURCES];

fn source(i: usize) -> *mut c_void {
    ptr::from_ref(&SOURCE_BYTES[i]).cast_mut().cast()
}

fn index(p: *mut c_void) -> Option<usize> {
    (0..SOURCES).find(|&i| source(i) == p)
}

const KEY_SOURCE_A: &CStr = c"transition_source_a";
const KEYS: [&CStr; 5] = [
    c"transition_alignment",
    c"transition_mode",
    c"transition_scale_type",
    c"transition_cx",
    c"transition_cy",
];

unsafe extern "C" {
    fn obs_data_create() -> *mut c_void;
    fn obs_data_release(data: *mut c_void);
    fn obs_data_set_string(data: *mut c_void, name: *const c_char, val: *const c_char);
    fn obs_data_set_int(data: *mut c_void, name: *const c_char, val: i64);
}

/// The oracles' stand-ins, as the Rust shim's libobs calls.
enum Oracle {}

// SAFETY: each is the stand-in the C oracle calls under the same name.
unsafe impl SizeCalls for Oracle {
    unsafe fn get_width(source: *mut c_void) -> u32 {
        // SAFETY: see the trait.
        unsafe { cs::oracle_size_get_width(source) }
    }
    unsafe fn get_height(source: *mut c_void) -> u32 {
        // SAFETY: see the trait.
        unsafe { cs::oracle_size_get_height(source) }
    }
    unsafe fn texrender_reset(texrender: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { cs::oracle_size_texrender_reset(texrender) }
    }
}

// SAFETY: as above.
unsafe impl Calls for Oracle {
    unsafe fn get_source_by_name(name: *const c_char) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { c::oracle_restore_get_source_by_name(name) }
    }
    unsafe fn add_active_child(parent: *mut c_void, child: *mut c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { c::oracle_restore_add_active_child(parent, child) }
    }
    unsafe fn remove_active_child(parent: *mut c_void, child: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_restore_remove_active_child(parent, child) }
    }
    unsafe fn release(source: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { c::oracle_restore_release(source) }
    }
}

/// The saved data: the name of slot A's source (`None`: the key is missing)
/// and the five ints (`None`: missing).
#[derive(Debug, Clone, PartialEq)]
struct Saved {
    name: Option<&'static CStr>,
    ints: [Option<i64>; 5],
}

/// How a transition and its sources start.
#[derive(Debug, Clone, PartialEq)]
struct Init {
    /// The old slot A and whether it is active.
    slot_a: Option<(usize, bool)>,
    slot_b: Option<usize>,
    /// Each source's width and height.
    sizes: [(u32, u32); SOURCES],
    /// The sources whose add the transition refuses.
    refuses: [bool; SOURCES],
    /// The fields before the load: the alignment, mode, scale type and
    /// size.
    fields: (u32, c_int, c_int, u32, u32),
}

impl Default for Init {
    fn default() -> Self {
        Self {
            slot_a: None,
            slot_b: None,
            sizes: [(100, 50), (320, 240), (1920, 1080), (0, 0)],
            refuses: [false; SOURCES],
            fields: (5, 1, 2, 640, 480),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    C,
    Rust,
}

/// What a side shows after the load.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    slots: [Option<usize>; 2],
    active_a: bool,
    fields: (u32, c_int, c_int, u32, u32),
    actual: (u32, u32),
    matrices: [[u32; 16]; 2],
    refs: [i64; SOURCES],
    links: [i64; SOURCES],
    unlocked: bool,
    /// The restore stand-ins' calls: kind, source, whether the parent is
    /// the transition, result.
    calls: Vec<(c_int, Option<usize>, bool, c_int)>,
    /// The size stand-ins' calls: kind and source.
    size_calls: Vec<(c_int, Option<usize>)>,
}

/// A fresh test transition and its sources, from `init`.
///
/// # Safety
/// The graphs are held by the caller.
unsafe fn setup(init: &Init) -> *mut c_void {
    // SAFETY: the caller's contract; every pointer is a live test
    // transition or an opaque test source.
    unsafe {
        cs::oracle_size_reset();
        c::oracle_restore_reset();
        let tr = cs::oracle_size_transition_new(c"saved transition".as_ptr());
        assert!(!tr.is_null());
        for (i, ((name, &(cx, cy)), &refuses)) in
            NAMES.iter().zip(&init.sizes).zip(&init.refuses).enumerate()
        {
            let held = i64::from(init.slot_a.is_some_and(|(a, _)| a == i))
                + i64::from(init.slot_b == Some(i));
            let linked = i64::from(init.slot_a == Some((i, true)));
            assert!(cs::oracle_size_set_child(source(i), cx, cy));
            assert!(c::oracle_restore_add_source(
                source(i),
                name.as_ptr(),
                1 + held,
                linked,
                refuses
            ));
        }
        let (a, active) = init
            .slot_a
            .map_or((ptr::null_mut(), false), |(i, on)| (source(i), on));
        seam::obs_source_transition_seam_set_sources(tr, 0, a);
        seam_r::obs_source_transition_seam_set_source_active(tr, 0, active);
        seam::obs_source_transition_seam_set_sources(
            tr,
            1,
            init.slot_b.map_or(ptr::null_mut(), source),
        );
        for i in 0..2 {
            seam::obs_source_transition_seam_set_matrix(tr, i, &Matrix4([7.0; 16]));
        }
        let (align, mode, scale_type, set_cx, set_cy) = init.fields;
        seam::obs_source_transition_seam_set_alignment(tr, align);
        seam::obs_source_transition_seam_set_mode(tr, mode);
        seam::obs_source_transition_seam_set_scale_type(tr, scale_type);
        seam::obs_source_transition_seam_set_cx(tr, set_cx);
        seam::obs_source_transition_seam_set_cy(tr, set_cy);
        tr
    }
}

/// Loads `saved` into a fresh transition from `init` with one side's load.
fn run_side(side: Side, init: &Init, saved: &Saved) -> Seen {
    // SAFETY: the graphs are held by the caller; every pointer is a live
    // test transition, an opaque test source or the data made here.
    unsafe {
        let tr = setup(init);
        let data = obs_data_create();
        if let Some(name) = saved.name {
            obs_data_set_string(data, KEY_SOURCE_A.as_ptr(), name.as_ptr());
        }
        for (key, v) in KEYS.iter().zip(saved.ints) {
            if let Some(v) = v {
                obs_data_set_int(data, key.as_ptr(), v);
            }
        }

        match side {
            Side::C => c::oracle_obs_transition_load(tr, data),
            Side::Rust => rs::load::<Oracle>(tr, data),
        }
        obs_data_release(data);

        let mut m = [Matrix4::default(); 2];
        for (i, out) in m.iter_mut().enumerate() {
            seam::obs_source_transition_seam_get_matrix(tr, i, out);
        }
        let unlocked = seam::obs_source_transition_seam_trylock(tr) == 0;
        if unlocked {
            seam::obs_source_transition_seam_unlock(tr);
        }
        let mut calls = Vec::new();
        for i in 0..c::oracle_restore_traced() {
            let (mut kind, mut s, mut p, mut r) = (0, ptr::null_mut(), ptr::null_mut(), 0);
            c::oracle_restore_event(i, &raw mut kind, &raw mut s, &raw mut p, &raw mut r);
            calls.push((kind, index(s), p == tr, r));
        }
        let mut size_calls = Vec::new();
        for i in 0..cs::oracle_size_traced() {
            let (mut kind, mut s) = (0, ptr::null_mut());
            cs::oracle_size_event(i, &raw mut kind, &raw mut s);
            size_calls.push((kind, index(s)));
        }
        let seen = Seen {
            slots: [
                index(seam::obs_source_transition_seam_get_sources(tr, 0)),
                index(seam::obs_source_transition_seam_get_sources(tr, 1)),
            ],
            active_a: seam_r::obs_source_transition_seam_get_source_active(tr, 0),
            fields: (
                seam::obs_source_transition_seam_get_alignment(tr),
                seam::obs_source_transition_seam_get_mode(tr),
                seam::obs_source_transition_seam_get_scale_type(tr),
                seam::obs_source_transition_seam_get_cx(tr),
                seam::obs_source_transition_seam_get_cy(tr),
            ),
            actual: (
                seam::obs_source_transition_seam_get_actual_cx(tr),
                seam::obs_source_transition_seam_get_actual_cy(tr),
            ),
            matrices: [m[0].0.map(f32::to_bits), m[1].0.map(f32::to_bits)],
            refs: std::array::from_fn(|i| c::oracle_restore_refs(source(i))),
            links: std::array::from_fn(|i| c::oracle_restore_links(source(i))),
            unlocked,
            calls,
            size_calls,
        };
        cs::oracle_size_reset();
        c::oracle_restore_reset();
        seen
    }
}

/// Whether the room the C's `(int)` casts take for slot `idx` fits in
/// `int`, where they are defined: the offsets of the child the slot holds
/// after the load, from the transition's size (the written width and height,
/// or the actual size where one is 0) as the C's `float`, the child's size,
/// the scale type and the fit, by the rules the C file asks
/// (`transition_clock::{size, fit}`, `transition_render::offset`). A slot
/// with no child, or a child with no size, builds no matrix and is always
/// compared.
#[allow(clippy::cast_precision_loss)]
fn room_defined(init: &Init, seen: &Seen, idx: usize) -> bool {
    let Some(child) = seen.slots[idx] else {
        return true;
    };
    let (_, _, scale_type, cx, cy) = seen.fields;
    let tr_cx = clock::size(cx, seen.actual.0) as f32;
    let tr_cy = clock::size(cy, seen.actual.1) as f32;
    let (source_cx, source_cy) = (init.sizes[child].0 as f32, init.sizes[child].1 as f32);
    let (x, y) = match clock::fit(tr_cx, tr_cy, source_cx, source_cy, scale_type) {
        clock::Fit::NoMatrix => return true,
        clock::Fit::Scale { x, y } => (x, y),
        clock::Fit::Unset => (1.0, 1.0),
    };
    let room = render::offset(tr_cx, tr_cy, source_cx, source_cy, x, y);
    let fits = |v: f32| (-2_147_483_648.0..2_147_483_648.0).contains(&v);
    fits(room.x) && fits(room.y)
}

/// The two sides' [`Seen`], with a slot's matrix left out only when its
/// room is past `int` (the intentional difference: undefined in C,
/// saturated in Rust). The C's slots, fields and actual size decide it;
/// they are compared too.
fn both(init: &Init, saved: &Saved) -> (Seen, Seen) {
    let _g = graph_lock();
    let mut want = run_side(Side::C, init, saved);
    let mut got = run_side(Side::Rust, init, saved);
    for idx in 0..2 {
        if !room_defined(init, &want, idx) {
            want.matrices[idx] = [0; 16];
            got.matrices[idx] = [0; 16];
        }
    }
    (want, got)
}

fn check(init: &Init, saved: &Saved) -> Seen {
    let (want, got) = both(init, saved);
    assert_eq!(got, want, "{init:?} {saved:?}");
    want
}

fn saved(name: Option<&'static CStr>) -> Saved {
    Saved {
        name,
        ints: [Some(0), Some(0), Some(1), Some(1280), Some(720)],
    }
}

#[test]
fn source_outcomes() {
    // found and kept, in each old slot A
    for slot_a in [None, Some((1, true)), Some((1, false)), Some((0, true))] {
        for name in [c"a", c"b", c"c"] {
            let init = Init {
                slot_a,
                slot_b: Some(2),
                ..Init::default()
            };
            let seen = check(&init, &saved(Some(name)));
            assert!(seen.active_a);
        }
    }
    // refused, not found, the empty name of a source that has it, the
    // missing key ("" is looked up)
    for (name, refuses) in [
        (Some(c"a"), [true, false, false, false]),
        (Some(c"gone"), [false; SOURCES]),
        (Some(c""), [false; SOURCES]),
        (Some(c""), [false, false, false, true]),
        (None, [false; SOURCES]),
    ] {
        for slot_a in [None, Some((1, true)), Some((0, false))] {
            let init = Init {
                slot_a,
                refuses,
                ..Init::default()
            };
            let seen = check(&init, &saved(name));
            assert!(seen.active_a);
        }
    }
}

#[test]
fn fields_and_casts() {
    let values = [
        0,
        1,
        2,
        3,
        -1,
        15,
        1 << 31,
        (1 << 32) | 5,
        i64::from(u32::MAX),
        i64::MIN,
        i64::MAX,
    ];
    for v in values {
        for k in 0..5 {
            let mut s = saved(Some(c"b"));
            s.ints[k] = Some(v);
            check(
                &Init {
                    slot_b: Some(0),
                    ..Init::default()
                },
                &s,
            );
        }
    }
    // every key missing: 0s
    let seen = check(
        &Init::default(),
        &Saved {
            name: None,
            ints: [None; 5],
        },
    );
    assert_eq!(seen.fields, (0, 0, 0, 0, 0));
}

/// A loaded size past 2^31 compares every matrix whose own room fits in
/// `int`: stretch fills the transition (no room), and so does an aspect fit
/// of a child of the same shape; max only leaves a child its size, a room
/// past `int`, so only that one is left out.
#[test]
fn huge_sizes_compare_defined_matrices() {
    let huge = i64::from(u32::MAX);
    for (scale_type, defined) in [(2, true), (0, false)] {
        let init = Init {
            slot_b: Some(1),
            ..Init::default()
        };
        let s = Saved {
            name: Some(c"b"),
            ints: [Some(0), Some(0), Some(scale_type), Some(huge), Some(huge)],
        };
        let (want, got) = both(&init, &s);
        assert_eq!(got, want);
        for idx in 0..2 {
            assert_eq!(
                room_defined(&init, &want, idx),
                defined,
                "{scale_type} {idx}"
            );
            assert_eq!(
                want.matrices[idx] == [0; 16],
                !defined,
                "{scale_type} {idx}"
            );
        }
    }
}

fn any_int() -> impl Strategy<Value = Option<i64>> {
    prop_oneof![
        1 => Just(None),
        3 => (-3i64..=3).prop_map(Some),
        2 => (0i64..4096).prop_map(Some),
        1 => any::<i64>().prop_map(Some),
    ]
}

fn any_name() -> impl Strategy<Value = Option<&'static CStr>> {
    prop_oneof![
        Just(None),
        Just(Some(c"a")),
        Just(Some(c"b")),
        Just(Some(c"c")),
        Just(Some(c"")),
        Just(Some(c"gone")),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(256)))]

    #[test]
    fn load_matches_the_c(
        name in any_name(),
        ints in prop::array::uniform5(any_int()),
        slot_a in prop_oneof![Just(None), ((0usize..SOURCES), any::<bool>()).prop_map(Some)],
        slot_b in prop_oneof![Just(None), (0usize..SOURCES).prop_map(Some)],
        sizes in prop::array::uniform4((0u32..4096, 0u32..4096)),
        refuses in any::<[bool; SOURCES]>(),
        fields in (0u32..16, -1i32..3, 0i32..3, 0u32..4096, 0u32..4096),
    ) {
        let init = Init { slot_a, slot_b, sizes, refuses, fields };
        let s = Saved { name, ints };
        let (want, got) = both(&init, &s);
        prop_assert_eq!(got, want);
    }
}

/// The log record of oracle/base_drive.c's recorder.
#[repr(C)]
struct Rec {
    calls: c_int,
    level: c_int,
    message: [c_char; 256],
}

unsafe extern "C" {
    fn base_test_record_ptr() -> *mut c_void;
    fn base_test_record_only_this_thread();
}

/// The process-wide log handler, pointed at a record while it lives.
struct Capture {
    rec: Box<Rec>,
    saved: (*mut c_void, *mut c_void),
}

impl Capture {
    fn arm() -> Self {
        let mut saved = (ptr::null_mut(), ptr::null_mut());
        let mut rec = Box::new(Rec {
            calls: 0,
            level: -1,
            message: [0; 256],
        });
        // SAFETY: the record outlives the handler (restored in drop).
        unsafe {
            base_get_log_handler(&raw mut saved.0, &raw mut saved.1);
            base_test_record_only_this_thread();
            base_set_log_handler(base_test_record_ptr(), (&raw mut *rec).cast());
        }
        Self { rec, saved }
    }

    /// The calls since the last take, the level and text of the last line.
    fn take(&mut self) -> (c_int, c_int, String) {
        // SAFETY: the recorder NUL-terminates what it writes; the record
        // starts zeroed.
        let text = unsafe { CStr::from_ptr(self.rec.message.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        let line = (self.rec.calls, self.rec.level, text);
        self.rec.calls = 0;
        self.rec.level = -1;
        self.rec.message[0] = 0;
        line
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // SAFETY: the handler read in `arm`.
        unsafe { base_set_log_handler(self.saved.0, self.saved.1) };
    }
}

/// `LOG_WARNING`.
const LOG_WARNING: c_int = 200;

/// A refused source (the one named "" too, which the missing key reads) and
/// a missing one each log one warning naming the transition and the
/// source, as the C does; a kept one logs nothing.
#[test]
fn warnings_log() {
    if !crate::output::isolated(concat!(module_path!(), "::warnings_log")) {
        return;
    }
    let mut log = Capture::arm();
    let cases = [
        (
            Some(c"a"),
            true,
            "Cannot set transition 'saved transition' to source 'a' due to infinite recursion",
        ),
        (
            Some(c"gone"),
            false,
            "Failed to find source 'gone' for transition 'saved transition'",
        ),
        // the missing key reads "", and the source named "" is refused
        (
            None,
            true,
            "Cannot set transition 'saved transition' to source '' due to infinite recursion",
        ),
    ];
    for (name, refuse, want) in cases {
        let refused = if name.is_some() { 0 } else { SOURCES - 1 };
        let mut refuses = [false; SOURCES];
        refuses[refused] = refuse;
        let init = Init {
            refuses,
            ..Init::default()
        };
        let mut lines = Vec::new();
        for side in [Side::C, Side::Rust] {
            let _g = graph_lock();
            run_side(side, &init, &saved(name));
            lines.push(log.take());
        }
        assert_eq!(lines[0], (1, LOG_WARNING, want.to_owned()), "the C's line");
        assert_eq!(lines[1], lines[0], "the Rust line");
    }
    for side in [Side::C, Side::Rust] {
        let _g = graph_lock();
        run_side(side, &Init::default(), &saved(Some(c"b")));
        assert_eq!(log.take().0, 0);
    }
}
