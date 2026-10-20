//! Tier 3: in-memory results against `libobs/obs-data.c`.
//!
//! Intentional differences, excluded below:
//! - Item pointers stay stable across set. C may realloc the item.
//! - A user value left behind when `set_autoselect` changes the type is not
//!   read as the new type. C type-puns those bytes. Getters for the new type
//!   return the empty value.
//! - The safe core's file replace removes the destination before the rename
//!   and does not log. The shim calls `os_quick_*`. This file compares the
//!   JSON text, not the replace syscall.
//! - A null name or a null vec out-pointer does nothing.

use std::ffi::{CStr, CString, c_char, c_void};

use obs_c_oracle as _;
use obs_data::Data;
use obs_util as _;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_obs_data_create() -> *mut c_void;
    fn oracle_obs_data_create_from_json(json_string: *const c_char) -> *mut c_void;
    fn oracle_obs_data_release(data: *mut c_void);
    fn oracle_obs_data_get_json(data: *mut c_void) -> *const c_char;
    fn oracle_obs_data_get_json_with_defaults(data: *mut c_void) -> *const c_char;
    fn oracle_obs_data_get_json_pretty(data: *mut c_void) -> *const c_char;
    fn oracle_obs_data_set_string(data: *mut c_void, name: *const c_char, val: *const c_char);
    fn oracle_obs_data_set_int(data: *mut c_void, name: *const c_char, val: i64);
    fn oracle_obs_data_set_double(data: *mut c_void, name: *const c_char, val: f64);
    fn oracle_obs_data_set_bool(data: *mut c_void, name: *const c_char, val: bool);
    fn oracle_obs_data_set_default_string(
        data: *mut c_void,
        name: *const c_char,
        val: *const c_char,
    );
    fn oracle_obs_data_get_int(data: *mut c_void, name: *const c_char) -> i64;
    fn oracle_obs_data_has_user_value(data: *mut c_void, name: *const c_char) -> bool;
    fn oracle_obs_data_erase(data: *mut c_void, name: *const c_char);
    fn oracle_obs_data_item_byname(data: *mut c_void, name: *const c_char) -> *mut c_void;
    fn oracle_obs_data_item_remove(item: *mut *mut c_void);
    fn oracle_obs_data_item_release(item: *mut *mut c_void);
}

fn c_name(text: &str) -> CString {
    CString::new(text).unwrap()
}

fn oracle_json(ptr: *mut c_void) -> String {
    // SAFETY: `ptr` is a live oracle object. The string lives until the next dump.
    let text = unsafe { oracle_obs_data_get_json(ptr) };
    assert!(!text.is_null());
    unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned()
}

fn release(ptr: *mut c_void) {
    unsafe { oracle_obs_data_release(ptr) };
}

#[test]
fn scalars_match_oracle() {
    let rust = Data::new();
    rust.set_string("s", None);
    rust.set_int("i", -4);
    rust.set_double("d", 1.5);
    rust.set_bool("t", true);
    rust.set_bool("f", false);

    // SAFETY: the oracle calls follow obs-data.c. Each object is released.
    let c = unsafe { oracle_obs_data_create() };
    let s = c_name("s");
    let i = c_name("i");
    let d = c_name("d");
    let t = c_name("t");
    let f = c_name("f");
    unsafe {
        oracle_obs_data_set_string(c, s.as_ptr(), std::ptr::null());
        oracle_obs_data_set_int(c, i.as_ptr(), -4);
        oracle_obs_data_set_double(c, d.as_ptr(), 1.5);
        oracle_obs_data_set_bool(c, t.as_ptr(), true);
        oracle_obs_data_set_bool(c, f.as_ptr(), false);
    }
    assert_eq!(rust.json(), oracle_json(c));
    let pretty = unsafe { oracle_obs_data_get_json_pretty(c) };
    let pretty = unsafe { CStr::from_ptr(pretty) }
        .to_string_lossy()
        .into_owned();
    assert_eq!(rust.json_pretty(), pretty);
    assert_eq!(rust.get_int("d"), unsafe {
        oracle_obs_data_get_int(c, d.as_ptr())
    });
    release(c);
}

