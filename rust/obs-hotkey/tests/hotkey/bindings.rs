//! Tier 1: the safe core of `libobs/obs-hotkey-bindings.c.h`.
//!
//! The first eighteen tests mirror `test/cmocka/test_hotkey_bindings_codec.c`
//! one to one, with the same names, the same JSON and the same expected
//! bytes. The C test goes
//! through the public hotkey API of a real libobs: obs_data parses the
//! JSON, `obs-hotkey.c.h` (still C) keeps the bindings and sends
//! `hotkey_bindings_changed`, and the codec reads and writes each item. Here
//! [`Model`] repeats what that C does around the codec (`load_bindings`,
//! `load_binding`, `create_binding`, `obs_hotkey_load_bindings`,
//! `save_bindings_helper`, and a source's `enum_load_bindings`), and
//! [`json`] what obs_data does with the JSON (only a JSON `true` reads as a
//! true bool, a missing or non-string `"key"` reads as `""`, array items
//! that are not objects are dropped), so every expectation of the C test
//! runs against the Rust codec.
//!
//! The cases after those pin the edges of the safe API.

use std::ffi::{CStr, CString};

use obs_hotkey::bindings::{
    Field, KEY_FIELD, MODIFIERS, combo_bindable, combo_from_fields, combo_to_fields,
};
use obs_hotkey::name_map::{KEY_NONE, OBS_KEY_LAST_VALUE, OBS_KEY_MOUSE1, key_from_name, key_name};
use obs_hotkey::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
    KeyCombination,
};

const LAST: i32 = OBS_KEY_LAST_VALUE as i32;

/// `INTERACT_CAPS_KEY` and `INTERACT_MOUSE_LEFT` (`obs-interaction.h`).
const INTERACT_CAPS_KEY: u32 = 1;
const INTERACT_MOUSE_LEFT: u32 = 1 << 4;

fn key(name: &str) -> i32 {
    let k = key_from_name(&CString::new(name).unwrap());
    assert_ne!(k, KEY_NONE, "{name} is a key");
    k
}

fn combo(modifiers: u32, key: i32) -> KeyCombination {
    KeyCombination { modifiers, key }
}

/// The JSON obs_data reads and writes, as far as the codec sees it.
mod json {
    /// A JSON value.
    #[derive(Debug, Clone, PartialEq)]
    pub enum Json {
        Null,
        Bool(bool),
        Number,
        Str(String),
        Array(Vec<Json>),
        Object(Vec<(String, Json)>),
    }

    impl Json {
        /// `obs_data_get_bool`: true only for a JSON `true`.
        pub fn get_bool(&self, name: &str) -> bool {
            matches!(self.get(name), Some(Json::Bool(true)))
        }

        /// `obs_data_get_string`: `""` unless the item is a string.
        pub fn get_string(&self, name: &str) -> &str {
            match self.get(name) {
                Some(Json::Str(s)) => s,
                _ => "",
            }
        }

        /// `obs_data_get_array`: the object items of an array, or `None`
        /// when the item is not an array.
        pub fn get_array(&self, name: &str) -> Option<Vec<Json>> {
            match self.get(name) {
                Some(a @ Json::Array(_)) => Some(a.objects()),
                _ => None,
            }
        }

        /// The items obs_data keeps of an array: its objects, in order.
        pub fn objects(&self) -> Vec<Json> {
            match self {
                Json::Array(items) => items
                    .iter()
                    .filter(|i| matches!(i, Json::Object(_)))
                    .cloned()
                    .collect(),
                _ => panic!("not an array: {self:?}"),
            }
        }

        /// The item named `name`. obs_data rejects JSON with a duplicate
        /// name, so there is at most one.
        fn get(&self, name: &str) -> Option<&Json> {
            match self {
                Json::Object(items) => items.iter().find(|(n, _)| n == name).map(|(_, v)| v),
                _ => None,
            }
        }
    }

    /// Parses the JSON the C test writes (no escapes, no whitespace).
    pub fn parse(text: &str) -> Json {
        let mut p = Parser {
            s: text.as_bytes(),
            i: 0,
        };
        let v = p.value();
        assert_eq!(p.i, p.s.len(), "trailing bytes in {text}");
        v
    }

