//! Tier 3 parity: the shims against the original
//! `libobs/obs-hotkey-bindings.c.h` (oracle/hotkey_bindings.c).
//!
//! `test_hotkey_load`, `test_wayland_hotkeys` and `test_portal_shortcuts`
//! compile the C into themselves, so they never see the Rust swap;
//! `test_hotkey_bindings_codec` sees it through a real libobs, and this
//! file compares the two directly.
//!
//! Both sides read and write obs_data through the Rust obs-data port, and
//! name keys through the Rust name map; each has its own parity test.
//!
//! - Parse: the same obs_data object goes to both. It is built by a random
//!   sequence of obs_data calls on modifier names, near-miss names
//!   (`"Shift"`, `"ctrl"`, `"KEY"`, `""`) and others: user, default and
//!   autoselect bools and strings, ints, doubles, objects, arrays and
//!   erases. Strings are key names, near misses, `""` and arbitrary bytes
//!   (non-UTF-8 included). Also every characterization input, through
//!   obs_data's JSON parser, and a null obs_data. Compared: the return value
//!   and the combination written.
//! - Bindable: every key code and the codes around the ends, with any
//!   modifier bits.
//! - Serialize: any modifier bits and any key code, in range or not, into
//!   two obs_data objects that start the same (empty, or filled by the same
//!   random calls). Compared: the JSON of each, byte for byte. Then the
//!   saved item is parsed back on both sides.
//!
//! Intentional differences, excluded here: a null `combo_out` returns false
//! in Rust; C writes through it. Nothing else is excluded.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::{CStr, CString, c_void};

use obs_c_oracle::hotkey_bindings::{
    OracleKeyCombination, VALUE_NAMES, oracle_hotkey_bindings_value,
    oracle_obs_hotkey_combination_bindable, oracle_obs_hotkey_combination_from_data,
    oracle_obs_hotkey_combination_to_data,
};
use obs_data::ffi::obs_data::{
    obs_data_array_create, obs_data_array_release, obs_data_create, obs_data_create_from_json,
    obs_data_erase, obs_data_get_json, obs_data_release, obs_data_set_array,
    obs_data_set_autoselect_bool, obs_data_set_autoselect_string, obs_data_set_bool,
    obs_data_set_default_bool, obs_data_set_default_string, obs_data_set_double, obs_data_set_int,
    obs_data_set_obj, obs_data_set_string,
};
use obs_hotkey::ffi::bindings::{
    obs_hotkey_combination_bindable, obs_hotkey_combination_from_data,
    obs_hotkey_combination_to_data,
};
use obs_hotkey::ffi::nix_hotkey_state::obs_key_combination;
use obs_hotkey::name_map::{
    KEY_NONE, OBS_KEY_LAST_VALUE, OBS_KEY_MOUSE1, OBS_KEY_MOUSE2, key_name,
};
use obs_hotkey::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
};
use proptest::prelude::*;

const LAST: i32 = OBS_KEY_LAST_VALUE as i32;

#[test]
fn values_match_c() {
    let rust: [i64; 8] = [
        KEY_NONE.into(),
        LAST.into(),
        OBS_KEY_MOUSE1.into(),
        OBS_KEY_MOUSE2.into(),
        INTERACT_SHIFT_KEY.into(),
        INTERACT_CONTROL_KEY.into(),
        INTERACT_ALT_KEY.into(),
        INTERACT_COMMAND_KEY.into(),
    ];
    for (i, (name, value)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain lookup.
        assert_eq!(unsafe { oracle_hotkey_bindings_value(i) }, value, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle_hotkey_bindings_value(VALUE_NAMES.len()) },
        -1
    );
}

/// An owned obs_data, released on drop.
struct Data(*mut c_void);

impl Data {
    fn new() -> Self {
        // SAFETY: plain constructor.
        Self(unsafe { obs_data_create() })
    }

    fn from_json(json: &str) -> Self {
        let json = CString::new(json).unwrap();
        // SAFETY: a NUL-terminated string.
        let d = unsafe { obs_data_create_from_json(json.as_ptr()) };
        assert!(!d.is_null(), "{json:?}");
        Self(d)
    }

