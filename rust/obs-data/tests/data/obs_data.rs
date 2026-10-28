//! Tier 1 mirror of `test/cmocka/test_obs_data.c`.
//!
//! Null receivers are the C ABI. The unchanged cmocka test covers those.
//! A null name is a no-op in the shim and is not exercised here.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use obs_c_oracle as _;
use obs_data::{Array, Data, Fps, NumType, Type, Vec2};
use obs_util as _;

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

fn temp_dir() -> PathBuf {
    let n = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("obs-data-{}-{}", std::process::id(), n));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_null_and_missing() {
    assert!(Data::from_json("{").is_none());
    let missing = std::env::temp_dir().join(format!("obs-data-missing-{}", std::process::id()));
    assert!(Data::from_json_file(&missing).is_none());

    let data = Data::new();
    assert_eq!(data.get_string("missing"), "");
    assert_eq!(data.get_int("missing"), 0);
    assert_eq!(data.get_double("missing"), 0.0);
    assert!(!data.get_bool("missing"));
    assert!(data.get_obj("missing").is_none());
    assert!(data.get_array("missing").is_none());
    assert!(!data.has_user_value("missing"));
    assert_eq!(data.json(), "{}");
    assert_eq!(data.last_json().as_deref(), Some("{}"));

    // Only objects and arrays parse. An array root becomes an empty object.
    let data = Data::from_json("[]").unwrap();
    assert_eq!(data.json(), "{}");
    assert!(Data::from_json("1").is_none());
    assert!(Data::from_json("null").is_none());
    assert!(Data::from_json("true").is_none());
    assert!(Data::from_json("\"hi\"").is_none());
}