    struct Parser<'a> {
        s: &'a [u8],
        i: usize,
    }

    impl Parser<'_> {
        fn peek(&self) -> u8 {
            self.s[self.i]
        }

        fn eat(&mut self, b: u8) -> bool {
            let hit = self.s.get(self.i) == Some(&b);
            if hit {
                self.i += 1;
            }
            hit
        }

        fn word(&mut self, w: &str) -> bool {
            let hit = self.s[self.i..].starts_with(w.as_bytes());
            if hit {
                self.i += w.len();
            }
            hit
        }

        fn string(&mut self) -> String {
            assert!(self.eat(b'"'));
            let start = self.i;
            while self.peek() != b'"' {
                assert_ne!(self.peek(), b'\\', "no escapes");
                self.i += 1;
            }
            let s = std::str::from_utf8(&self.s[start..self.i])
                .unwrap()
                .to_owned();
            self.i += 1;
            s
        }

        fn value(&mut self) -> Json {
            match self.peek() {
                b'"' => Json::Str(self.string()),
                b'[' => {
                    self.i += 1;
                    let mut items = Vec::new();
                    if !self.eat(b']') {
                        loop {
                            items.push(self.value());
                            if self.eat(b']') {
                                break;
                            }
                            assert!(self.eat(b','));
                        }
                    }
                    Json::Array(items)
                }
                b'{' => {
                    self.i += 1;
                    let mut items = Vec::new();
                    if !self.eat(b'}') {
                        loop {
                            let name = self.string();
                            assert!(self.eat(b':'));
                            items.push((name, self.value()));
                            if self.eat(b'}') {
                                break;
                            }
                            assert!(self.eat(b','));
                        }
                    }
                    Json::Object(items)
                }
                _ if self.word("true") => Json::Bool(true),
                _ if self.word("false") => Json::Bool(false),
                _ if self.word("null") => Json::Null,
                _ => {
                    let start = self.i;
                    while self
                        .s
                        .get(self.i)
                        .is_some_and(|b| b.is_ascii_digit() || b"-+.eE".contains(b))
                    {
                        self.i += 1;
                    }
                    assert!(self.i > start, "bad JSON at {start}");
                    Json::Number
                }
            }
        }
    }
}

use json::Json;

/// The codec's parse of one item: what `load_binding` reads.
fn from_item(item: &Json) -> Option<KeyCombination> {
    let modifiers = MODIFIERS.map(|m| item.get_bool(m.name.to_str().unwrap()));
    let key_name = CString::new(item.get_string(KEY_FIELD.to_str().unwrap())).unwrap();
    combo_from_fields(modifiers, &key_name)
}

/// The codec's save of one binding, as obs_data writes the item.
fn item_json(c: KeyCombination) -> String {
    let fields: Vec<String> = combo_to_fields(c)
        .map(|f| match f {
            Field::Modifier(name) => format!("\"{}\":true", name.to_str().unwrap()),
            Field::Key(k) => format!(
                "\"{}\":\"{}\"",
                KEY_FIELD.to_str().unwrap(),
                key_name(k).unwrap_or(c"").to_str().unwrap()
            ),
        })
        .collect();
    format!("{{{}}}", fields.join(","))
}

/// The C around the codec: the hotkeys, `obs->hotkeys.bindings` and the
/// `hotkey_bindings_changed` count per hotkey.
#[derive(Default)]
struct Model {
    names: Vec<String>,
    bindings: Vec<(usize, KeyCombination)>,
    changed: Vec<usize>,
}

impl Model {
    fn register(&mut self, name: &str) -> usize {
        self.names.push(name.to_owned());
        self.changed.push(0);
        self.names.len() - 1
    }

    /// `remove_bindings`: true if it removed any.
    fn remove(&mut self, id: usize) -> bool {
        let before = self.bindings.len();
        self.bindings.retain(|(h, _)| *h != id);
        self.bindings.len() != before
    }

    /// `create_binding`.
    fn create(&mut self, id: usize, c: KeyCombination) {
        if combo_bindable(c) {
            self.bindings.push((id, c));
        }
    }

    /// `load_bindings`: every item, then the signal if the array has any or
    /// the caller removed bindings first.
    fn load_bindings(&mut self, id: usize, items: &[Json], removed: bool) {
        for item in items {
            // load_binding
            if let Some(c) = from_item(item) {
                self.create(id, c);
            }
        }
        if !items.is_empty() || removed {
            self.changed[id] += 1;
        }
    }