#[test]
fn json_shapes_match_oracle() {
    let text = r#"{"n":null,"o":{"k":2},"a":[1,{"k":"v"},"s",null]}"#;
    let rust = Data::from_json(text).unwrap();
    let c_text = c_name(text);
    let c = unsafe { oracle_obs_data_create_from_json(c_text.as_ptr()) };
    assert!(!c.is_null());
    assert_eq!(rust.json(), oracle_json(c));
    release(c);

    let dup = c_name(r#"{"a":1,"a":2}"#);
    assert!(unsafe { oracle_obs_data_create_from_json(dup.as_ptr()) }.is_null());
    assert!(Data::from_json(r#"{"a":1,"a":2}"#).is_none());
}

#[test]
fn defaults_json_matches_oracle() {
    let rust = Data::new();
    rust.set_default_string("only", Some("default"));
    rust.set_string("both", Some("user"));
    rust.set_default_string("both", Some("default"));

    let c = unsafe { oracle_obs_data_create() };
    let only = c_name("only");
    let both = c_name("both");
    let def = c_name("default");
    let user = c_name("user");
    unsafe {
        oracle_obs_data_set_default_string(c, only.as_ptr(), def.as_ptr());
        oracle_obs_data_set_string(c, both.as_ptr(), user.as_ptr());
        oracle_obs_data_set_default_string(c, both.as_ptr(), def.as_ptr());
    }
    assert_eq!(rust.json(), oracle_json(c));
    let with = unsafe { oracle_obs_data_get_json_with_defaults(c) };
    let with = unsafe { CStr::from_ptr(with) }
        .to_string_lossy()
        .into_owned();
    assert_eq!(rust.json_with_defaults(), with);
    release(c);
}

fn key() -> impl Strategy<Value = String> {
    "[a-z]{1,3}".prop_map(|s| s)
}

fn value() -> impl Strategy<Value = Scalar> {
    prop_oneof![
        any::<i16>().prop_map(Scalar::Int),
        any::<bool>().prop_map(Scalar::Bool),
        "[a-z0-9]{0,6}".prop_map(Scalar::Text),
    ]
}

#[derive(Clone, Debug)]
enum Scalar {
    Int(i16),
    Bool(bool),
    Text(String),
}

proptest! {
    #[test]
    fn random_scalars_match_oracle(pairs in prop::collection::vec((key(), value()), 0..5)) {
        let mut seen = std::collections::BTreeMap::new();
        for (name, value) in pairs {
            seen.insert(name, value);
        }
        let rust = Data::new();
        let c = unsafe { oracle_obs_data_create() };
        for (name, value) in &seen {
            let name_c = c_name(name);
            match value {
                Scalar::Int(n) => {
                    rust.set_int(name, i64::from(*n));
                    unsafe { oracle_obs_data_set_int(c, name_c.as_ptr(), i64::from(*n)) };
                }
                Scalar::Bool(b) => {
                    rust.set_bool(name, *b);
                    unsafe { oracle_obs_data_set_bool(c, name_c.as_ptr(), *b) };
                }
                Scalar::Text(text) => {
                    rust.set_string(name, Some(text));
                    let c_text = c_name(text);
                    unsafe { oracle_obs_data_set_string(c, name_c.as_ptr(), c_text.as_ptr()) };
                }
            }
        }
        prop_assert_eq!(rust.json(), oracle_json(c));
        release(c);
    }
}

/// One step on an object with up to 40 names, so it grows past and shrinks
/// below the size where the name index starts (#1453).
#[derive(Clone, Debug)]
enum Op {
    Set(u8, i64),
    Erase(u8),
    RemoveItem(u8),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0u8..40, any::<i64>()).prop_map(|(k, v)| Op::Set(k, v)),
        1 => (0u8..40).prop_map(Op::Erase),
        1 => (0u8..40).prop_map(Op::RemoveItem),
    ]
}

proptest! {
    #[test]
    fn many_keys_set_erase_remove_match_oracle(ops in prop::collection::vec(op(), 0..120)) {
        let rust = Data::new();
        // SAFETY: an oracle object, released at the end; names outlive each call.
        let c = unsafe { oracle_obs_data_create() };
        for op in &ops {
            let k = match op {
                Op::Set(k, _) | Op::Erase(k) | Op::RemoveItem(k) => *k,
            };
            let name = format!("key_{k}");
            let cn = c_name(&name);
            match op {
                Op::Set(_, v) => {
                    rust.set_int(&name, *v);
                    unsafe { oracle_obs_data_set_int(c, cn.as_ptr(), *v) };
                }
                Op::Erase(_) => {
                    rust.erase(&name);
                    unsafe { oracle_obs_data_erase(c, cn.as_ptr()) };
                }
                Op::RemoveItem(_) => {
                    if let Some(item) = rust.item(&name) {
                        item.remove();
                    }
                    unsafe {
                        let mut item = oracle_obs_data_item_byname(c, cn.as_ptr());
                        if !item.is_null() {
                            oracle_obs_data_item_remove(&mut item);
                            oracle_obs_data_item_release(&mut item);
                        }
                    }
                }
            }
            for probe in 0u8..40 {
                let name = format!("key_{probe}");
                let cn = c_name(&name);
                prop_assert_eq!(rust.get_int(&name), unsafe { oracle_obs_data_get_int(c, cn.as_ptr()) });
                prop_assert_eq!(rust.has_user_value(&name), unsafe { oracle_obs_data_has_user_value(c, cn.as_ptr()) });
            }
        }
        prop_assert_eq!(rust.json(), oracle_json(c));
        release(c);
    }
}
