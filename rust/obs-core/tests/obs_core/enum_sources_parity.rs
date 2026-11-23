//! Tier 3: the original extracted `obs_enum_sources()`,
//! `obs_enum_all_sources()`, `obs_enum_canvases()` and
//! `obs_canvas_enum_scenes()` versus the shims on one recording fixture core
//! (oracle/enum_sources.c), through the real-named adapters. The fixture
//! links the same real `struct obs_source` entries in the core's uuid table
//! in order and in canvas 1's own table in reverse, leaves the main canvas
//! empty and names a pool of real `struct obs_canvas`, so a walk of the
//! wrong table shows. Compared: the trace of every lock and unlock with its
//! mutex, every get_ref with its answer, every release and every callback
//! with the entry's count at the call (with its checks that each step runs
//! under the walk's lock, that the canvases' head is read before their lock
//! is taken and that the callback gets the caller's param), and the final
//! entries, links and counts, and every mutex released. Cases: the C test's
//! tables, every flag of one entry, early stops, the callback unlinking the
//! current entry or taking the next one's last reference, and proptest
//! tables.
//!
//! Excluded: an uninitialized core, a dead canvas or listed entry, a NULL
//! callback and a table change from another thread, outside the original's
//! contract. No intentional differences. Mutants show the probe catches
//! each broken step.
use core::ffi::{CStr, c_char, c_void};
use obs_core::ffi::enum_sources::{
    EnumProc, obs_canvas_enum_scenes, obs_enum_all_sources, obs_enum_canvases, obs_enum_sources,
};
use proptest::prelude::*;

const POOL: usize = 8;
const NEVER: u8 = 0xff;

const PRIVATE: u8 = 0x01;
const SCENE: u8 = 1 << 1;
const FILTER: u8 = 2 << 1;
const TRANSITION: u8 = 3 << 1;
const GROUP_ID: u8 = 0x08;
const DEAD: u8 = 0x10;
const GROUP: u8 = SCENE | GROUP_ID;

const SOURCES: u8 = 0;
const ALL: u8 = 1;
const CANVASES: u8 = 2;
const SCENES: u8 = 3;

type Enum = unsafe extern "C" fn(EnumProc, *mut c_void);
type CanvasEnum = unsafe extern "C" fn(*mut c_void, EnumProc, *mut c_void);

#[repr(C)]
#[derive(Clone, Copy)]
struct Candidates {
    sources: Enum,
    all: Enum,
    canvases: Enum,
    scenes: CanvasEnum,
}

const SHIMS: Candidates = Candidates {
    sources: obs_enum_sources,
    all: obs_enum_all_sources,
    canvases: obs_enum_canvases,
    scenes: obs_canvas_enum_scenes,
};

#[repr(C)]
struct Case {
    flags: *const u8,
    count: usize,
    walk: u8,
    stop_after: u8,
    unlink_at: u8,
    kill_next_at: u8,
}

unsafe extern "C" {
    fn oracle_enum_sources_probe(candidates: *const Candidates, case: *const Case) -> u32;
    fn oracle_enum_sources_original(case: *const Case, trace: *mut c_char, size: usize) -> bool;
    fn oracle_enum_sources_adapters() -> bool;
}

/// What the callback does, by zero-based visit.
#[derive(Debug, Clone, Copy)]
struct Actions {
    stop_after: u8,
    unlink_at: u8,
    kill_next_at: u8,
}

const NONE: Actions = Actions {
    stop_after: NEVER,
    unlink_at: NEVER,
    kill_next_at: NEVER,
};

fn stop(at: u8) -> Actions {
    Actions {
        stop_after: at,
        ..NONE
    }
}

fn case(flags: &[u8], walk: u8, actions: Actions) -> Case {
    Case {
        flags: flags.as_ptr(),
        count: flags.len(),
        walk,
        stop_after: actions.stop_after,
        unlink_at: actions.unlink_at,
        kill_next_at: actions.kill_next_at,
    }
}

fn probe_with(candidates: &Candidates, flags: &[u8], walk: u8, actions: Actions) -> u32 {
    let case = case(flags, walk, actions);
    // SAFETY: the oracle owns the fixture core every adapter reaches; the
    // flags outlive the call.
    unsafe { oracle_enum_sources_probe(candidates, &case) }
}