    /// The object's JSON, as `obs_data_get_json` writes it.
    fn json(&self) -> Vec<u8> {
        // SAFETY: a live obs_data; the string lives until the next call.
        let p = unsafe { obs_data_get_json(self.0) };
        assert!(!p.is_null());
        // SAFETY: NUL-terminated, copied before any other call.
        unsafe { CStr::from_ptr(p) }.to_bytes().to_vec()
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        // SAFETY: owned reference.
        unsafe { obs_data_release(self.0) };
    }
}

/// The item names the calls use: the codec's, near misses, and others.
const NAMES: [&CStr; 10] = [
    c"shift", c"control", c"alt", c"command", c"key", c"Shift", c"ctrl", c"KEY", c"", c"other",
];

/// One obs_data call on `NAMES[name]`.
#[derive(Debug, Clone)]
enum Op {
    Bool(bool),
    DefaultBool(bool),
    AutoselectBool(bool),
    Str(CString),
    DefaultStr(CString),
    AutoselectStr(CString),
    Int(i64),
    Double(f64),
    Obj,
    Array,
    Erase,
}

fn apply(data: &Data, ops: &[(usize, Op)]) {
    for (name, op) in ops {
        let n = NAMES[*name].as_ptr();
        let d = data.0;
        // SAFETY: a live obs_data, a static name and NUL-terminated values.
        unsafe {
            match op {
                Op::Bool(b) => obs_data_set_bool(d, n, *b),
                Op::DefaultBool(b) => obs_data_set_default_bool(d, n, *b),
                Op::AutoselectBool(b) => obs_data_set_autoselect_bool(d, n, *b),
                Op::Str(s) => obs_data_set_string(d, n, s.as_ptr()),
                Op::DefaultStr(s) => obs_data_set_default_string(d, n, s.as_ptr()),
                Op::AutoselectStr(s) => obs_data_set_autoselect_string(d, n, s.as_ptr()),
                Op::Int(v) => obs_data_set_int(d, n, *v),
                Op::Double(v) => obs_data_set_double(d, n, *v),
                Op::Obj => {
                    let o = obs_data_create();
                    obs_data_set_obj(d, n, o);
                    obs_data_release(o);
                }
                Op::Array => {
                    let a = obs_data_array_create();
                    obs_data_set_array(d, n, a);
                    obs_data_array_release(a);
                }
                Op::Erase => obs_data_erase(d, n),
            }
        }
    }
}

fn key_name_bytes(k: i32) -> Vec<u8> {
    key_name(k).unwrap().to_bytes().to_vec()
}

/// Key names, near misses and arbitrary bytes, without NUL.
fn name_bytes() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        4 => (KEY_NONE..LAST).prop_map(key_name_bytes),
        1 => (KEY_NONE..LAST).prop_map(|k| key_name_bytes(k).to_ascii_lowercase()),
        1 => (KEY_NONE..LAST).prop_map(|k| {
            let mut b = key_name_bytes(k);
            b.push(b' ');
            b
        }),
        1 => (KEY_NONE..LAST).prop_map(|k| key_name_bytes(k)[8..].to_vec()),
        1 => Just(Vec::new()),
        1 => Just(b"OBS_KEY_NONE".to_vec()),
        2 => proptest::collection::vec(1u8..=255, 0..40),
    ]
}

fn cstring() -> impl Strategy<Value = CString> {
    name_bytes().prop_map(|b| CString::new(b).unwrap())
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => any::<bool>().prop_map(Op::Bool),
        1 => any::<bool>().prop_map(Op::DefaultBool),
        1 => any::<bool>().prop_map(Op::AutoselectBool),
        4 => cstring().prop_map(Op::Str),
        1 => cstring().prop_map(Op::DefaultStr),
        1 => cstring().prop_map(Op::AutoselectStr),
        1 => any::<i64>().prop_map(Op::Int),
        1 => any::<f64>().prop_map(Op::Double),
        1 => Just(Op::Obj),
        1 => Just(Op::Array),
        1 => Just(Op::Erase),
    ]
}

