//! Tier 3 parity: the shims against the original
//! `libobs/obs-hotkey-save-load.c.h` (oracle/hotkey_save_load.c).
//!
//! The C tests see the Rust swap through a real libobs. This file compares the
//! two implementations directly, on every OS.
//!
//! Both sides run on the same fake registry. This file defines the private seam
//! (`obs-hotkey-seam.h`) and the public getters the C and the shims call, and
//! logs every call, per thread: the lock, each lookup, each binding added or
//! removed, and each signal. Both sides read and write obs_data through the Rust
//! obs-data port, and use the Rust binding codec; each has its own parity test.
//!
//! A case is a random sequence of the 13 public functions, over hotkeys, a
//! missing hotkey, pairs (one with a missing half), a missing pair and the four
//! kinds of object, from random JSON, with the lock failing or not and the
//! host's volume keys swapped or not. Compared: the log, the bindings left, and
//! each result's JSON, byte for byte.
//!
//! Intentional differences: none that this file can reach. See the shim docs.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_void};

use obs_c_oracle::hotkey_save_load as c;
use obs_data::ffi::obs_data::{
    obs_data_array_release, obs_data_create, obs_data_create_from_json, obs_data_get_array,
    obs_data_get_json, obs_data_release, obs_data_set_array,
};
use obs_hotkey::ffi::binding_press::obs_hotkey_binding;
use obs_hotkey::ffi::nix_hotkey_state::obs_key_combination;
use obs_hotkey::ffi::pair::{obs_hotkey_pair, obs_hotkey_t};
use obs_hotkey::ffi::save_load as rs;
use obs_hotkey::name_map::{OBS_KEY_VK_VOLUME_DOWN, OBS_KEY_VK_VOLUME_UP};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// The fake registry.

struct FakeHotkey {
    id: usize,
    name: CString,
}

struct FakePair {
    ids: [usize; 2],
}

#[derive(Clone, Copy)]
struct FakeBinding {
    combo: obs_key_combination,
    hotkey_id: usize,
}

/// A fake object: the ids in its context. The object is its own context.
struct FakeContext {
    ids: Vec<usize>,
}

/// Boxes: the C and the shims hold pointers to these while the vectors grow.
#[derive(Default)]
#[allow(clippy::vec_box)]
struct Registry {
    lock_ok: bool,
    swapped: bool,
    hotkeys: Vec<Box<FakeHotkey>>,
    pairs: Vec<(usize, Box<FakePair>)>,
    bindings: Vec<Box<FakeBinding>>,
    objects: Vec<Box<FakeContext>>,
    log: Vec<String>,
}

thread_local! {
    static REGISTRY: RefCell<Registry> = RefCell::default();
}

fn log(entry: String) {
    REGISTRY.with_borrow_mut(|r| r.log.push(entry));
}

const OBJECTS: usize = 4;
const MISSING_ID: usize = 99;

fn reset(lock_ok: bool, swapped: bool) {
    REGISTRY.with_borrow_mut(|r| {
        *r = Registry {
            lock_ok,
            swapped,
            ..Registry::default()
        };
        for id in 1..=4 {
            r.hotkeys.push(Box::new(FakeHotkey {
                id,
                name: CString::new(format!("key.{id}")).expect("no NUL"),
            }));
        }
        r.pairs.push((1, Box::new(FakePair { ids: [1, 2] })));
        r.pairs.push((
            2,
            Box::new(FakePair {
                ids: [3, MISSING_ID],
            }),
        ));
        r.objects.push(Box::new(FakeContext { ids: vec![1, 2] }));
        // A missing id is skipped.
        r.objects.push(Box::new(FakeContext {
            ids: vec![3, MISSING_ID, 4],
        }));
        r.objects.push(Box::new(FakeContext { ids: vec![4] }));
        // No hotkeys: its save is NULL.
        r.objects.push(Box::new(FakeContext { ids: vec![] }));
    });
}

fn object(which: usize) -> *mut c_void {
    REGISTRY.with_borrow_mut(|r| (&raw mut *r.objects[which]).cast())
}