fn probe(flags: &[u8], walk: u8, actions: Actions) -> u32 {
    probe_with(&SHIMS, flags, walk, actions)
}

/// The original's trace.
fn original(flags: &[u8], walk: u8, actions: Actions) -> String {
    let case = case(flags, walk, actions);
    let mut trace = [0 as c_char; 1024];
    // SAFETY: the buffer is writable for its size; the oracle writes
    // NUL-terminated text.
    let released = unsafe { oracle_enum_sources_original(&case, trace.as_mut_ptr(), trace.len()) };
    assert!(released, "the original left a mutex held");
    // SAFETY: NUL-terminated by the oracle.
    unsafe { CStr::from_ptr(trace.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned()
}

/// The C test's populate(): IN_A, PRIV_IN, SCENE, FILTER, GROUP, IN_B,
/// PRIV_GROUP, PRIV_SCENE.
const POPULATED: [u8; 8] = [
    0,
    PRIVATE,
    SCENE,
    FILTER,
    GROUP,
    0,
    GROUP | PRIVATE,
    SCENE | PRIVATE,
];

#[test]
fn real_adapters_read_the_real_fields() {
    // SAFETY: the oracle arms and frees its own fixture.
    assert!(unsafe { oracle_enum_sources_adapters() });
}

#[test]
fn original_c_is_the_characterized_walk() {
    // Public inputs and groups only, each referenced for its call and
    // released before the next link is read.
    assert_eq!(
        original(&POPULATED, SOURCES, NONE),
        "Lsga+ca2ragb+rbgc+rcgd+rdge+ce2regf+cf2rfgg+rggh+rhUs"
    );
    // Every source, no reference.
    assert_eq!(
        original(&[0, PRIVATE | DEAD, TRANSITION], ALL, NONE),
        "Lsca1cb0cc1Us"
    );
    // The canvases' head before their lock.
    assert_eq!(original(&[0, 0, 0], CANVASES, NONE), "LncAcBcCUn");
    // Canvas 1's own table, reversed; a non-scene skipped, a stop releases.
    assert_eq!(
        original(&[SCENE, 0, GROUP], SCENES, stop(0)),
        "L1gc+cc2rcU1"
    );
}

/// Each walk's shim against the original over the named edges.
#[test]
fn characterized_edges() {
    let mut tables: Vec<Vec<u8>> = vec![
        vec![],
        POPULATED.to_vec(),
        vec![0, TRANSITION, 0],
        vec![DEAD, 0, GROUP, 0],
        vec![GROUP_ID, 0],
        vec![SCENE; 3],
        vec![SCENE, DEAD | SCENE, FILTER, GROUP, PRIVATE | SCENE],
    ];
    // Every flag combination of one entry.
    for flags in 0..0x20_u8 {
        tables.push(vec![flags]);
        tables.push(vec![flags, flags, 0]);
    }
    let mut actions = vec![NONE];
    for at in [0, 1, 2, 7] {
        actions.push(stop(at));
        actions.push(Actions {
            unlink_at: at,
            ..NONE
        });
        actions.push(Actions {
            kill_next_at: at,
            ..NONE
        });
    }
    actions.push(Actions {
        stop_after: 2,
        unlink_at: 0,
        kill_next_at: 1,
    });
    for walk in [SOURCES, ALL, CANVASES, SCENES] {
        for table in &tables {
            for &a in &actions {
                assert_eq!(probe(table, walk, a), 0, "{walk} {table:?} {a:?}");
            }
        }
    }
}

// Mutants: a walk that breaks one step must be caught.

unsafe extern "C" {
    fn obs_enum_sources_lock();
    fn obs_enum_sources_unlock();
    fn obs_enum_sources_first() -> *mut c_void;
    fn obs_enum_sources_next(source: *const c_void) -> *mut c_void;
    fn obs_enum_sources_get_ref(source: *mut c_void) -> *mut c_void;
    fn obs_enum_sources_release(source: *mut c_void);
    fn obs_enum_sources_canvases_lock();
    fn obs_enum_sources_canvases_unlock();
    fn obs_enum_sources_canvases_first() -> *mut c_void;
    fn obs_enum_sources_canvases_next(canvas: *const c_void) -> *mut c_void;
}

/// obs_enum_sources calling back every referenced source, private and
/// non-input ones included.
unsafe extern "C" fn every_source(enum_proc: EnumProc, param: *mut c_void) {
    let enum_proc = enum_proc.unwrap();
    // SAFETY: the fixture's adapters, on this thread's fixture.
    unsafe {
        obs_enum_sources_lock();
        let mut source = obs_enum_sources_first();
        while !source.is_null() {
            let s = obs_enum_sources_get_ref(source);
            if !s.is_null() {
                let more = enum_proc(param, s);
                obs_enum_sources_release(s);
                if !more {
                    break;
                }
            }
            source = obs_enum_sources_next(source);
        }
        obs_enum_sources_unlock();
    }
}

/// obs_enum_all_sources reading the next link after the callback.
unsafe extern "C" fn next_after_call(enum_proc: EnumProc, param: *mut c_void) {
    let enum_proc = enum_proc.unwrap();
    // SAFETY: as above.
    unsafe {
        obs_enum_sources_lock();
        let mut source = obs_enum_sources_first();
        while !source.is_null() {
            if !enum_proc(param, source) {
                break;
            }
            source = obs_enum_sources_next(source);
        }
        obs_enum_sources_unlock();
    }
}

/// obs_enum_canvases reading the head under the lock.
unsafe extern "C" fn head_under_lock(enum_proc: EnumProc, param: *mut c_void) {
    let enum_proc = enum_proc.unwrap();
    // SAFETY: as above.
    unsafe {
        obs_enum_sources_canvases_lock();
        let mut canvas = obs_enum_sources_canvases_first();
        while !canvas.is_null() {
            let next = obs_enum_sources_canvases_next(canvas);
            if !enum_proc(param, canvas) {
                break;
            }
            canvas = next;
        }
        obs_enum_sources_canvases_unlock();
    }
}

/// obs_canvas_enum_scenes on the core's uuid table.
unsafe extern "C" fn wrong_table(_canvas: *mut c_void, enum_proc: EnumProc, param: *mut c_void) {
    // SAFETY: as the shim.
    unsafe { obs_enum_all_sources(enum_proc, param) }
}

#[test]
fn mutants_are_caught() {
    let mutant = |f: fn(&mut Candidates)| {
        let mut c = SHIMS;
        f(&mut c);
        c
    };
    let every = mutant(|c| c.sources = every_source);
    assert_ne!(probe_with(&every, &POPULATED, SOURCES, NONE), 0);
    let late = mutant(|c| c.all = next_after_call);
    assert_ne!(
        probe_with(
            &late,
            &[0, 0, 0],
            ALL,
            Actions {
                unlink_at: 1,
                ..NONE
            }
        ),
        0
    );
    let locked = mutant(|c| c.canvases = head_under_lock);
    assert_ne!(probe_with(&locked, &[0, 0], CANVASES, NONE), 0);
    let wrong = mutant(|c| c.scenes = wrong_table);
    assert_ne!(probe_with(&wrong, &[SCENE, SCENE], SCENES, NONE), 0);
    // The shims themselves match on the same cases.
    assert_eq!(probe(&POPULATED, SOURCES, NONE), 0);
    assert_eq!(
        probe(
            &[0, 0, 0],
            ALL,
            Actions {
                unlink_at: 1,
                ..NONE
            }
        ),
        0
    );
    assert_eq!(probe(&[0, 0], CANVASES, NONE), 0);
    assert_eq!(probe(&[SCENE, SCENE], SCENES, NONE), 0);
}

fn actions() -> impl Strategy<Value = Actions> {
    let at = || prop_oneof![1 => Just(NEVER), 3 => 0..POOL as u8];
    (at(), at(), at()).prop_map(|(stop_after, unlink_at, kill_next_at)| Actions {
        stop_after,
        unlink_at,
        kill_next_at,
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn any_walk_matches_original_c(
        flags in proptest::collection::vec(0..0x20_u8, 0..=POOL),
        walk in 0..4_u8,
        actions in actions(),
    ) {
        prop_assert_eq!(probe(&flags, walk, actions), 0);
    }
}