    /// `obs_hotkey_load(id, array)`; `None` is a NULL array.
    fn load_array(&mut self, id: usize, array: Option<&Json>) {
        let removed = self.remove(id);
        self.load_bindings(id, &array.map(Json::objects).unwrap_or_default(), removed);
    }

    fn load(&mut self, id: usize, array_json: &str) {
        self.load_array(id, Some(&json::parse(array_json)));
    }

    /// `obs_hotkey_load_bindings`.
    fn load_api(&mut self, id: usize, combos: &[KeyCombination]) {
        let removed = self.remove(id);
        for &c in combos {
            self.create(id, c);
        }
        if !combos.is_empty() || removed {
            self.changed[id] += 1;
        }
    }

    /// `obs_hotkey_save`: `None` for an unknown id.
    fn save(&self, id: usize) -> Option<String> {
        self.names.get(id)?;
        let items: Vec<String> = self
            .bindings
            .iter()
            .filter(|(h, _)| *h == id)
            .map(|(_, c)| item_json(*c))
            .collect();
        Some(format!("[{}]", items.join(",")))
    }

    fn assert_saved(&self, id: usize, expected: &str) {
        assert_eq!(self.save(id).unwrap(), expected);
    }

    /// load(input), then the saved JSON is `expected`, and loading that
    /// saves it again unchanged (save -> load -> save is a fixpoint).
    fn assert_load_saves(&mut self, id: usize, input: &str, expected: &str) {
        self.load(id, input);
        self.assert_saved(id, expected);
        self.load(id, expected);
        self.assert_saved(id, expected);
    }
}

/// The C test's setup: one frontend hotkey.
fn setup() -> (Model, usize) {
    let mut m = Model::default();
    let id = m.register("test.codec");
    (m, id)
}

// ---------------------------------------------------------------------------
// Modifiers

#[test]
fn each_modifier_alone() {
    let (mut m, h) = setup();
    m.assert_load_saves(
        h,
        "[{\"shift\":true,\"key\":\"OBS_KEY_A\"}]",
        "[{\"shift\":true,\"key\":\"OBS_KEY_A\"}]",
    );
    m.assert_load_saves(
        h,
        "[{\"control\":true,\"key\":\"OBS_KEY_A\"}]",
        "[{\"control\":true,\"key\":\"OBS_KEY_A\"}]",
    );
    m.assert_load_saves(
        h,
        "[{\"alt\":true,\"key\":\"OBS_KEY_A\"}]",
        "[{\"alt\":true,\"key\":\"OBS_KEY_A\"}]",
    );
    m.assert_load_saves(
        h,
        "[{\"command\":true,\"key\":\"OBS_KEY_A\"}]",
        "[{\"command\":true,\"key\":\"OBS_KEY_A\"}]",
    );
    m.assert_load_saves(h, "[{\"key\":\"OBS_KEY_A\"}]", "[{\"key\":\"OBS_KEY_A\"}]");
}

/// Saved keys come in a fixed order (shift, control, alt, command, key)
/// whatever order the input had; a false modifier is not written.
#[test]
fn modifier_order_and_false() {
    let (mut m, h) = setup();
    m.assert_load_saves(
        h,
        "[{\"key\":\"OBS_KEY_F1\",\"command\":true,\"alt\":true,\"control\":true,\"shift\":true}]",
        "[{\"shift\":true,\"control\":true,\"alt\":true,\"command\":true,\"key\":\"OBS_KEY_F1\"}]",
    );
    m.assert_load_saves(
        h,
        "[{\"key\":\"OBS_KEY_F1\",\"alt\":true,\"shift\":true}]",
        "[{\"shift\":true,\"alt\":true,\"key\":\"OBS_KEY_F1\"}]",
    );
    m.assert_load_saves(
        h,
        "[{\"shift\":false,\"control\":false,\"alt\":false,\"command\":false,\"key\":\"OBS_KEY_F1\"}]",
        "[{\"key\":\"OBS_KEY_F1\"}]",
    );
}

