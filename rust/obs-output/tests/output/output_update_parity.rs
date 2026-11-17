//! Tier 3: the unchanged extracted `obs_output_update()`
//! (`obs-output-update.c.h`) against the Rust ABI, both over the real
//! private output layout through the production field adapter, with a
//! recording update callback. Both sides merge through the same
//! `obs_data_apply()`, so this compares what reaches it and the callback:
//! the stored settings before and after, the incoming settings after, and
//! the callback's count, data, settings object and the settings it saw.
//! No intentional differences; no input exclusions.
use core::ffi::{CStr, c_char, c_void};
use core::ptr;
use obs_output::ffi::output_update::obs_output_update as update;
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
    fn obs_data_set_obj(data: *mut c_void, name: *const c_char, obj: *mut c_void);
    fn oracle_output_update_new(settings: *mut c_void, has_update: bool, flags: u32)
    -> *mut c_void;
    fn oracle_output_update_output(fixture: *mut c_void) -> *mut c_void;
    fn oracle_output_update_calls(
        fixture: *const c_void,
        identity: *mut bool,
        seen: *mut *const c_char,
    ) -> u32;
    fn oracle_output_update_unchanged(
        fixture: *const c_void,
        settings: *mut c_void,
        has_update: bool,
    ) -> bool;
    fn oracle_output_update_free(fixture: *mut c_void);
    fn oracle_obs_output_update(output: *mut c_void, settings: *mut c_void);
    fn oracle_output_update_logs(
        update: Update,
        get: GetLog,
        set: SetLog,
        incoming: *mut c_void,
    ) -> bool;
}

const KEYS: [&CStr; 4] = [c"a", c"b", c"overwrite", c"nested"];

#[derive(Clone, Debug)]
enum Value {
    Int(i64),
    Str(&'static CStr),
    Bool(bool),
    Obj(Vec<(usize, i64)>),
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
                Value::Obj(inner) => {
                    let obj = obs_data_create();
                    for (k, v) in inner {
                        obs_data_set_int(obj, KEYS[k % KEYS.len()].as_ptr(), *v);
                    }
                    obs_data_set_obj(data, key, obj);
                    obs_data_release(obj);
                }
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

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    stored: Option<String>,
    incoming: Option<String>,
    calls: u32,
    identity: bool,
    seen: Option<String>,
}

/// One side's run: fresh objects from the specs, then the update.
fn run(
    f: unsafe extern "C" fn(*mut c_void, *mut c_void),
    stored: &Spec,
    incoming: &Spec,
    same: bool,
    has_update: bool,
    flags: u32,
) -> Outcome {
    let stored_data = build(stored);
    let incoming_data = if same { stored_data } else { build(incoming) };
    // SAFETY: the fixture borrows the stored object, which outlives it; every
    // object created here is released once.
    unsafe {
        let fixture = oracle_output_update_new(stored_data, has_update, flags);
        assert!(!fixture.is_null());
        f(oracle_output_update_output(fixture), incoming_data);
        assert!(oracle_output_update_unchanged(
            fixture,
            stored_data,
            has_update
        ));
        let mut identity = false;
        let mut seen = ptr::null();
        let calls = oracle_output_update_calls(fixture, &mut identity, &mut seen);
        let seen = (!seen.is_null()).then(|| CStr::from_ptr(seen).to_string_lossy().into_owned());
        let outcome = Outcome {
            stored: json(stored_data),
            incoming: json(incoming_data),
            calls,
            identity,
            seen,
        };
        oracle_output_update_free(fixture);
        if !same && !incoming_data.is_null() {
            obs_data_release(incoming_data);
        }
        if !stored_data.is_null() {
            obs_data_release(stored_data);
        }
        outcome
    }
}

fn check(stored: &Spec, incoming: &Spec, same: bool, has_update: bool, flags: u32) {
    let r = run(update, stored, incoming, same, has_update, flags);
    let c = run(
        oracle_obs_output_update,
        stored,
        incoming,
        same,
        has_update,
        flags,
    );
    assert_eq!(r, c);
    assert_eq!(r.calls, u32::from(has_update));
    assert_eq!(r.identity, has_update);
}

fn specs() -> Vec<Spec> {
    vec![
        None,
        Some(vec![]),
        Some(vec![(2, Value::Int(10)), (0, Value::Str(c"original"))]),
        Some(vec![(2, Value::Int(20)), (1, Value::Bool(true))]),
        Some(vec![
            (3, Value::Obj(vec![(0, 1), (1, 2)])),
            (2, Value::Int(-1)),
        ]),
        Some(vec![(3, Value::Int(5))]),
    ]
}

#[test]
fn characterized_edges() {
    for stored in specs() {
        for incoming in specs() {
            for has_update in [false, true] {
                check(&stored, &incoming, false, has_update, 0);
                check(&stored, &incoming, false, has_update, u32::MAX);
            }
        }
        for has_update in [false, true] {
            check(&stored, &stored, true, has_update, 0);
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    if !super::isolated(concat!(module_path!(), "::null_diagnostics_match_c")) {
        return;
    }
    for incoming in specs() {
        let data = build(&incoming);
        let before = json(data);
        // SAFETY: this isolated process owns the log handler; the probe
        // restores it. The incoming object is released once, after.
        unsafe {
            assert!(oracle_output_update_logs(
                update,
                base_get_log_handler,
                base_set_log_handler,
                data
            ));
            assert_eq!(json(data), before);
            if !data.is_null() {
                obs_data_release(data);
            }
        }
    }
}

fn value() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<i64>().prop_map(Value::Int),
        prop_oneof![Just(c""), Just(c"original"), Just(c"x y")].prop_map(Value::Str),
        any::<bool>().prop_map(Value::Bool),
        prop::collection::vec((0_usize..4, any::<i64>()), 0..3).prop_map(Value::Obj),
    ]
}

fn spec() -> impl Strategy<Value = Spec> {
    prop::option::weighted(0.9, prop::collection::vec((0_usize..4, value()), 0..5))
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None, .. ProptestConfig::default()
    })]
    #[test]
    fn random_updates_match_c(
        stored in spec(),
        incoming in spec(),
        same in prop::bool::weighted(0.1),
        has_update in any::<bool>(),
        flags in any::<u32>(),
    ) {
        check(&stored, &incoming, same, has_update, flags);
    }
}
