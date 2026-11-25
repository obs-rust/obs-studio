//! Tier 3: the unchanged extracted `obs_source_update()`
//! (`obs-source-update.c.h`) against the Rust ABI, both over the real
//! private source layout through the production field adapter, with a
//! recording update callback, real source and core signal handlers and an
//! optional nested update from inside the callback. Both sides merge
//! through the same `obs_data_apply()`, so this compares what reaches it
//! and everything after: the stored and incoming settings after, the event
//! order, the callback's count, arguments and the settings it saw, and the
//! defer count. No intentional differences; no input exclusions.
use core::ffi::{CStr, c_char, c_long, c_void};
use core::ptr;
use obs_source::ffi::source_update::obs_source_update as update;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

type Update = unsafe extern "C" fn(*mut c_void, *mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);
unsafe extern "C" {
    fn obs_data_create() -> *mut c_void;
    fn obs_data_release(data: *mut c_void);
    fn obs_data_get_json(data: *mut c_void) -> *const c_char;
    fn obs_data_set_int(data: *mut c_void, name: *const c_char, value: i64);
    fn obs_data_set_string(data: *mut c_void, name: *const c_char, value: *const c_char);
    fn obs_data_set_bool(data: *mut c_void, name: *const c_char, value: bool);
    fn oracle_obs_source_update(source: *mut c_void, settings: *mut c_void);
    fn oracle_source_update_new(
        entry: Update,
        stored: *mut c_void,
        nested: *mut c_void,
        flags: u32,
        bits: u32,
        defer: c_long,
    ) -> *mut c_void;
    fn oracle_source_update_run(fixture: *mut c_void, null_source: bool, incoming: *mut c_void);
    fn oracle_source_update_record(
        fixture: *const c_void,
        updates: *mut u32,
        identity: *mut bool,
        seen: *mut *const c_char,
        defer: *mut c_long,
        stored: *mut c_void,
        bits: u32,
    ) -> *const c_char;
    fn oracle_source_update_free(fixture: *mut c_void);
    fn oracle_source_update_logs(
        update: Update,
        get: GetLog,
        set: SetLog,
        incoming: *mut c_void,
    ) -> bool;
}

const KEYS: [&CStr; 3] = [c"a", c"b", c"c"];