/// Only a JSON true sets a modifier: a string, a number, null, an object or
/// an array does not. Unknown keys in an item are dropped.
#[test]
fn modifier_not_a_bool() {
    let (mut m, h) = setup();
    m.assert_load_saves(
        h,
        "[{\"shift\":\"true\",\"control\":1,\"alt\":1.0,\"command\":null,\"key\":\"OBS_KEY_B\"}]",
        "[{\"key\":\"OBS_KEY_B\"}]",
    );
    m.assert_load_saves(
        h,
        "[{\"shift\":{},\"control\":[],\"Shift\":true,\"ctrl\":true,\"key\":\"OBS_KEY_B\"}]",
        "[{\"key\":\"OBS_KEY_B\"}]",
    );
}

/// All 16 modifier sets on one key, in one array, in order.
#[test]
fn all_modifier_sets() {
    let (mut m, h) = setup();
    let names = ["shift", "control", "alt", "command"];
    let mut json = String::from("[");
    for set in 0..16 {
        json.push_str(if set != 0 { ",{" } else { "{" });
        for (i, name) in names.iter().enumerate() {
            if set & (1 << i) != 0 {
                json.push_str(&format!("\"{name}\":true,"));
            }
        }
        json.push_str("\"key\":\"OBS_KEY_Z\"}");
    }
    json.push(']');
    m.assert_load_saves(h, &json, &json);
}

// ---------------------------------------------------------------------------
// Keys

/// Every key name round-trips, MOUSE1 and MOUSE2 aside (rejected below),
/// and OBS_KEY_NONE, which alone is no binding (dropped below). The array is
/// built the way the codec writes it, so it must load and save to the same
/// bytes.
#[test]
fn every_key_name() {
    let (mut m, h) = setup();
    let mouse2 = key("OBS_KEY_MOUSE2");
    let mut json = String::from("[");
    let mut count = 0;
    for k in KEY_NONE..LAST {
        if k == KEY_NONE || k == OBS_KEY_MOUSE1 || k == mouse2 {
            continue;
        }
        let name = key_name(k).unwrap();
        assert!(!name.is_empty());
        assert_eq!(key_from_name(name), k);
        json.push_str(&format!(
            "{}{{\"{}key\":\"{}\"}}",
            if count != 0 { "," } else { "" },
            if k % 2 != 0 { "shift\":true,\"" } else { "" },
            name.to_str().unwrap()
        ));
        count += 1;
    }
    json.push(']');
    m.assert_load_saves(h, &json, &json);
}

/// An item with no modifiers and no known key is no binding: it could never
/// fire, so load_binding() drops it. An unknown, empty, missing or
/// non-string "key", {}, and the {"key":"OBS_KEY_NONE"} that older builds
/// saved for these, load as nothing and are not saved back.
#[test]
fn unknown_or_missing_key_dropped() {
    let (mut m, h) = setup();
    let none = "[]";
    m.assert_load_saves(h, "[{\"key\":\"OBS_KEY_NOT_A_KEY\"}]", none);
    m.assert_load_saves(h, "[{\"key\":\"obs_key_a\"}]", none);
    m.assert_load_saves(h, "[{\"key\":\" OBS_KEY_A\"}]", none);
    m.assert_load_saves(h, "[{\"key\":\"OBS_KEY_A \"}]", none);
    m.assert_load_saves(h, "[{\"key\":\"A\"}]", none);
    m.assert_load_saves(h, "[{\"key\":\"\"}]", none);
    m.assert_load_saves(h, "[{\"key\":65}]", none);
    m.assert_load_saves(h, "[{\"key\":true}]", none);
    m.assert_load_saves(h, "[{\"key\":null}]", none);
    m.assert_load_saves(h, "[{\"key\":{}}]", none);
    m.assert_load_saves(h, "[{\"key\":\"OBS_KEY_NONE\"}]", none);
    m.assert_load_saves(h, "[{}]", none);
    m.assert_load_saves(h, "[{\"unrelated\":1}]", none);
}