// ---------------------------------------------------------------------------
// The seam and the getters, by their C names.

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_seam_lock() -> bool {
    log("lock".into());
    REGISTRY.with_borrow(|r| r.lock_ok)
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_seam_unlock() {
    log("unlock".into());
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_seam_find(id: usize) -> *mut obs_hotkey_t {
    log(format!("find {id}"));
    REGISTRY.with_borrow_mut(|r| {
        r.hotkeys
            .iter_mut()
            .find(|h| h.id == id)
            .map_or(std::ptr::null_mut(), |h| (&raw mut **h).cast())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_seam_find_pair(id: usize) -> *mut obs_hotkey_pair {
    log(format!("find_pair {id}"));
    REGISTRY.with_borrow_mut(|r| {
        r.pairs
            .iter_mut()
            .find(|(pair_id, _)| *pair_id == id)
            .map_or(std::ptr::null_mut(), |(_, p)| (&raw mut **p).cast())
    })
}

/// # Safety
///
/// `pair` is a [`FakePair`] of this thread's registry.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_seam_pair_id(
    pair: *const obs_hotkey_pair,
    half: usize,
) -> usize {
    // SAFETY: per the caller.
    unsafe { (*pair.cast::<FakePair>()).ids[half] }
}

/// # Safety
///
/// `hotkey` is a [`FakeHotkey`] of this thread's registry.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_seam_add_binding(
    hotkey: *mut obs_hotkey_t,
    combo: obs_key_combination,
) {
    // SAFETY: per the caller.
    let hotkey_id = unsafe { (*hotkey.cast::<FakeHotkey>()).id };
    log(format!("add {hotkey_id} {}/{}", combo.modifiers, combo.key));
    REGISTRY.with_borrow_mut(|r| r.bindings.push(Box::new(FakeBinding { combo, hotkey_id })));
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_seam_remove_bindings(id: usize) -> bool {
    let removed = REGISTRY.with_borrow_mut(|r| {
        let before = r.bindings.len();
        r.bindings.retain(|b| b.hotkey_id != id);
        r.bindings.len() != before
    });
    log(format!("remove {id} {removed}"));
    removed
}

/// # Safety
///
/// `signal` is a NUL-terminated string; `hotkey` is null or a [`FakeHotkey`] of
/// this thread's registry.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_seam_signal(signal: *const c_char, hotkey: *mut obs_hotkey_t) {
    // SAFETY: per the caller.
    let signal = unsafe { CStr::from_ptr(signal) }.to_string_lossy();
    let id = if hotkey.is_null() {
        0
    } else {
        // SAFETY: per the caller.
        unsafe { (*hotkey.cast::<FakeHotkey>()).id }
    };
    log(format!("signal {signal} {id}"));
}

/// # Safety
///
/// `func` takes `data`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_seam_enum_bindings(
    func: unsafe extern "C" fn(*mut c_void, usize, *mut obs_hotkey_binding) -> bool,
    data: *mut c_void,
) {
    log("enum_bindings".into());
    // The bindings do not change during a save, so the boxes stay live.
    let bindings: Vec<*mut FakeBinding> =
        REGISTRY.with_borrow_mut(|r| r.bindings.iter_mut().map(|b| &raw mut **b).collect());
    for (i, binding) in bindings.into_iter().enumerate() {
        // SAFETY: per the caller; a live box.
        if !unsafe { func(data, i, binding.cast()) } {
            break;
        }
    }
}

/// # Safety
///
/// `context` is a [`FakeContext`] of this thread's registry; `func` takes `data`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_seam_enum_context(
    context: *mut c_void,
    func: unsafe extern "C" fn(*mut c_void, *mut obs_hotkey_t) -> bool,
    data: *mut c_void,
) {
    log("enum_context".into());
    // SAFETY: per the caller.
    let ids = unsafe { (*context.cast::<FakeContext>()).ids.clone() };
    for id in ids {
        let hotkey: *mut obs_hotkey_t = REGISTRY.with_borrow_mut(|r| {
            r.hotkeys
                .iter_mut()
                .find(|h| h.id == id)
                .map_or(std::ptr::null_mut(), |h| (&raw mut **h).cast())
        });
        if hotkey.is_null() {
            continue;
        }
        // SAFETY: per the caller; a live box.
        if !unsafe { func(data, hotkey) } {
            break;
        }
    }
}

/// # Safety
///
/// `context` is a [`FakeContext`] of this thread's registry.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_seam_context_count(context: *const c_void) -> usize {
    // SAFETY: per the caller.
    unsafe { (*context.cast::<FakeContext>()).ids.len() }
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_seam_saved_volume_keys_swapped() -> bool {
    log("swapped".into());
    REGISTRY.with_borrow(|r| r.swapped)
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_seam_encoder_context(encoder: *mut c_void) -> *mut c_void {
    log("encoder_context".into());
    encoder
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_seam_output_context(output: *mut c_void) -> *mut c_void {
    log("output_context".into());
    output
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_seam_service_context(service: *mut c_void) -> *mut c_void {
    log("service_context".into());
    service
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_seam_source_context(source: *mut c_void) -> *mut c_void {
    log("source_context".into());
    source
}

/// # Safety
///
/// `key` is a [`FakeHotkey`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_get_id(key: *const obs_hotkey_t) -> usize {
    // SAFETY: per the caller.
    unsafe { (*key.cast::<FakeHotkey>()).id }
}

/// # Safety
///
/// `key` is a [`FakeHotkey`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_get_name(key: *const obs_hotkey_t) -> *const c_char {
    // SAFETY: per the caller.
    unsafe { (*key.cast::<FakeHotkey>()).name.as_ptr() }
}

/// # Safety
///
/// `binding` is a [`FakeBinding`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_binding_get_key_combination(
    binding: *mut obs_hotkey_binding,
) -> obs_key_combination {
    // SAFETY: per the caller.
    unsafe { (*binding.cast::<FakeBinding>()).combo }
}

/// # Safety
///
/// `binding` is a [`FakeBinding`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_binding_get_hotkey_id(
    binding: *mut obs_hotkey_binding,
) -> usize {
    // SAFETY: per the caller.
    unsafe { (*binding.cast::<FakeBinding>()).hotkey_id }
}

// ---------------------------------------------------------------------------
// JSON helpers.

/// `"[...]"` to a new `obs_data_array_t`, or null for `None`.
fn array_from(items: Option<&str>) -> *mut c_void {
    let Some(items) = items else {
        return std::ptr::null_mut();
    };
    let json = CString::new(format!("{{\"b\":{items}}}")).expect("no NUL");
    // SAFETY: a NUL-terminated string; a static name; the data is released and
    // the array keeps its own reference.
    unsafe {
        let data = obs_data_create_from_json(json.as_ptr());
        assert!(!data.is_null());
        let array = obs_data_get_array(data, c"b".as_ptr());
        obs_data_release(data);
        array
    }
}

fn data_json(data: *mut c_void) -> String {
    // SAFETY: a live obs_data; the JSON is copied before the release.
    unsafe {
        let json = CStr::from_ptr(obs_data_get_json(data))
            .to_string_lossy()
            .into_owned();
        obs_data_release(data);
        json
    }
}

/// The JSON of a result array, and its release. Null is `"null"`.
fn array_json(array: *mut c_void) -> String {
    if array.is_null() {
        return "null".into();
    }
    // SAFETY: a new obs_data takes its own reference; a live array and name.
    unsafe {
        let data = obs_data_create();
        obs_data_set_array(data, c"b".as_ptr(), array);
        obs_data_array_release(array);
        data_json(data)
    }
}

fn object_json(data: *mut c_void) -> String {
    if data.is_null() {
        "null".into()
    } else {
        data_json(data)
    }
}

// ---------------------------------------------------------------------------
// The two sides.

#[derive(Clone, Copy, Debug)]
enum Side {
    C,
    Rust,
}

/// The four kinds of object, in the order of the shims.
#[derive(Clone, Copy, Debug)]
enum Kind {
    Encoder,
    Output,
    Service,
    Source,
}

impl Side {
    fn load_bindings(self, id: usize, combos: &[obs_key_combination]) {
        let converted: Vec<c::OracleKeyCombination> = combos
            .iter()
            .map(|combo| c::OracleKeyCombination {
                modifiers: combo.modifiers,
                key: combo.key,
            })
            .collect();
        // SAFETY: the slices are valid for their lengths.
        unsafe {
            match self {
                Side::C => {
                    c::oracle_obs_hotkey_load_bindings(id, converted.as_ptr(), converted.len());
                }
                Side::Rust => rs::obs_hotkey_load_bindings(id, combos.as_ptr(), combos.len()),
            }
        }
    }

    fn load(self, id: usize, data: *mut c_void) {
        // SAFETY: a live array or null.
        unsafe {
            match self {
                Side::C => c::oracle_obs_hotkey_load(id, data),
                Side::Rust => rs::obs_hotkey_load(id, data),
            }
        }
    }

    fn pair_load(self, id: usize, data0: *mut c_void, data1: *mut c_void) {
        // SAFETY: live arrays or null.
        unsafe {
            match self {
                Side::C => c::oracle_obs_hotkey_pair_load(id, data0, data1),
                Side::Rust => rs::obs_hotkey_pair_load(id, data0, data1),
            }
        }
    }

    fn save(self, id: usize) -> String {
        // SAFETY: no precondition.
        array_json(match self {
            Side::C => unsafe { c::oracle_obs_hotkey_save(id) },
            Side::Rust => rs::obs_hotkey_save(id),
        })
    }

    fn pair_save(self, id: usize, want0: bool, want1: bool) -> (String, String) {
        let (mut d0, mut d1) = (std::ptr::null_mut(), std::ptr::null_mut());
        let p0 = if want0 {
            &raw mut d0
        } else {
            std::ptr::null_mut()
        };
        let p1 = if want1 {
            &raw mut d1
        } else {
            std::ptr::null_mut()
        };
        // SAFETY: writable or null.
        unsafe {
            match self {
                Side::C => c::oracle_obs_hotkey_pair_save(id, p0, p1),
                Side::Rust => rs::obs_hotkey_pair_save(id, p0, p1),
            }
        }
        (array_json(d0), array_json(d1))
    }

    fn load_object(self, kind: Kind, object: *mut c_void, data: *mut c_void) {
        // SAFETY: a fake object or null, and live data or null.
        unsafe {
            match (self, kind) {
                (Side::C, Kind::Encoder) => c::oracle_obs_hotkeys_load_encoder(object, data),
                (Side::C, Kind::Output) => c::oracle_obs_hotkeys_load_output(object, data),
                (Side::C, Kind::Service) => c::oracle_obs_hotkeys_load_service(object, data),
                (Side::C, Kind::Source) => c::oracle_obs_hotkeys_load_source(object, data),
                (Side::Rust, Kind::Encoder) => rs::obs_hotkeys_load_encoder(object, data),
                (Side::Rust, Kind::Output) => rs::obs_hotkeys_load_output(object, data),
                (Side::Rust, Kind::Service) => rs::obs_hotkeys_load_service(object, data),
                (Side::Rust, Kind::Source) => rs::obs_hotkeys_load_source(object, data),
            }
        }
    }

    fn save_object(self, kind: Kind, object: *mut c_void) -> String {
        // SAFETY: a fake object.
        object_json(unsafe {
            match (self, kind) {
                (Side::C, Kind::Encoder) => c::oracle_obs_hotkeys_save_encoder(object),
                (Side::C, Kind::Output) => c::oracle_obs_hotkeys_save_output(object),
                (Side::C, Kind::Service) => c::oracle_obs_hotkeys_save_service(object),
                (Side::C, Kind::Source) => c::oracle_obs_hotkeys_save_source(object),
                (Side::Rust, Kind::Encoder) => rs::obs_hotkeys_save_encoder(object),
                (Side::Rust, Kind::Output) => rs::obs_hotkeys_save_output(object),
                (Side::Rust, Kind::Service) => rs::obs_hotkeys_save_service(object),
                (Side::Rust, Kind::Source) => rs::obs_hotkeys_save_source(object),
            }
        })
    }
}

#[derive(Clone, Debug)]
enum Op {
    LoadBindings(usize, Vec<obs_key_combination>),
    Load(usize, Option<String>),
    PairLoad(usize, Option<String>, Option<String>),
    Save(usize),
    PairSave(usize, bool, bool),
    LoadObject(Kind, usize, Option<String>),
    LoadNullObject(Kind),
    SaveObject(Kind, usize),
}

fn free_array(array: *mut c_void) {
    if !array.is_null() {
        // SAFETY: a live reference of this file's.
        unsafe { obs_data_array_release(array) };
    }
}

/// What a sequence did on one side.
fn run(side: Side, ops: &[Op], lock_ok: bool, swapped: bool) -> (Vec<String>, Vec<String>) {
    reset(lock_ok, swapped);
    let mut results = Vec::new();
    for op in ops {
        match op {
            Op::LoadBindings(id, combos) => side.load_bindings(*id, combos),
            Op::Load(id, items) => {
                let array = array_from(items.as_deref());
                side.load(*id, array);
                free_array(array);
            }
            Op::PairLoad(id, items0, items1) => {
                let (a0, a1) = (array_from(items0.as_deref()), array_from(items1.as_deref()));
                side.pair_load(*id, a0, a1);
                free_array(a0);
                free_array(a1);
            }
            Op::Save(id) => results.push(side.save(*id)),
            Op::PairSave(id, want0, want1) => {
                let (d0, d1) = side.pair_save(*id, *want0, *want1);
                results.push(d0);
                results.push(d1);
            }
            Op::LoadObject(kind, which, json) => {
                let data = match json {
                    Some(json) => {
                        let json = CString::new(json.as_str()).expect("no NUL");
                        // SAFETY: a NUL-terminated string.
                        unsafe { obs_data_create_from_json(json.as_ptr()) }
                    }
                    None => std::ptr::null_mut(),
                };
                side.load_object(*kind, object(*which), data);
                if !data.is_null() {
                    // SAFETY: created above.
                    unsafe { obs_data_release(data) };
                }
            }
            Op::LoadNullObject(kind) => {
                // SAFETY: a NUL-terminated string.
                let data = unsafe { obs_data_create_from_json(c"{}".as_ptr()) };
                side.load_object(*kind, std::ptr::null_mut(), data);
                // SAFETY: created above.
                unsafe { obs_data_release(data) };
            }
            Op::SaveObject(kind, which) => {
                results.push(side.save_object(*kind, object(*which)));
            }
        }
    }
    REGISTRY.with_borrow_mut(|r| {
        for b in &r.bindings {
            results.push(format!(
                "binding {} {}/{}",
                b.hotkey_id, b.combo.modifiers, b.combo.key
            ));
        }
        (std::mem::take(&mut r.log), results)
    })
}

fn assert_parity(ops: &[Op], lock_ok: bool, swapped: bool) {
    let c_side = run(Side::C, ops, lock_ok, swapped);
    let rust_side = run(Side::Rust, ops, lock_ok, swapped);
    assert_eq!(c_side.0, rust_side.0, "log, {ops:?}");
    assert_eq!(c_side.1, rust_side.1, "results, {ops:?}");
}

// ---------------------------------------------------------------------------
// Inputs.

fn item() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just(r#"{"key":"OBS_KEY_A"}"#),
        Just(r#"{"shift":true,"control":true,"key":"OBS_KEY_B"}"#),
        // Rejected: a primary mouse button, an unknown name, no key.
        Just(r#"{"key":"OBS_KEY_MOUSE1"}"#),
        Just(r#"{"key":"nope"}"#),
        Just(r#"{"shift":true}"#),
        Just(r#"{"key":"OBS_KEY_VK_VOLUME_UP"}"#),
        Just(r#"{"key":"OBS_KEY_VK_VOLUME_DOWN"}"#),
        Just(r#"{"key":"OBS_KEY_VK_VOLUME_UP","volume_keys_fixed":true}"#),
        Just(r#"{"key":"OBS_KEY_VK_VOLUME_DOWN","volume_keys_fixed":false}"#),
    ]
}

fn items() -> impl Strategy<Value = String> {
    prop::collection::vec(item(), 0..4).prop_map(|items| format!("[{}]", items.join(",")))
}

fn maybe_items() -> impl Strategy<Value = Option<String>> {
    prop::option::of(items())
}

fn id() -> impl Strategy<Value = usize> {
    prop_oneof![1usize..=4, Just(MISSING_ID)]
}

fn pair_id() -> impl Strategy<Value = usize> {
    prop_oneof![1usize..=2, Just(MISSING_ID)]
}

fn kind() -> impl Strategy<Value = Kind> {
    prop_oneof![
        Just(Kind::Encoder),
        Just(Kind::Output),
        Just(Kind::Service),
        Just(Kind::Source),
    ]
}

fn object_data() -> impl Strategy<Value = Option<String>> {
    // A map: obs_data's JSON parser rejects a duplicate key.
    prop::option::of(
        prop::collection::btree_map(0usize..=5, items(), 0..4).prop_map(|fields| {
            let fields: Vec<String> = fields
                .into_iter()
                .map(|(id, items)| format!("\"key.{id}\":{items}"))
                .collect();
            format!("{{{}}}", fields.join(","))
        }),
    )
}

fn combo() -> impl Strategy<Value = obs_key_combination> {
    (
        0u32..16,
        prop_oneof![
            Just(1),
            Just(OBS_KEY_VK_VOLUME_UP),
            Just(OBS_KEY_VK_VOLUME_DOWN),
            0..200
        ],
    )
        .prop_map(|(modifiers, key)| obs_key_combination { modifiers, key })
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (id(), prop::collection::vec(combo(), 0..4)).prop_map(|(i, c)| Op::LoadBindings(i, c)),
        (id(), maybe_items()).prop_map(|(i, a)| Op::Load(i, a)),
        (pair_id(), maybe_items(), maybe_items()).prop_map(|(i, a, b)| Op::PairLoad(i, a, b)),
        id().prop_map(Op::Save),
        (pair_id(), any::<bool>(), any::<bool>()).prop_map(|(i, a, b)| Op::PairSave(i, a, b)),
        (kind(), 0..OBJECTS, object_data()).prop_map(|(k, w, d)| Op::LoadObject(k, w, d)),
        kind().prop_map(Op::LoadNullObject),
        (kind(), 0..OBJECTS).prop_map(|(k, w)| Op::SaveObject(k, w)),
    ]
}

// ---------------------------------------------------------------------------

#[test]
fn values_match_c() {
    for (i, (name, value)) in c::VALUE_NAMES
        .iter()
        .zip([OBS_KEY_VK_VOLUME_UP, OBS_KEY_VK_VOLUME_DOWN])
        .enumerate()
    {
        // SAFETY: a plain lookup.
        assert_eq!(
            unsafe { c::oracle_hotkey_save_load_value(i) },
            i64::from(value),
            "{name}"
        );
    }
}

/// The internal entry `obs_hotkey_register_internal()` calls, in all four
/// flag combinations.
#[test]
fn load_bindings_matches_c() {
    for (removed, saved, swapped) in (0..8).map(|n| (n & 1 != 0, n & 2 != 0, n & 4 != 0)) {
        for items in [
            None,
            Some("[]"),
            Some(r#"[{"key":"OBS_KEY_A"},{"key":"OBS_KEY_VK_VOLUME_UP"}]"#),
        ] {
            let run_side = |side: Side| {
                reset(true, swapped);
                let hotkey: *mut c_void =
                    REGISTRY.with_borrow_mut(|r| (&raw mut *r.hotkeys[0]).cast());
                let array = array_from(items);
                // SAFETY: a fake hotkey, a live array or null.
                unsafe {
                    match side {
                        Side::C => c::oracle_load_bindings(hotkey, array, removed, saved),
                        Side::Rust => rs::load_bindings(hotkey.cast(), array, removed, saved),
                    }
                }
                free_array(array);
                REGISTRY.with_borrow_mut(|r| std::mem::take(&mut r.log))
            };
            assert_eq!(run_side(Side::C), run_side(Side::Rust));
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(1000)))]

    #[test]
    fn sequences_match_c(
        ops in prop::collection::vec(op(), 0..12),
        lock_ok in prop::bool::weighted(0.9),
        swapped in any::<bool>(),
    ) {
        assert_parity(&ops, lock_ok, swapped);
    }
}
