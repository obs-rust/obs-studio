//! Tier 3: the original extracted `obs_enum_outputs()`, `obs_enum_encoders()`
//! and `obs_enum_services()` versus the shims on one recording fixture core
//! (oracle/enum_contexts.c), through the real-named lock, head, next and
//! get_ref/release adapters. The fixture holds the same list for each kind,
//! each with its own re-entrant mutex, so a walk of the wrong kind shows.
//! Compared: the trace of every lock and unlock with its kind, every get_ref
//! with its answer, every callback and every walk release and destroy (with
//! its checks that the head, each next, get_ref and the callback run under
//! the kind's lock, that the callback gets the caller's param and never a
//! destroyed context), and the final contexts, links, heads and counts, and
//! every mutex released. Cases: empty lists, dying entries skipped by
//! get_ref, early stops, the callback releasing the current entry, nested
//! enumeration of the same or another kind and a context created inside the
//! callback.
//!
//! Excluded: an uninitialized core, a NULL callback, a dead listed context
//! and a list change from another thread, outside the original's contract.
//! No intentional differences. Mutants show the probe catches each broken
//! step.
use core::ffi::{CStr, c_char, c_void};
use obs_core::ffi::enum_contexts::{
    EnumProc, obs_enum_encoders, obs_enum_outputs, obs_enum_services,
};
use proptest::prelude::*;

const LIST: usize = 8;
const NEVER: u8 = 0xff;

type Enum = unsafe extern "C" fn(EnumProc, *mut c_void);
type Proc = unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool;

const SHIMS: [Enum; 3] = [obs_enum_outputs, obs_enum_encoders, obs_enum_services];

#[repr(C)]
struct Case {
    refs: *const u8,
    count: usize,
    kind: u8,
    stop_after: u8,
    release_at: u8,
    nested_at: u8,
    nested_kind: u8,
    create_at: u8,
}

unsafe extern "C" {
    fn oracle_enum_contexts_probe(candidates: *const Enum, case: *const Case) -> u32;
    fn oracle_enum_contexts_original(case: *const Case, trace: *mut c_char, size: usize) -> bool;
    fn oracle_enum_contexts_adapters() -> bool;
}

/// What the callback does, by visit index of the outer walk.
#[derive(Debug, Clone, Copy)]
struct Actions {
    stop_after: u8,
    release_at: u8,
    nested_at: u8,
    nested_kind: u8,
    create_at: u8,
}

const NONE: Actions = Actions {
    stop_after: NEVER,
    release_at: NEVER,
    nested_at: NEVER,
    nested_kind: 0,
    create_at: NEVER,
};

fn case(refs: &[u8], kind: u8, actions: Actions) -> Case {
    Case {
        refs: refs.as_ptr(),
        count: refs.len(),
        kind,
        stop_after: actions.stop_after,
        release_at: actions.release_at,
        nested_at: actions.nested_at,
        nested_kind: actions.nested_kind,
        create_at: actions.create_at,
    }
}

fn probe_with(candidates: &[Enum; 3], refs: &[u8], kind: u8, actions: Actions) -> u32 {
    let case = case(refs, kind, actions);
    // SAFETY: the oracle owns the fixture core and contexts every adapter
    // reaches; the refs outlive the call.
    unsafe { oracle_enum_contexts_probe(candidates.as_ptr(), &case) }
}

fn probe(refs: &[u8], kind: u8, actions: Actions) -> u32 {
    probe_with(&SHIMS, refs, kind, actions)
}

/// The original's trace.
fn original(refs: &[u8], kind: u8, actions: Actions) -> String {
    let case = case(refs, kind, actions);
    let mut trace = [0 as c_char; 1024];
    // SAFETY: the buffer is writable for its size; the oracle writes
    // NUL-terminated text.
    let released = unsafe { oracle_enum_contexts_original(&case, trace.as_mut_ptr(), trace.len()) };
    assert!(released, "the original left a mutex held");
    // SAFETY: NUL-terminated by the oracle.
    unsafe { CStr::from_ptr(trace.as_ptr()) }
        .to_str()
        .unwrap()
        .to_owned()
}

#[test]
fn real_adapters_read_the_real_fields() {
    // SAFETY: the oracle arms and frees its own fixture.
    assert!(unsafe { oracle_enum_contexts_adapters() });
}