/// A modifier-only binding (key OBS_KEY_NONE) is a real binding: an unknown
/// or missing key with modifiers keeps the modifiers.
#[test]
fn modifiers_only() {
    let (mut m, h) = setup();
    m.assert_load_saves(
        h,
        "[{\"shift\":true}]",
        "[{\"shift\":true,\"key\":\"OBS_KEY_NONE\"}]",
    );
    m.assert_load_saves(
        h,
        "[{\"control\":true,\"alt\":true,\"key\":\"bogus\"}]",
        "[{\"control\":true,\"alt\":true,\"key\":\"OBS_KEY_NONE\"}]",
    );
    m.assert_load_saves(
        h,
        "[{\"command\":true,\"key\":\"OBS_KEY_NONE\"}]",
        "[{\"command\":true,\"key\":\"OBS_KEY_NONE\"}]",
    );
}

// ---------------------------------------------------------------------------
// Mouse buttons (ours): primary buttons are reserved for UI interaction.

#[test]
fn primary_mouse_buttons_rejected() {
    let (mut m, h) = setup();
    m.load(
        h,
        concat!(
            "[{\"key\":\"OBS_KEY_MOUSE1\"},",
            "{\"key\":\"OBS_KEY_A\"},",
            "{\"shift\":true,\"key\":\"OBS_KEY_MOUSE2\"},",
            "{\"shift\":true,\"control\":true,\"alt\":true,\"command\":true,\"key\":\"OBS_KEY_MOUSE1\"},",
            "{\"key\":\"OBS_KEY_MOUSE3\"},",
            "{\"control\":true,\"key\":\"OBS_KEY_MOUSE29\"}]"
        ),
    );
    m.assert_saved(
        h,
        concat!(
            "[{\"key\":\"OBS_KEY_A\"},",
            "{\"key\":\"OBS_KEY_MOUSE3\"},",
            "{\"control\":true,\"key\":\"OBS_KEY_MOUSE29\"}]"
        ),
    );
}

/// A load whose every item is rejected still replaces the old bindings.
#[test]
fn rejected_load_clears() {
    let (mut m, h) = setup();
    m.load(h, "[{\"key\":\"OBS_KEY_A\"}]");
    m.load(
        h,
        "[{\"key\":\"OBS_KEY_MOUSE1\"},{\"alt\":true,\"key\":\"OBS_KEY_MOUSE2\"}]",
    );
    m.assert_saved(h, "[]");
}

/// The same rule for API bindings (obs_hotkey_load_bindings).
#[test]
fn api_primary_mouse_buttons_rejected() {
    let (mut m, h) = setup();
    m.load_api(
        h,
        &[
            combo(0, OBS_KEY_MOUSE1),
            combo(INTERACT_SHIFT_KEY, key("OBS_KEY_MOUSE2")),
            combo(
                INTERACT_CONTROL_KEY | INTERACT_COMMAND_KEY,
                key("OBS_KEY_C"),
            ),
            combo(0, key("OBS_KEY_MOUSE3")),
        ],
    );
    m.assert_saved(
        h,
        "[{\"control\":true,\"command\":true,\"key\":\"OBS_KEY_C\"},{\"key\":\"OBS_KEY_MOUSE3\"}]",
    );
}

/// Only the four modifier bits are saved; other bits of an API binding are
/// dropped.
#[test]
fn api_other_modifier_bits_dropped() {
    let (mut m, h) = setup();
    m.load_api(
        h,
        &[
            combo(0xFFFF_FFFF, key("OBS_KEY_D")),
            combo(INTERACT_CAPS_KEY | INTERACT_MOUSE_LEFT, key("OBS_KEY_E")),
        ],
    );
    m.assert_saved(
        h,
        "[{\"shift\":true,\"control\":true,\"alt\":true,\"command\":true,\"key\":\"OBS_KEY_D\"},{\"key\":\"OBS_KEY_E\"}]",
    );
}

// ---------------------------------------------------------------------------
// Arrays

/// Duplicates are kept and the order is the load order.
#[test]
fn duplicates_and_order() {
    let (mut m, h) = setup();
    let json = concat!(
        "[{\"key\":\"OBS_KEY_A\"},{\"key\":\"OBS_KEY_A\"},{\"shift\":true,\"key\":\"OBS_KEY_B\"},",
        "{\"key\":\"OBS_KEY_A\"},{\"key\":\"OBS_KEY_NUM9\"},{\"key\":\"OBS_KEY_1\"},",
        "{\"shift\":true,\"key\":\"OBS_KEY_B\"}]"
    );
    m.assert_load_saves(h, json, json);
}