fn ops() -> impl Strategy<Value = Vec<(usize, Op)>> {
    proptest::collection::vec((0..NAMES.len(), op()), 0..14)
}

/// Any modifier bits, weighted toward the four the codec knows.
fn modifiers() -> impl Strategy<Value = u32> {
    prop_oneof![
        3 => 0u32..16,
        1 => 0u32..0x1000,
        1 => any::<u32>(),
    ]
}

/// Any key code, mostly in range.
fn key_code() -> impl Strategy<Value = i32> {
    prop_oneof![
        8 => KEY_NONE..LAST,
        1 => -3..0,
        1 => LAST..LAST + 3,
        1 => any::<i32>(),
    ]
}

const SENTINEL: u32 = 0xA5A5_A5A5;

fn oracle_from(data: *mut c_void) -> (bool, OracleKeyCombination) {
    let mut c = OracleKeyCombination {
        modifiers: SENTINEL,
        key: -7,
    };
    // SAFETY: a live obs_data or null, and a writable combination.
    let ok = unsafe { oracle_obs_hotkey_combination_from_data(data, &mut c) };
    (ok, c)
}

fn shim_from(data: *mut c_void) -> (bool, OracleKeyCombination) {
    let mut c = obs_key_combination {
        modifiers: SENTINEL,
        key: -7,
    };
    // SAFETY: as above.
    let ok = unsafe { obs_hotkey_combination_from_data(data, &mut c) };
    (
        ok,
        OracleKeyCombination {
            modifiers: c.modifiers,
            key: c.key,
        },
    )
}

fn assert_from_parity(data: *mut c_void) {
    let (c_ok, c) = oracle_from(data);
    let (r_ok, r) = shim_from(data);
    assert_eq!(r_ok, c_ok);
    assert_eq!((r.modifiers, r.key), (c.modifiers, c.key));
}

/// Saves `(modifiers, key)` into two objects that start as `ops` make
/// them, one per side, and compares their JSON; then parses each back.
fn assert_to_parity(modifiers: u32, key: i32, ops: &[(usize, Op)]) {
    let c_data = Data::new();
    let r_data = Data::new();
    apply(&c_data, ops);
    apply(&r_data, ops);
    // SAFETY: live obs_data objects.
    unsafe {
        oracle_obs_hotkey_combination_to_data(OracleKeyCombination { modifiers, key }, c_data.0);
        obs_hotkey_combination_to_data(obs_key_combination { modifiers, key }, r_data.0);
    }
    let c_json = c_data.json();
    assert_eq!(
        String::from_utf8_lossy(&r_data.json()),
        String::from_utf8_lossy(&c_json),
        "{modifiers:#x} {key}"
    );
    assert_eq!(r_data.json(), c_json);
    assert_from_parity(c_data.0);
    assert_from_parity(r_data.0);
}

/// The characterization inputs, one binding item each, through the JSON
/// parser.
const ITEMS: &[&str] = &[
    "{\"shift\":true,\"key\":\"OBS_KEY_A\"}",
    "{\"control\":true,\"key\":\"OBS_KEY_A\"}",
    "{\"alt\":true,\"key\":\"OBS_KEY_A\"}",
    "{\"command\":true,\"key\":\"OBS_KEY_A\"}",
    "{\"key\":\"OBS_KEY_A\"}",
    "{\"key\":\"OBS_KEY_F1\",\"command\":true,\"alt\":true,\"control\":true,\"shift\":true}",
    "{\"shift\":false,\"control\":false,\"alt\":false,\"command\":false,\"key\":\"OBS_KEY_F1\"}",
    "{\"shift\":\"true\",\"control\":1,\"alt\":1.0,\"command\":null,\"key\":\"OBS_KEY_B\"}",
    "{\"shift\":{},\"control\":[],\"Shift\":true,\"ctrl\":true,\"key\":\"OBS_KEY_B\"}",
    "{\"key\":\"OBS_KEY_NOT_A_KEY\"}",
    "{\"key\":\"obs_key_a\"}",
    "{\"key\":\" OBS_KEY_A\"}",
    "{\"key\":\"OBS_KEY_A \"}",
    "{\"key\":\"A\"}",
    "{\"key\":\"\"}",
    "{\"key\":65}",
    "{\"key\":true}",
    "{\"key\":null}",
    "{\"key\":{}}",
    "{\"key\":\"OBS_KEY_NONE\"}",
    "{}",
    "{\"unrelated\":1}",
    "{\"shift\":true}",
    "{\"control\":true,\"alt\":true,\"key\":\"bogus\"}",
    "{\"key\":\"OBS_KEY_MOUSE1\"}",
    "{\"shift\":true,\"key\":\"OBS_KEY_MOUSE2\"}",
    "{\"key\":\"OBS_KEY_MOUSE3\"}",
    "{\"control\":true,\"key\":\"OBS_KEY_MOUSE29\"}",
    "{\"key\":\"OBS_KEY_\\u00e9\"}",
];