#[test]
fn original_c_is_the_characterized_walk() {
    // An empty list: one hold, no call.
    assert_eq!(original(&[], 1, NONE), "L1U1");
    // get_ref for each listed entry in order, a dying one skipped; each
    // callback under its own hold; the snapshot released in order after.
    assert_eq!(
        original(&[1, 0, 2], 0, NONE),
        "L0g0a+g0b-g0c+U0L0c0aU0L0c0cU0r0ar0c"
    );
    // An early stop still releases the whole snapshot.
    assert_eq!(
        original(
            &[1, 1],
            2,
            Actions {
                stop_after: 0,
                ..NONE
            }
        ),
        "L2g2a+g2b+U2L2c2aU2r2ar2b"
    );
    // The callback's release leaves the entry to the walk's, which destroys
    // it after the last unlock.
    assert_eq!(
        original(
            &[1],
            1,
            Actions {
                release_at: 0,
                ..NONE
            }
        ),
        "L1g1a+U1L1c1aO1aU1r1aD1aL1U1"
    );
    // A nested walk runs inside the outer callback's hold; a context created
    // in the callback goes to the head and is not visited.
    assert_eq!(
        original(
            &[1],
            0,
            Actions {
                nested_at: 0,
                nested_kind: 0,
                create_at: 0,
                ..NONE
            }
        ),
        "L0g0a+U0L0c0aNL0g0a+U0L0c0aU0r0anC0bL0U0U0r0a"
    );
}

/// Each kind's shim against the original over the named edges.
#[test]
fn characterized_edges() {
    let lists: [&[u8]; 7] = [
        &[],
        &[1],
        &[0],
        &[1, 1, 1],
        &[0, 1, 0],
        &[2, 0, 1, 1],
        &[1, 0, 1, 2, 0, 1, 1, 1],
    ];
    let mut actions = vec![NONE];
    for at in [0, 1, 2, 7] {
        actions.push(Actions {
            stop_after: at,
            ..NONE
        });
        actions.push(Actions {
            release_at: at,
            ..NONE
        });
        actions.push(Actions {
            create_at: at,
            ..NONE
        });
        for nested_kind in 0..3 {
            actions.push(Actions {
                nested_at: at,
                nested_kind,
                ..NONE
            });
        }
    }
    actions.push(Actions {
        stop_after: 1,
        release_at: 1,
        nested_at: 1,
        nested_kind: 0,
        create_at: 0,
    });
    for kind in 0..3_u8 {
        for refs in lists {
            for &a in &actions {
                let mut a = a;
                if a.nested_at != NEVER && a.nested_kind == 0 {
                    a.nested_kind = kind;
                }
                assert_eq!(probe(refs, kind, a), 0, "{kind} {refs:?} {a:?}");
            }
        }
    }
}

// Mutants: a walk that breaks one step must be caught.

struct Mutant;

impl Mutant {
    /// The shims with one kind's export replaced.
    fn shims(kind: usize, mutant: Enum) -> [Enum; 3] {
        let mut shims = SHIMS;
        shims[kind] = mutant;
        shims
    }
}

/// Calls the callback on the live list without a snapshot or a reference.
unsafe extern "C" fn no_snapshot(enum_proc: EnumProc, param: *mut c_void) {
    unsafe extern "C" {
        fn obs_enum_contexts_lock(kind: u32);
        fn obs_enum_contexts_unlock(kind: u32);
        fn obs_enum_contexts_first(kind: u32) -> *mut c_void;
        fn obs_enum_contexts_next(context: *const c_void) -> *mut c_void;
    }
    let enum_proc: Proc = enum_proc.unwrap();
    // SAFETY: the fixture's adapters, on this thread's fixture.
    unsafe {
        obs_enum_contexts_lock(0);
        let mut context = obs_enum_contexts_first(0);
        while !context.is_null() {
            let next = obs_enum_contexts_next(context);
            if !enum_proc(param, context) {
                break;
            }
            context = next;
        }
        obs_enum_contexts_unlock(0);
    }
}

/// The encoders' walk on the outputs' list.
unsafe extern "C" fn wrong_kind(enum_proc: EnumProc, param: *mut c_void) {
    // SAFETY: as the shim.
    unsafe { obs_enum_outputs(enum_proc, param) }
}

#[test]
fn mutants_are_caught() {
    let releases = Actions {
        release_at: 0,
        ..NONE
    };
    assert_ne!(
        probe_with(&Mutant::shims(0, no_snapshot), &[1, 1], 0, releases),
        0
    );
    assert_ne!(
        probe_with(&Mutant::shims(0, no_snapshot), &[1, 0], 0, NONE),
        0
    );
    assert_ne!(
        probe_with(&Mutant::shims(1, wrong_kind), &[1, 1], 1, NONE),
        0
    );
    // The shims themselves match.
    assert_eq!(probe(&[1, 1], 0, releases), 0);
}

fn actions() -> impl Strategy<Value = Actions> {
    let at = || prop_oneof![1 => Just(NEVER), 3 => 0..LIST as u8];
    (at(), at(), at(), 0..3_u8, at()).prop_map(
        |(stop_after, release_at, nested_at, nested_kind, create_at)| Actions {
            stop_after,
            release_at,
            nested_at,
            nested_kind,
            create_at,
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn any_walk_matches_original_c(
        refs in proptest::collection::vec(prop_oneof![1 => Just(0_u8), 3 => 1..3_u8], 0..=LIST),
        kind in 0..3_u8,
        actions in actions(),
    ) {
        prop_assert_eq!(probe(&refs, kind, actions), 0);
    }
}