/// Items that are not objects are dropped by the parser before the codec;
/// the objects keep their order.
#[test]
fn non_object_items() {
    let (mut m, h) = setup();
    m.assert_load_saves(
        h,
        "[1,\"OBS_KEY_A\",null,true,[],{\"key\":\"OBS_KEY_G\"},[{\"key\":\"OBS_KEY_H\"}],{\"key\":\"OBS_KEY_I\"}]",
        "[{\"key\":\"OBS_KEY_G\"},{\"key\":\"OBS_KEY_I\"}]",
    );
}

/// A load replaces the hotkey's bindings, and other hotkeys keep theirs.
#[test]
fn load_replaces_only_its_hotkey() {
    let (mut m, h) = setup();
    let other = m.register("test.codec.other");
    m.load(other, "[{\"key\":\"OBS_KEY_O\"}]");
    m.load(h, "[{\"key\":\"OBS_KEY_A\"},{\"key\":\"OBS_KEY_B\"}]");
    m.load(h, "[{\"key\":\"OBS_KEY_C\"}]");
    m.assert_saved(h, "[{\"key\":\"OBS_KEY_C\"}]");
    m.assert_saved(other, "[{\"key\":\"OBS_KEY_O\"}]");
}

/// A hotkey with no bindings saves an empty array; an unknown id saves
/// nothing.
#[test]
fn empty_save() {
    let (m, h) = setup();
    m.assert_saved(h, "[]");
    assert_eq!(m.save(usize::MAX), None);
}

/// An empty (or missing) array clears the bindings and sends
/// hotkey_bindings_changed when it removed any, as
/// obs_hotkey_load_bindings() does; clearing a hotkey with no bindings
/// sends nothing. A non-empty array signals even when every item is
/// rejected.
#[test]
fn empty_array_clears_with_signal() {
    let (mut m, h) = setup();
    m.load(h, "[{\"key\":\"OBS_KEY_A\"}]");
    assert_eq!(m.changed[h], 1);

    m.load(h, "[]");
    m.assert_saved(h, "[]");
    assert_eq!(m.changed[h], 2);
    m.load(h, "[]");
    assert_eq!(m.changed[h], 2);

    m.load(h, "[{\"key\":\"OBS_KEY_A\"}]");
    assert_eq!(m.changed[h], 3);
    m.load_array(h, None);
    m.assert_saved(h, "[]");
    assert_eq!(m.changed[h], 4);
    m.load_array(h, None);
    assert_eq!(m.changed[h], 4);

    m.load(h, "[{\"key\":\"OBS_KEY_MOUSE1\"}]");
    assert_eq!(m.changed[h], 5);

    m.load(h, "[{\"key\":\"OBS_KEY_A\"}]");
    assert_eq!(m.changed[h], 6);
    m.load_api(h, &[]);
    m.assert_saved(h, "[]");
    assert_eq!(m.changed[h], 7);
    m.load_api(h, &[]);
    assert_eq!(m.changed[h], 7);
}

// ---------------------------------------------------------------------------
// Pairs and sources: the other public entry points into the codec.

#[test]
fn pair_round_trip() {
    let (mut m, _) = setup();
    // obs_hotkey_pair_register_frontend: two hotkeys.
    let on = m.register("test.codec.on");
    let off = m.register("test.codec.off");

    // obs_hotkey_pair_load
    m.load(
        on,
        "[{\"alt\":true,\"key\":\"OBS_KEY_1\"},{\"key\":\"OBS_KEY_MOUSE2\"}]",
    );
    m.load(off, "[{\"command\":true,\"key\":\"OBS_KEY_2\"},{}]");

    // obs_hotkey_pair_save
    assert_eq!(
        m.save(on).unwrap(),
        "[{\"alt\":true,\"key\":\"OBS_KEY_1\"}]"
    );
    assert_eq!(
        m.save(off).unwrap(),
        "[{\"command\":true,\"key\":\"OBS_KEY_2\"}]"
    );
}