#[test]
fn test_scalars_and_json() {
    let data = Data::new();
    data.set_string("s", None);
    data.set_int("i", -4);
    data.set_double("d", 1.5);
    data.set_bool("t", true);
    data.set_bool("f", false);

    assert_eq!(data.get_string("s"), "");
    assert_eq!(data.get_int("i"), -4);
    assert_eq!(data.get_double("d"), 1.5);
    assert!(data.get_bool("t"));
    assert!(!data.get_bool("f"));
    assert!(data.has_user_value("f"));
    assert_eq!(data.json(), r#"{"s":"","i":-4,"d":1.5,"t":true,"f":false}"#);
    assert!(data.json_pretty().contains("\n    \"s\": \"\""));
    assert_eq!(data.get_int("d"), 1);

    let round = Data::from_json(&data.json()).unwrap();
    assert_eq!(round.get_string("s"), "");
    assert_eq!(round.get_int("i"), -4);
    assert_eq!(round.get_double("d"), 1.5);
    let item = round.item("i").unwrap();
    assert_eq!(item.numtype(), NumType::Int);
}

#[test]
fn test_json_shapes() {
    assert!(Data::from_json(r#"{"a":1,"a":2}"#).is_none());

    let data = Data::from_json(r#"{"n":null,"o":{"k":2},"a":[1,{"k":"v"},"s",null]}"#).unwrap();
    let item = data.item("n").unwrap();
    assert_eq!(item.ty(), Type::Object);
    assert!(item.has_user_value());
    assert!(data.get_obj("n").is_none());
    assert_eq!(data.json(), r#"{"n":null,"o":{"k":2},"a":[{"k":"v"}]}"#);

    let child = data.get_obj("o").unwrap();
    assert_eq!(child.get_int("k"), 2);

    let array = data.get_array("a").unwrap();
    assert_eq!(array.count(), 1);
    let child = array.item(0).unwrap();
    assert_eq!(child.get_string("k"), "v");
    assert!(array.item(1).is_none());
}

#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "scenario table: one block per ported step"
)]
fn test_default_and_autoselect() {
    let data = Data::new();
    data.set_default_string("only", Some("default"));
    assert!(!data.has_user_value("only"));
    assert!(data.has_default_value("only"));
    assert_eq!(data.get_string("only"), "default");
    assert_eq!(data.get_default_string("only"), "default");
    assert_eq!(data.json(), "{}");
    assert_eq!(data.json_with_defaults(), r#"{"only":"default"}"#);

    data.set_string("both", Some("user"));
    data.set_default_string("both", Some("default"));
    assert_eq!(data.get_string("both"), "user");
    assert_eq!(data.get_default_string("both"), "default");
    assert_eq!(data.json(), r#"{"both":"user"}"#);
    assert_eq!(
        data.json_with_defaults(),
        r#"{"only":"default","both":"user"}"#
    );

    data.set_default_int("both", 9);
    assert!(data.has_default_value("both"));
    assert_eq!(data.get_default_string("both"), "default");
    assert_eq!(data.get_default_int("both"), 0);

    data.set_autoselect_string("auto", Some("picked"));
    assert!(!data.has_user_value("auto"));
    assert!(data.has_autoselect_value("auto"));
    assert_eq!(data.get_string("auto"), "picked");
    assert_eq!(data.get_autoselect_string("auto"), "picked");
    assert_eq!(
        data.json_with_defaults(),
        r#"{"only":"default","both":"user","auto":"picked"}"#
    );

    data.set_default_string("layered", Some("front"));
    data.set_autoselect_string("layered", Some("back"));
    assert_eq!(data.get_string("layered"), "front");
    assert_eq!(data.get_autoselect_string("layered"), "back");

    data.unset_user_value("both");
    assert!(!data.has_user_value("both"));
    assert_eq!(data.get_string("both"), "default");
    assert_eq!(data.json(), "{}");

    data.set_string("retype", Some("hi"));
    data.set_autoselect_int("retype", 7);
    let item = data.item("retype").unwrap();
    assert_eq!(item.ty(), Type::Number);
    assert_eq!(item.get_autoselect_int(), 7);
    assert_eq!(item.get_string(), "");

    let defaults = Data::defaults_from(Some(&data));
    assert_eq!(defaults.get_string("only"), "default");
    assert!(defaults.has_user_value("only"));
    assert_eq!(defaults.get_string("both"), "default");

    let defaults = Data::defaults_from(None);
    assert_eq!(defaults.json(), "{}");
}

#[test]
fn test_apply_erase_clear() {
    let src = Data::new();
    src.set_string("keep", Some("a"));
    src.set_default_string("skip", Some("hidden"));
    let nested = Data::new();
    nested.set_int("n", 3);
    src.set_obj("child", Some(&nested));

    let dst = Data::new();
    dst.set_string("keep", Some("old"));
    dst.apply(&src);
    dst.apply(&dst);
    assert_eq!(dst.get_string("keep"), "a");
    assert!(!dst.has_user_value("skip"));
    let copied = dst.get_obj("child").unwrap();
    assert_eq!(copied.get_int("n"), 3);
    nested.set_int("n", 9);
    assert_eq!(copied.get_int("n"), 3);

    dst.erase("keep");
    assert!(!dst.has_user_value("keep"));
    assert_eq!(dst.get_string("keep"), "");
    dst.erase("missing");

    dst.set_string("gone", Some("x"));
    dst.set_default_string("gone", Some("y"));
    dst.clear();
    assert!(!dst.has_user_value("gone"));
    assert!(dst.has_default_value("gone"));
    assert_eq!(dst.get_string("gone"), "y");
}

#[test]
fn test_arrays_and_iteration() {
    let first = Data::new();
    let second = Data::new();
    let third = Data::new();
    first.set_string("k", Some("a"));
    second.set_string("k", Some("b"));
    third.set_string("k", Some("c"));

    assert_eq!(Array::count_of(None), 0);
    let array = Array::new();
    assert_eq!(Array::push_of(None, Some(&first)), 0);
    assert_eq!(array.push(None), 0);
    array.push(Some(&first));
    array.push(Some(&second));
    array.insert(0, Some(&third));
    assert_eq!(array.count(), 3);
    assert_eq!(array.item(0).unwrap().get_string("k"), "c");
    assert_eq!(array.item(1).unwrap().get_string("k"), "a");

    array.erase(1);
    assert_eq!(array.count(), 2);
    assert_eq!(array.item(1).unwrap().get_string("k"), "b");

    let extra = Array::new();
    extra.push(Some(&first));
    array.push_array(&extra);
    assert_eq!(array.count(), 3);

    let data = Data::new();
    data.set_array("arr", Some(&array));
    data.set_string("z", Some("last"));
    data.set_int("m", 1);

    let item = data.first().unwrap();
    assert_eq!(item.name(), "arr");
    assert_eq!(item.ty(), Type::Array);
    let item = item.next().unwrap();
    assert_eq!(item.name(), "z");
    let item = item.next().unwrap();
    assert_eq!(item.name(), "m");
    assert_eq!(item.numtype(), NumType::Int);
    assert!(item.next().is_none());

    let item = data.item("z").unwrap();
    item.remove();
    assert_eq!(item.name(), "z");
    assert!(!data.has_user_value("z"));
}

#[test]
fn test_vec_and_frames() {
    let data = Data::new();
    data.set_vec2("v", Vec2 { x: 1.5, y: -2.25 });
    let out = data.get_vec2("v").unwrap();
    assert_eq!(out.x, 1.5);
    assert_eq!(out.y, -2.25);
    assert!(data.get_vec2("missing").is_none());

    data.set_fps(
        "rate",
        Fps {
            numerator: 30000,
            denominator: 1001,
        },
        None,
    );
    let read = data.get_fps("rate", true);
    assert!(read.ok);
    assert_eq!(
        read.fps,
        Some(Fps {
            numerator: 30000,
            denominator: 1001
        })
    );
    assert!(read.option.is_none());

    data.set_fps(
        "named",
        Fps {
            numerator: 0,
            denominator: 0,
        },
        Some("ntsc"),
    );
    let read = data.get_fps("named", true);
    assert!(read.ok);
    assert_eq!(read.option.as_deref(), Some("ntsc"));
    let read = data.get_fps("named", false);
    assert!(!read.ok);

    let obj = Data::new();
    obj.set_int("numerator", -5);
    obj.set_int("denominator", 1);
    data.set_obj("bad", Some(&obj));
    let read = data.get_fps("bad", true);
    assert!(!read.ok);
    assert_eq!(
        read.fps,
        Some(Fps {
            numerator: 0,
            denominator: 1
        })
    );
    assert!(read.option.is_none());
}

#[test]
fn test_json_files() {
    let dir = temp_dir();
    let save = dir.join("save.json");
    let safe = dir.join("safe.json");
    let safe_tmp = dir.join("safe.json.tmp");
    let safe_bak = dir.join("safe.json.bak");
    let broken = dir.join("broken.json");
    let broken_bak = dir.join("broken.json.bak");

    let data = Data::new();
    data.set_string("name", Some("one"));
    assert!(data.save_json(&save));
    let loaded = Data::from_json_file(&save).unwrap();
    assert_eq!(loaded.get_string("name"), "one");

    data.set_string("name", Some("two"));
    assert!(data.save_json_safe(&safe, "tmp", "bak"));
    assert_eq!(fs::read_to_string(&safe).unwrap(), r#"{"name":"two"}"#);
    assert!(!safe_tmp.exists());

    data.set_string("name", Some("three"));
    assert!(data.save_json_safe(&safe, "tmp", "bak"));
    assert_eq!(fs::read_to_string(&safe).unwrap(), r#"{"name":"three"}"#);
    assert_eq!(fs::read_to_string(&safe_bak).unwrap(), r#"{"name":"two"}"#);

    fs::write(&broken, "not-json").unwrap();
    fs::write(&broken_bak, r#"{"name":"bak"}"#).unwrap();
    assert!(Data::from_json_file(&broken).is_none());
    let loaded = Data::from_json_file_safe(&broken, "bak").unwrap();
    assert_eq!(loaded.get_string("name"), "bak");
    assert_eq!(fs::read_to_string(&broken).unwrap(), r#"{"name":"bak"}"#);
    assert!(!broken_bak.exists());

    fs::write(dir.join("empty.json"), "").unwrap();
    assert!(Data::from_json_file(&dir.join("empty.json")).is_none());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn vec3_json_omits_w() {
    let data = Data::new();
    data.set_vec3(
        "p",
        obs_data::Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            w: 4.0,
        },
    );
    assert_eq!(data.json(), r#"{"p":{"x":1.0,"y":2.0,"z":3.0}}"#);
    let got = data.get_vec3("p").unwrap();
    assert_eq!(got.x, 1.0);
    assert_eq!(got.y, 2.0);
    assert_eq!(got.z, 3.0);
}