#[derive(Clone, Debug)]
enum Value {
    Int(i64),
    Str(&'static CStr),
    Bool(bool),
}

/// A settings object, or NULL.
type Spec = Option<Vec<(usize, Value)>>;

fn build(spec: &Spec) -> *mut c_void {
    let Some(items) = spec else {
        return ptr::null_mut();
    };
    // SAFETY: each object is created here and every key is a static C string.
    unsafe {
        let data = obs_data_create();
        for (key, value) in items {
            let key = KEYS[key % KEYS.len()].as_ptr();
            match value {
                Value::Int(v) => obs_data_set_int(data, key, *v),
                Value::Str(v) => obs_data_set_string(data, key, v.as_ptr()),
                Value::Bool(v) => obs_data_set_bool(data, key, *v),
            }
        }
        data
    }
}

fn json(data: *mut c_void) -> Option<String> {
    if data.is_null() {
        return None;
    }
    // SAFETY: a live data object; its JSON lives until the next change.
    let text = unsafe { obs_data_get_json(data) };
    // SAFETY: obs_data_get_json() gives NULL or a NUL-terminated string.
    (!text.is_null()).then(|| {
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
    })
}

#[derive(Clone, Debug)]
struct Case {
    stored: Spec,
    incoming: Spec,
    nested: Spec,
    null_source: bool,
    flags: u32,
    bits: u32,
    defer: c_long,
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    events: String,
    updates: u32,
    identity: bool,
    seen: Option<String>,
    defer: c_long,
    stored: Option<String>,
    incoming: Option<String>,
}

fn run(entry: Update, case: &Case) -> Outcome {
    let stored = build(&case.stored);
    let incoming = build(&case.incoming);
    let nested = build(&case.nested);
    // SAFETY: the fixture borrows the three objects, which outlive it; each
    // object created here is released once.
    unsafe {
        let fixture =
            oracle_source_update_new(entry, stored, nested, case.flags, case.bits, case.defer);
        assert!(!fixture.is_null());
        oracle_source_update_run(fixture, case.null_source, incoming);
        let (mut updates, mut identity, mut seen, mut defer) = (0, false, ptr::null(), 0);
        let events = oracle_source_update_record(
            fixture,
            &mut updates,
            &mut identity,
            &mut seen,
            &mut defer,
            stored,
            case.bits,
        );
        let outcome = Outcome {
            events: CStr::from_ptr(events).to_string_lossy().into_owned(),
            updates,
            identity,
            seen: (!seen.is_null()).then(|| CStr::from_ptr(seen).to_string_lossy().into_owned()),
            defer,
            stored: json(stored),
            incoming: json(incoming),
        };
        oracle_source_update_free(fixture);
        for data in [stored, incoming, nested] {
            if !data.is_null() {
                obs_data_release(data);
            }
        }
        outcome
    }
}

fn check(case: &Case) -> Outcome {
    let rust = run(update, case);
    let original = run(oracle_obs_source_update, case);
    assert_eq!(rust, original, "{case:?}");
    assert!(rust.identity, "{case:?}");
    rust
}

fn specs() -> Vec<Spec> {
    vec![
        None,
        Some(vec![]),
        Some(vec![(0, Value::Int(1)), (1, Value::Str(c"stored"))]),
        Some(vec![(1, Value::Int(2)), (2, Value::Bool(true))]),
    ]
}

#[test]
fn characterized_edges() {
    if !crate::output::isolated(concat!(module_path!(), "::characterized_edges")) {
        return;
    }
    for stored in specs() {
        for incoming in specs() {
            for flags in [0, 1, 2, u32::MAX, u32::MAX - 1] {
                for bits in 0..16 {
                    for defer in [-1, 0, 5] {
                        let case = Case {
                            stored: stored.clone(),
                            incoming: incoming.clone(),
                            nested: Some(vec![(2, Value::Int(30))]),
                            null_source: false,
                            flags,
                            bits,
                            defer,
                        };
                        let outcome = check(&case);
                        let video = flags & 1 != 0;
                        let dispatch = !video && bits & 3 == 3;
                        assert_eq!(outcome.defer, if video { defer + 1 } else { defer });
                        assert_eq!(
                            outcome.updates,
                            if dispatch { 1 + ((bits >> 3) & 1) } else { 0 }
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn pinned_orders_match_test_source_update() {
    if !crate::output::isolated(concat!(
        module_path!(),
        "::pinned_orders_match_test_source_update"
    )) {
        return;
    }
    let case = |bits| Case {
        stored: Some(vec![(0, Value::Int(1))]),
        incoming: Some(vec![(1, Value::Int(2))]),
        nested: Some(vec![(2, Value::Int(30))]),
        null_source: false,
        flags: 0,
        bits,
        defer: 0,
    };
    assert_eq!(check(&case(0b0011)).events, "UGL");
    assert_eq!(check(&case(0b0111)).events, "UL");
    assert_eq!(check(&case(0b1011)).events, "UUGL)GL");
    assert_eq!(check(&case(0b0001)).events, "");
    assert_eq!(check(&case(0b0010)).events, "");
    let video = Case {
        flags: 1,
        ..case(0b1011)
    };
    let outcome = check(&video);
    assert_eq!((outcome.events.as_str(), outcome.defer), ("", 1));
}

#[test]
fn null_source_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::null_source_matches_c")) {
        return;
    }
    for incoming in specs() {
        let outcome = check(&Case {
            stored: Some(vec![(0, Value::Int(1))]),
            incoming,
            nested: None,
            null_source: true,
            flags: 0,
            bits: 3,
            defer: 0,
        });
        assert_eq!((outcome.events.as_str(), outcome.updates), ("", 0));
        let data = build(&Some(vec![(1, Value::Int(2))]));
        let before = json(data);
        // SAFETY: this isolated process owns the log handler; the probe
        // restores it. The incoming object is released once, after.
        unsafe {
            assert!(oracle_source_update_logs(
                update,
                base_get_log_handler,
                base_set_log_handler,
                data
            ));
            assert_eq!(json(data), before);
            obs_data_release(data);
        }
    }
}

fn value() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<i64>().prop_map(Value::Int),
        prop_oneof![Just(c""), Just(c"stored"), Just(c"x y")].prop_map(Value::Str),
        any::<bool>().prop_map(Value::Bool),
    ]
}

fn spec() -> impl Strategy<Value = Spec> {
    prop::option::weighted(0.8, prop::collection::vec((0_usize..3, value()), 0..4))
}

#[test]
fn random_updates_match_c() {
    if !crate::output::isolated(concat!(module_path!(), "::random_updates_match_c")) {
        return;
    }
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(
                (spec(), spec(), spec()),
                prop::bool::weighted(0.1),
                any::<u32>(),
                0_u32..16,
                -3 as c_long..4,
            ),
            |((stored, incoming, nested), null_source, flags, bits, defer)| {
                check(&Case {
                    stored,
                    incoming,
                    nested,
                    null_source,
                    flags,
                    bits,
                    defer,
                });
                Ok(())
            },
        )
        .unwrap();
}