/// A source made from `hotkeys_json` (two hotkeys, registered in order and
/// loaded from their arrays as `enum_load_bindings` does), saved as
/// `obs_hotkeys_save_source` writes it.
fn source_saved(hotkeys_json: &str) -> String {
    let data = json::parse(hotkeys_json);
    let mut m = Model::default();
    let names = ["codec.first", "codec.second"];
    let mut saved = Vec::new();
    for name in names {
        let id = m.register(name);
        if let Some(items) = data.get_array(name) {
            m.load_bindings(id, &items, false);
        }
    }
    for (id, name) in names.iter().enumerate() {
        saved.push(format!("\"{name}\":{}", m.save(id).unwrap()));
    }
    format!("{{{}}}", saved.join(","))
}

/// A source's hotkeys, as a scene collection stores them: an object of one
/// array per hotkey name, loaded when the source registers each hotkey and
/// saved in registration order. A name the source has no hotkey for is
/// dropped, and a missing name saves an empty array. A source made from the
/// saved data saves the same bytes.
#[test]
fn source_round_trip() {
    let expected = concat!(
        "{\"codec.first\":[{\"shift\":true,\"key\":\"OBS_KEY_F5\"},{\"key\":\"OBS_KEY_F6\"}],",
        "\"codec.second\":[{\"control\":true,\"command\":true,\"key\":\"OBS_KEY_NONE\"}]}"
    );

    let first = source_saved(concat!(
        "{\"codec.second\":[{\"command\":true,\"control\":true,\"key\":\"nope\"}],",
        "\"codec.unknown\":[{\"key\":\"OBS_KEY_U\"}],",
        "\"codec.first\":[{\"shift\":true,\"key\":\"OBS_KEY_F5\"},{\"key\":\"OBS_KEY_MOUSE1\"},{\"key\":\"OBS_KEY_F6\"}]}"
    ));
    assert_eq!(first, expected);

    let second = source_saved(&first);
    assert_eq!(second, expected);

    let partial = source_saved("{\"codec.second\":[{\"key\":\"OBS_KEY_S\"}]}");
    assert_eq!(
        partial,
        "{\"codec.first\":[],\"codec.second\":[{\"key\":\"OBS_KEY_S\"}]}"
    );
}

// ---------------------------------------------------------------------------
// Edge cases of the safe API

const ALL: u32 =
    INTERACT_SHIFT_KEY | INTERACT_CONTROL_KEY | INTERACT_ALT_KEY | INTERACT_COMMAND_KEY;

/// The modifier set of a 4-bit index, in MODIFIERS order.
fn set_of(index: u32) -> [bool; 4] {
    [0, 1, 2, 3].map(|i| index & (1 << i) != 0)
}

#[test]
fn modifiers_table_matches_c() {
    let names: Vec<&str> = MODIFIERS.iter().map(|m| m.name.to_str().unwrap()).collect();
    assert_eq!(names, ["shift", "control", "alt", "command"]);
    let flags: Vec<u32> = MODIFIERS.iter().map(|m| m.flag).collect();
    assert_eq!(
        flags,
        [
            INTERACT_SHIFT_KEY,
            INTERACT_CONTROL_KEY,
            INTERACT_ALT_KEY,
            INTERACT_COMMAND_KEY
        ]
    );
    assert_eq!(KEY_FIELD, c"key");
}

#[test]
fn from_fields_every_modifier_set() {
    let a = key("OBS_KEY_A");
    for index in 0..16 {
        let mut expected = 0;
        for (i, m) in MODIFIERS.iter().enumerate() {
            if index & (1 << i) != 0 {
                expected |= m.flag;
            }
        }
        assert_eq!(
            from_fields_ok(set_of(index), c"OBS_KEY_A"),
            combo(expected, a),
            "set {index}"
        );
    }
}

fn from_fields_ok(modifiers: [bool; 4], name: &CStr) -> KeyCombination {
    combo_from_fields(modifiers, name).expect("a binding with a key or a modifier")
}

/// No known key: dropped without modifiers, a modifier-only binding (key
/// OBS_KEY_NONE) with any, whatever the name.
#[test]
fn from_fields_drops_only_empty() {
    let long = CString::new(vec![b'A'; 4096]).unwrap();
    let names: [&CStr; 7] = [
        c"",
        c"OBS_KEY_NONE",
        c"bogus",
        c"OBS_KEY_\xff",
        c"\xc3\x28",
        c"OBS_KEY_A\x01",
        &long,
    ];
    for name in names {
        assert_eq!(combo_from_fields([false; 4], name), None, "{name:?}");
        for index in 1..16 {
            let c = from_fields_ok(set_of(index), name);
            assert_eq!(c.key, KEY_NONE, "{name:?}");
        }
    }
}