#[test]
fn characterization_items_match_c() {
    for json in ITEMS {
        let d = Data::from_json(json);
        assert_from_parity(d.0);
    }
}

#[test]
fn null_data_matches_c() {
    assert_from_parity(std::ptr::null_mut());
    // SAFETY: obs_data ignores a null object; nothing to compare but that
    // neither side crashes.
    unsafe {
        oracle_obs_hotkey_combination_to_data(
            OracleKeyCombination {
                modifiers: 0xFFFF_FFFF,
                key: 1,
            },
            std::ptr::null_mut(),
        );
        obs_hotkey_combination_to_data(
            obs_key_combination {
                modifiers: 0xFFFF_FFFF,
                key: 1,
            },
            std::ptr::null_mut(),
        );
    }
}

/// Intentional difference: a null `combo_out` returns false in Rust.
#[test]
fn null_combo_out_returns_false() {
    let d = Data::from_json("{\"key\":\"OBS_KEY_A\"}");
    // SAFETY: a live obs_data; a null out-pointer is the case under test.
    assert!(!unsafe { obs_hotkey_combination_from_data(d.0, std::ptr::null_mut()) });
}

#[test]
fn bindable_matches_c_for_every_key() {
    for key in [i32::MIN, -2, -1, LAST, LAST + 1, i32::MAX]
        .into_iter()
        .chain(KEY_NONE..LAST)
    {
        for modifiers in [0, INTERACT_SHIFT_KEY, 0xFFFF_FFFF] {
            // SAFETY: by-value calls.
            let c = unsafe {
                oracle_obs_hotkey_combination_bindable(OracleKeyCombination { modifiers, key })
            };
            let r = obs_hotkey_combination_bindable(obs_key_combination { modifiers, key });
            assert_eq!(r, c, "{key}");
        }
    }
}

#[test]
fn to_data_matches_c_for_every_key_and_modifier_set() {
    for key in [-1, LAST].into_iter().chain(KEY_NONE..LAST) {
        for modifiers in 0..16u32 {
            let bits = [
                INTERACT_SHIFT_KEY,
                INTERACT_CONTROL_KEY,
                INTERACT_ALT_KEY,
                INTERACT_COMMAND_KEY,
            ]
            .iter()
            .enumerate()
            .filter(|(i, _)| modifiers & (1 << i) != 0)
            .fold(0, |acc, (_, b)| acc | b);
            assert_to_parity(bits, key, &[]);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(2000)))]

    #[test]
    fn from_data_matches_c(ops in ops()) {
        let d = Data::new();
        apply(&d, &ops);
        assert_from_parity(d.0);
    }

    #[test]
    fn bindable_matches_c(modifiers in modifiers(), key in key_code()) {
        // SAFETY: by-value calls.
        let c = unsafe {
            oracle_obs_hotkey_combination_bindable(OracleKeyCombination { modifiers, key })
        };
        let r = obs_hotkey_combination_bindable(obs_key_combination { modifiers, key });
        prop_assert_eq!(r, c);
    }

    #[test]
    fn to_data_matches_c(modifiers in modifiers(), key in key_code(), ops in ops()) {
        assert_to_parity(modifiers, key, &ops);
    }
}