#[test]
fn from_fields_every_key_name() {
    for k in KEY_NONE..LAST {
        let name = key_name(k).unwrap();
        // The first key with a name wins, as in obs_key_from_name.
        // OBS_KEY_NONE alone is no binding.
        if key_from_name(name) == KEY_NONE {
            assert_eq!(combo_from_fields([false; 4], name), None);
        } else {
            assert_eq!(from_fields_ok([false; 4], name).key, key_from_name(name));
        }
        assert_eq!(
            from_fields_ok([true; 4], name),
            combo(ALL, key_from_name(name))
        );
    }
}

#[test]
fn bindable_rejects_only_primary_mouse_buttons() {
    let mouse2 = key("OBS_KEY_MOUSE2");
    assert_eq!(mouse2, OBS_KEY_MOUSE1 + 1);
    for modifiers in [0, INTERACT_SHIFT_KEY, ALL, 0xFFFF_FFFF, INTERACT_CAPS_KEY] {
        for k in [i32::MIN, -1, KEY_NONE, LAST, LAST + 1, i32::MAX]
            .into_iter()
            .chain(KEY_NONE..LAST)
        {
            let expected = k != OBS_KEY_MOUSE1 && k != mouse2;
            assert_eq!(combo_bindable(combo(modifiers, k)), expected, "{k}");
        }
    }
}

#[test]
fn to_fields_order_and_count() {
    let f1 = key("OBS_KEY_F1");
    for modifiers in [0u32, 1, 0xFFFF_FFFF, 0x8000_0000, 0x7FFF_FFFF, ALL, !ALL]
        .into_iter()
        .chain(0..=0x1FF)
    {
        let fields: Vec<Field> = combo_to_fields(combo(modifiers, f1)).collect();
        let expected: Vec<Field> = MODIFIERS
            .iter()
            .filter(|m| modifiers & m.flag == m.flag)
            .map(|m| Field::Modifier(m.name))
            .chain([Field::Key(f1)])
            .collect();
        assert_eq!(fields, expected, "{modifiers:#x}");
    }
}

#[test]
fn to_fields_any_key_code() {
    for k in [i32::MIN, -1, LAST, LAST + 1, i32::MAX]
        .into_iter()
        .chain(KEY_NONE..LAST)
    {
        let mut it = combo_to_fields(combo(ALL, k));
        for m in MODIFIERS {
            assert_eq!(it.next(), Some(Field::Modifier(m.name)));
        }
        assert_eq!(it.next(), Some(Field::Key(k)));
        // Fused: nothing after the key, however often asked.
        for _ in 0..3 {
            assert_eq!(it.next(), None);
        }
    }
}

/// save -> load gives the binding back for every known key and modifier
/// set; an unknown code comes back as OBS_KEY_NONE (its name is ""), and
/// as nothing when no modifier is set either.
#[test]
fn to_fields_then_from_fields() {
    for k in [-1, LAST, i32::MAX].into_iter().chain(KEY_NONE..LAST) {
        for index in 0..16u32 {
            let mut modifiers = 0;
            for (i, m) in MODIFIERS.iter().enumerate() {
                if index & (1 << i) != 0 {
                    modifiers |= m.flag;
                }
            }
            let c = combo(modifiers | INTERACT_CAPS_KEY, k);
            let mut set = [false; 4];
            let mut name: &CStr = c"";
            for f in combo_to_fields(c) {
                match f {
                    Field::Modifier(n) => {
                        let i = MODIFIERS.iter().position(|m| m.name == n).unwrap();
                        set[i] = true;
                    }
                    Field::Key(code) => name = key_name(code).unwrap_or(c""),
                }
            }
            let back = combo_from_fields(set, name);
            let expected_key = if key_name(k).is_some() {
                key_from_name(key_name(k).unwrap())
            } else {
                KEY_NONE
            };
            let expected = (modifiers != 0 || expected_key != KEY_NONE)
                .then(|| combo(modifiers, expected_key));
            assert_eq!(back, expected, "{k} {index}");
        }
    }
}
