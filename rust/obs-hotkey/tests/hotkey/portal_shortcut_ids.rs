//! Tier 1: the safe core of `libobs/obs-nix-portal-shortcut-ids.c.h`.
//!
//! The first fifteen tests mirror `test/cmocka/test_portal_shortcut_ids.c`
//! one to one, with the same names and the same cases, "characterized, not
//! endorsed" ones included. The C test's teardown checks that every `dstr`
//! the codec allocates is freed; [`Counted`] is a [`Text`] that counts its
//! live buffers the way `bnum_allocs` counts `bmalloc`s, and [`checked`]
//! runs each test and then checks the count.
//!
//! The cases after those pin the edges of the safe API.

use std::cell::Cell;
use std::ffi::{CStr, CString};

use obs_hotkey::name_map::{
    KEY_NONE, OBS_KEY_LAST_VALUE, OBS_KEY_MOUSE1, OBS_KEY_MOUSE29, key_from_name, key_name,
};
use obs_hotkey::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
    KeyCombination,
};
use obs_hotkey::portal_shortcut_ids::{
    MODIFIER_NAMES, Text, bindable, shortcut_combo, shortcut_id, shortcut_trigger,
};

const LAST: i32 = OBS_KEY_LAST_VALUE as i32;

const ALL_MODIFIERS: u32 =
    INTERACT_CONTROL_KEY | INTERACT_ALT_KEY | INTERACT_SHIFT_KEY | INTERACT_COMMAND_KEY;

/// `INTERACT_CAPS_KEY` .. `INTERACT_IS_RIGHT` without the four known ones
/// (`obs-interaction.h`), which the codec ignores.
const OTHER_FLAGS: [u32; 8] = [1, 1 << 4, 1 << 5, 1 << 6, 1 << 8, 1 << 9, 1 << 10, 1 << 11];

thread_local! {
    /// Live [`Counted`] buffers on this thread (`bnum_allocs`).
    static LIVE: Cell<i64> = const { Cell::new(0) };
}

/// A `dstr`: no storage until something is appended, storage freed only by
/// [`Text::free`]. Dropping one with storage leaks it, as in C.
#[derive(Debug, Default)]
struct Counted(Option<Vec<u8>>);

impl Counted {
    fn of(s: &str) -> Self {
        let mut t = Self::default();
        t.cat(&CString::new(s).unwrap());
        t
    }

    fn bytes(&self) -> &[u8] {
        self.0.as_deref().unwrap_or_default()
    }

    fn text(&self) -> &str {
        std::str::from_utf8(self.bytes()).unwrap()
    }

    fn append(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.0
            .get_or_insert_with(|| {
                LIVE.with(|c| c.set(c.get() + 1));
                Vec::new()
            })
            .extend_from_slice(bytes);
    }
}

impl Text for Counted {
    fn free(&mut self) {
        if self.0.take().is_some() {
            LIVE.with(|c| c.set(c.get() - 1));
        }
    }
    fn cat(&mut self, s: &CStr) {
        self.append(s.to_bytes());
    }
    fn cat_text(&mut self, other: &Self) {
        self.append(other.bytes());
    }
    fn is_empty(&self) -> bool {
        self.bytes().first().is_none_or(|&b| b == 0)
    }
}

/// Runs `body`, then checks the codec freed what it allocated (the C
/// test's setup and teardown).
fn checked(body: impl FnOnce()) {
    let before = LIVE.with(Cell::get);
    body();
    assert_eq!(LIVE.with(Cell::get), before, "allocations before and after");
}

fn key(name: &str) -> i32 {
    let code = key_from_name(&CString::new(name).unwrap());
    assert_ne!(code, KEY_NONE, "{name:?} is a key");
    code
}

fn name_of(key: i32) -> &'static CStr {
    key_name(key).expect("a key")
}

fn combo_of(modifiers: u32, key: i32) -> KeyCombination {
    KeyCombination { modifiers, key }
}

fn is_mouse(key: i32) -> bool {
    (OBS_KEY_MOUSE1..=OBS_KEY_MOUSE29).contains(&key)
}

/// The expected prefix of a modifier set, in the codec's order.
fn expected_prefix(modifiers: u32, trigger: bool) -> String {
    let mut out = String::new();
    for m in MODIFIER_NAMES {
        if modifiers & m.modifier != 0 {
            out.push_str((if trigger { m.trigger } else { m.id }).to_str().unwrap());
            out.push('+');
        }
    }
    out
}

/// Modifier set `set` of the 16: bit `i` is `MODIFIER_NAMES[i]`.
fn modifier_set(set: u32) -> u32 {
    MODIFIER_NAMES
        .iter()
        .enumerate()
        .filter(|(i, _)| set & (1 << i) != 0)
        .fold(0, |acc, (_, m)| acc | m.modifier)
}

fn assert_id(combo: KeyCombination, expected: &str) {
    let mut id = Counted::default();
    assert!(shortcut_id(combo, &mut id), "{combo:?} has an id");
    assert_eq!(id.text(), expected, "{combo:?}");
    id.free();
}

fn assert_no_id(combo: KeyCombination) {
    let mut id = Counted::of("untouched");
    assert!(!shortcut_id(combo, &mut id), "{combo:?} has no id");
    assert_eq!(id.text(), "untouched");
    id.free();
}

fn assert_combo(id: &str, modifiers: u32, key: i32) {
    let back = shortcut_combo(&CString::new(id).unwrap());
    assert_eq!(back, Some(combo_of(modifiers, key)), "{id:?}");
}

fn assert_no_combo(id: &str) {
    let back = shortcut_combo(&CString::new(id).unwrap());
    assert_eq!(back, None, "{id:?}");
}

// --------------------------------------------------------------------------
// Key names for triggers

thread_local! {
    static NAME_CALLS: Cell<u32> = const { Cell::new(0) };
    static NAME_LAST_KEY: Cell<i32> = const { Cell::new(LAST) };
}

fn record(key: i32) {
    NAME_CALLS.with(|c| c.set(c.get() + 1));
    NAME_LAST_KEY.with(|c| c.set(key));
}

fn reset_name_calls() {
    NAME_CALLS.with(|c| c.set(0));
    NAME_LAST_KEY.with(|c| c.set(LAST));
}

fn name_calls() -> u32 {
    NAME_CALLS.with(Cell::get)
}

fn name_last_key() -> i32 {
    NAME_LAST_KEY.with(Cell::get)
}

type KeyNameFn = fn(i32, &mut Counted) -> bool;

/// What `test_portal_shortcuts` uses: the key name without `OBS_KEY_`,
/// single letters in lower case.
fn lower_name(key: i32, name: &mut Counted) -> bool {
    record(key);
    let full = key_name(key).map_or(&b""[..], CStr::to_bytes);
    let Some(rest) = full.strip_prefix(b"OBS_KEY_") else {
        return false;
    };
    if key == KEY_NONE {
        return false;
    }
    let mut rest = rest.to_vec();
    if rest.len() == 1 {
        rest.make_ascii_lowercase();
    }
    name.append(&rest);
    true
}

fn no_name(key: i32, _name: &mut Counted) -> bool {
    record(key);
    false
}

fn empty_name(key: i32, name: &mut Counted) -> bool {
    record(key);
    // dstr_copy(name, ""): frees and leaves it empty.
    name.free();
    true
}

fn unset_name(key: i32, _name: &mut Counted) -> bool {
    record(key);
    true
}

/// Fills the name, then says there is none: the codec frees it.
fn filled_but_no_name(key: i32, name: &mut Counted) -> bool {
    record(key);
    name.append(b"leaked?");
    false
}

/// Appends to whatever the name holds: shows the codec starts it empty.
fn appending_name(key: i32, name: &mut Counted) -> bool {
    record(key);
    name.append(b"K");
    true
}

fn assert_trigger(combo: KeyCombination, key_name: KeyNameFn, expected: &str) {
    let mut trigger = Counted::default();
    reset_name_calls();
    assert!(
        shortcut_trigger(combo, Some(key_name), &mut trigger),
        "{combo:?}"
    );
    assert_eq!(name_calls(), 1);
    assert_eq!(name_last_key(), combo.key);
    assert_eq!(trigger.text(), expected, "{combo:?}");
    trigger.free();
}

fn assert_no_trigger(combo: KeyCombination, key_name: Option<KeyNameFn>, calls: u32) {
    let mut trigger = Counted::of("untouched");
    reset_name_calls();
    assert!(
        !shortcut_trigger(combo, key_name, &mut trigger),
        "{combo:?}"
    );
    assert_eq!(name_calls(), calls);
    assert_eq!(trigger.text(), "untouched");
    trigger.free();
}

// --------------------------------------------------------------------------
// bindable

#[test]
fn test_bindable_keys() {
    checked(|| {
        assert!(!bindable(combo_of(0, KEY_NONE)));
        assert!(!bindable(combo_of(ALL_MODIFIERS, KEY_NONE)));
        assert!(!bindable(combo_of(0, LAST)));
        assert!(!bindable(combo_of(0, LAST + 1)));
        assert!(!bindable(combo_of(0, -1)));

        for key in KEY_NONE + 1..LAST {
            let expected = !is_mouse(key);
            assert_eq!(bindable(combo_of(0, key)), expected, "{key}");
            // Modifiers never matter.
            assert_eq!(bindable(combo_of(u32::MAX, key)), expected, "{key}");
        }

        // The mouse range ends exactly at MOUSE29: the key after it is a
        // keyboard key again.
        assert!(!bindable(combo_of(0, OBS_KEY_MOUSE1)));
        assert!(!bindable(combo_of(0, OBS_KEY_MOUSE29)));
        assert!(bindable(combo_of(0, OBS_KEY_MOUSE1 - 1)));
        assert!(bindable(combo_of(0, key("OBS_KEY_BACKSLASH_RT102"))));
        assert_eq!(key("OBS_KEY_BACKSLASH_RT102"), OBS_KEY_MOUSE29 + 1);
    });
}

// --------------------------------------------------------------------------
// Shortcut ids

#[test]
fn test_id_plain_keys() {
    checked(|| {
        for name in [
            "OBS_KEY_A",
            "OBS_KEY_Z",
            "OBS_KEY_0",
            "OBS_KEY_F1",
            "OBS_KEY_F13",
            "OBS_KEY_F35",
            "OBS_KEY_SPACE",
            "OBS_KEY_RETURN",
            "OBS_KEY_NUMPLUS",
            "OBS_KEY_PLUS",
            "OBS_KEY_BACKSLASH_RT102",
        ] {
            assert_id(combo_of(0, key(name)), name);
        }

        // Modifier keys are keys: alone they get an id like any other.
        assert_id(combo_of(0, key("OBS_KEY_SHIFT")), "OBS_KEY_SHIFT");
        assert_id(combo_of(0, key("OBS_KEY_CONTROL")), "OBS_KEY_CONTROL");
        assert_id(
            combo_of(INTERACT_CONTROL_KEY, key("OBS_KEY_CONTROL")),
            "ctrl+OBS_KEY_CONTROL",
        );
    });
}

#[test]
fn test_id_every_modifier_set() {
    checked(|| {
        let a = key("OBS_KEY_A");
        assert_id(combo_of(INTERACT_CONTROL_KEY, a), "ctrl+OBS_KEY_A");
        assert_id(combo_of(INTERACT_ALT_KEY, a), "alt+OBS_KEY_A");
        assert_id(combo_of(INTERACT_SHIFT_KEY, a), "shift+OBS_KEY_A");
        assert_id(combo_of(INTERACT_COMMAND_KEY, a), "super+OBS_KEY_A");
        assert_id(
            combo_of(INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY, a),
            "ctrl+shift+OBS_KEY_A",
        );
        assert_id(
            combo_of(INTERACT_ALT_KEY | INTERACT_COMMAND_KEY, key("OBS_KEY_F1")),
            "alt+super+OBS_KEY_F1",
        );
        assert_id(
            combo_of(ALL_MODIFIERS, key("OBS_KEY_SPACE")),
            "ctrl+alt+shift+super+OBS_KEY_SPACE",
        );

        // All 16 sets, always in the order ctrl, alt, shift, super.
        for set in 0..(1 << MODIFIER_NAMES.len()) {
            let modifiers = modifier_set(set);
            let expected = expected_prefix(modifiers, false) + "OBS_KEY_F13";
            assert_id(combo_of(modifiers, key("OBS_KEY_F13")), &expected);
        }
    });
}

/// Characterized, not endorsed: modifier bits other than the four known
/// ones are dropped from the id, so the id does not round-trip them and two
/// combinations that differ only in them share an id.
#[test]
fn test_id_ignores_other_modifier_bits() {
    checked(|| {
        let a = key("OBS_KEY_A");
        for flag in OTHER_FLAGS {
            assert_id(combo_of(flag, a), "OBS_KEY_A");
            assert_id(combo_of(flag | INTERACT_CONTROL_KEY, a), "ctrl+OBS_KEY_A");
        }
        assert_id(combo_of(u32::MAX, a), "ctrl+alt+shift+super+OBS_KEY_A");
    });
}

#[test]
fn test_id_unbindable() {
    checked(|| {
        // No key: modifier-only combinations stay focused-only.
        assert_no_id(combo_of(0, KEY_NONE));
        assert_no_id(combo_of(INTERACT_CONTROL_KEY, KEY_NONE));
        assert_no_id(combo_of(ALL_MODIFIERS, KEY_NONE));

        // Mouse buttons, with and without modifiers.
        for key in OBS_KEY_MOUSE1..=OBS_KEY_MOUSE29 {
            assert_no_id(combo_of(0, key));
            assert_no_id(combo_of(INTERACT_SHIFT_KEY, key));
        }

        // Out of range.
        assert_no_id(combo_of(0, LAST));
        assert_no_id(combo_of(0, LAST + 100));
        assert_no_id(combo_of(INTERACT_ALT_KEY, -1));
    });
}

#[test]
fn test_id_replaces_previous_contents() {
    checked(|| {
        let mut id = Counted::of("a much longer previous id that is replaced");
        assert!(shortcut_id(
            combo_of(INTERACT_SHIFT_KEY, key("OBS_KEY_B")),
            &mut id
        ));
        assert_eq!(id.text(), "shift+OBS_KEY_B");

        // The same text again.
        assert!(shortcut_id(combo_of(0, key("OBS_KEY_C")), &mut id));
        assert_eq!(id.text(), "OBS_KEY_C");
        id.free();
    });
}

// --------------------------------------------------------------------------
// Ids back to combinations

#[test]
fn test_combo_valid_ids() {
    checked(|| {
        let a = key("OBS_KEY_A");
        assert_combo("OBS_KEY_A", 0, a);
        assert_combo("OBS_KEY_F13", 0, key("OBS_KEY_F13"));
        assert_combo("OBS_KEY_PLUS", 0, key("OBS_KEY_PLUS"));
        assert_combo("OBS_KEY_BACKSLASH_RT102", 0, key("OBS_KEY_BACKSLASH_RT102"));
        assert_combo("ctrl+OBS_KEY_A", INTERACT_CONTROL_KEY, a);
        assert_combo("alt+OBS_KEY_A", INTERACT_ALT_KEY, a);
        assert_combo("shift+OBS_KEY_A", INTERACT_SHIFT_KEY, a);
        assert_combo("super+OBS_KEY_A", INTERACT_COMMAND_KEY, a);
        assert_combo(
            "ctrl+shift+OBS_KEY_A",
            INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY,
            a,
        );
        assert_combo(
            "ctrl+alt+shift+super+OBS_KEY_SPACE",
            ALL_MODIFIERS,
            key("OBS_KEY_SPACE"),
        );
    });
}

/// Characterized, not endorsed: the parser accepts more than the ids it
/// makes. Modifiers come in any order and may repeat; the result is the
/// same combination as the canonical id.
#[test]
fn test_combo_non_canonical_ids() {
    checked(|| {
        let a = key("OBS_KEY_A");
        assert_combo(
            "shift+ctrl+OBS_KEY_A",
            INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY,
            a,
        );
        assert_combo("super+shift+alt+ctrl+OBS_KEY_A", ALL_MODIFIERS, a);
        assert_combo("ctrl+ctrl+OBS_KEY_A", INTERACT_CONTROL_KEY, a);
        assert_combo(
            "alt+shift+alt+OBS_KEY_A",
            INTERACT_ALT_KEY | INTERACT_SHIFT_KEY,
            a,
        );

        // A modifier key named as the key.
        assert_combo(
            "ctrl+OBS_KEY_CONTROL",
            INTERACT_CONTROL_KEY,
            key("OBS_KEY_CONTROL"),
        );
    });
}

#[test]
fn test_combo_rejects() {
    checked(|| {
        for id in [
            // Empty, or modifiers with no key.
            "",
            "ctrl+",
            "ctrl",
            "ctrl+shift+",
            "+",
            "++",
            // Empty or unknown modifier segments.
            "+OBS_KEY_A",
            "ctrl++OBS_KEY_A",
            "hyper+OBS_KEY_A",
            "meta+OBS_KEY_A",
            "control+OBS_KEY_A",
            "logo+OBS_KEY_A",
            "cmd+OBS_KEY_A",
            "ctr+OBS_KEY_A",
            "ctrlx+OBS_KEY_A",
            // Matching is exact and case-sensitive: the trigger names and
            // other spellings are not ids.
            "CTRL+OBS_KEY_A",
            "Ctrl+OBS_KEY_A",
            "SHIFT+OBS_KEY_A",
            "LOGO+OBS_KEY_A",
            " ctrl+OBS_KEY_A",
            "ctrl +OBS_KEY_A",
            "obs_key_a",
            "OBS_KEY_a",
            "OBS_KEY_A ",
            " OBS_KEY_A",
            // Not an OBS key name.
            "A",
            "F13",
            "ctrl+a",
            "OBS_KEY_",
            "OBS_KEY",
            "OBS_KEY_NOT_A_KEY",
            "OBS_KEY_LAST_VALUE",
            "some-other-app-shortcut",
            "ctrl+shift+F13",
            // Something after the key.
            "OBS_KEY_A+",
            "OBS_KEY_A+ctrl",
            "ctrl+OBS_KEY_A+OBS_KEY_B",
            // Known names that are not bindable.
            "OBS_KEY_NONE",
            "ctrl+OBS_KEY_NONE",
            "OBS_KEY_MOUSE1",
            "OBS_KEY_MOUSE29",
            "shift+OBS_KEY_MOUSE3",
        ] {
            assert_no_combo(id);
        }
    });
}

/// Every bindable key, with no modifiers, each one and all four, makes an
/// id that parses back to the same combination, and every key name parses
/// to its key.
#[test]
fn test_round_trip_every_key() {
    checked(|| {
        let sets = [
            0,
            INTERACT_CONTROL_KEY,
            INTERACT_ALT_KEY,
            INTERACT_SHIFT_KEY,
            INTERACT_COMMAND_KEY,
            INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY,
            ALL_MODIFIERS,
        ];
        let mut bindable_keys = 0;

        for key in KEY_NONE + 1..LAST {
            let name = name_of(key).to_str().unwrap();
            if is_mouse(key) {
                assert_no_id(combo_of(0, key));
                assert_no_combo(name);
                continue;
            }
            bindable_keys += 1;

            for &modifiers in &sets {
                let expected = expected_prefix(modifiers, false) + name;
                let mut id = Counted::default();
                assert!(shortcut_id(combo_of(modifiers, key), &mut id), "{name}");
                assert_eq!(id.text(), expected);
                let back = shortcut_combo(&CString::new(id.bytes()).unwrap());
                assert_eq!(back, Some(combo_of(modifiers, key)), "{expected}");
                id.free();
            }
        }

        // Every key in obs-hotkeys.h but OBS_KEY_NONE and the 29 mouse
        // buttons.
        assert_eq!(bindable_keys, LAST - 1 - 29);
    });
}

// --------------------------------------------------------------------------
// Preferred triggers

#[test]
fn test_trigger_plain_and_modifiers() {
    checked(|| {
        let a = key("OBS_KEY_A");
        let f13 = key("OBS_KEY_F13");
        assert_trigger(combo_of(0, a), lower_name, "a");
        assert_trigger(combo_of(0, f13), lower_name, "F13");
        assert_trigger(combo_of(0, key("OBS_KEY_SPACE")), lower_name, "SPACE");
        assert_trigger(combo_of(INTERACT_CONTROL_KEY, a), lower_name, "CTRL+a");
        assert_trigger(combo_of(INTERACT_ALT_KEY, a), lower_name, "ALT+a");
        assert_trigger(combo_of(INTERACT_SHIFT_KEY, a), lower_name, "SHIFT+a");
        assert_trigger(combo_of(INTERACT_COMMAND_KEY, a), lower_name, "LOGO+a");
        assert_trigger(
            combo_of(INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY, a),
            lower_name,
            "CTRL+SHIFT+a",
        );
        assert_trigger(
            combo_of(INTERACT_ALT_KEY | INTERACT_COMMAND_KEY, f13),
            lower_name,
            "ALT+LOGO+F13",
        );
        assert_trigger(
            combo_of(ALL_MODIFIERS, key("OBS_KEY_F1")),
            lower_name,
            "CTRL+ALT+SHIFT+LOGO+F1",
        );

        // All 16 sets, always in the order CTRL, ALT, SHIFT, LOGO.
        for set in 0..(1 << MODIFIER_NAMES.len()) {
            let modifiers = modifier_set(set);
            let expected = expected_prefix(modifiers, true) + "F13";
            assert_trigger(combo_of(modifiers, f13), lower_name, &expected);
        }

        // The key name is used as given, whatever it holds.
        assert_trigger(
            combo_of(INTERACT_SHIFT_KEY, key("OBS_KEY_B")),
            appending_name,
            "SHIFT+K",
        );
    });
}

/// Characterized, not endorsed: as for ids, other modifier bits are
/// dropped.
#[test]
fn test_trigger_ignores_other_modifier_bits() {
    checked(|| {
        let a = key("OBS_KEY_A");
        for flag in OTHER_FLAGS {
            assert_trigger(combo_of(flag | INTERACT_ALT_KEY, a), lower_name, "ALT+a");
        }
        assert_trigger(combo_of(u32::MAX, a), lower_name, "CTRL+ALT+SHIFT+LOGO+a");
    });
}

#[test]
fn test_trigger_without_name() {
    checked(|| {
        let a = key("OBS_KEY_A");
        let f13 = key("OBS_KEY_F13");

        // No key name function.
        assert_no_trigger(combo_of(INTERACT_CONTROL_KEY, a), None, 0);

        // The function has no name, an empty one, or leaves it unset.
        assert_no_trigger(combo_of(INTERACT_CONTROL_KEY, a), Some(no_name), 1);
        assert_eq!(name_last_key(), a);
        assert_no_trigger(combo_of(0, f13), Some(empty_name), 1);
        assert_no_trigger(combo_of(0, f13), Some(unset_name), 1);

        // A name filled in before returning false is freed (`checked`
        // checks the allocation count).
        assert_no_trigger(
            combo_of(INTERACT_SHIFT_KEY, key("OBS_KEY_B")),
            Some(filled_but_no_name),
            1,
        );
    });
}

#[test]
fn test_trigger_unbindable() {
    checked(|| {
        // The key name function is not called for an unbindable
        // combination.
        let lower: Option<KeyNameFn> = Some(lower_name);
        assert_no_trigger(combo_of(0, KEY_NONE), lower, 0);
        assert_no_trigger(combo_of(ALL_MODIFIERS, KEY_NONE), lower, 0);
        assert_no_trigger(combo_of(0, OBS_KEY_MOUSE1), lower, 0);
        assert_no_trigger(combo_of(INTERACT_CONTROL_KEY, OBS_KEY_MOUSE29), lower, 0);
        assert_no_trigger(combo_of(0, LAST), lower, 0);
        assert_no_trigger(combo_of(0, -1), lower, 0);
        assert_no_trigger(combo_of(0, OBS_KEY_MOUSE1), None, 0);
    });
}

#[test]
fn test_trigger_replaces_previous_contents() {
    checked(|| {
        let mut trigger = Counted::of("a much longer previous trigger that is replaced");
        assert!(shortcut_trigger(
            combo_of(INTERACT_ALT_KEY, key("OBS_KEY_C")),
            Some(lower_name),
            &mut trigger
        ));
        assert_eq!(trigger.text(), "ALT+c");

        assert!(shortcut_trigger(
            combo_of(0, key("OBS_KEY_F2")),
            Some(lower_name),
            &mut trigger
        ));
        assert_eq!(trigger.text(), "F2");
        trigger.free();
    });
}

// --------------------------------------------------------------------------
// Edges of the safe API

#[test]
fn modifier_names_are_the_c_table() {
    let rows: Vec<_> = MODIFIER_NAMES
        .iter()
        .map(|m| {
            (
                m.modifier,
                m.id.to_str().unwrap(),
                m.trigger.to_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (INTERACT_CONTROL_KEY, "ctrl", "CTRL"),
            (INTERACT_ALT_KEY, "alt", "ALT"),
            (INTERACT_SHIFT_KEY, "shift", "SHIFT"),
            (INTERACT_COMMAND_KEY, "super", "LOGO"),
        ]
    );
}

#[test]
fn bindable_extremes() {
    assert!(!bindable(combo_of(0, i32::MIN)));
    assert!(!bindable(combo_of(0, i32::MAX)));
    assert!(bindable(combo_of(0, KEY_NONE + 1)));
    assert!(bindable(combo_of(0, LAST - 1)));
}

/// `Vec<u8>` is a `Text` too: `dstr_free` clears it, `dstr_is_empty` is
/// true for no bytes or a leading NUL.
#[test]
fn vec_text() {
    let mut v = b"old".to_vec();
    assert!(shortcut_id(
        combo_of(INTERACT_ALT_KEY, key("OBS_KEY_F5")),
        &mut v
    ));
    assert_eq!(v, b"alt+OBS_KEY_F5");
    v.free();
    assert!(Text::is_empty(&v));
    assert!(Text::is_empty(&vec![0u8, b'x']));
    assert!(!Text::is_empty(&b"x\0".to_vec()));

    let mut t = Vec::new();
    let name = |_key: i32, n: &mut Vec<u8>| {
        n.extend_from_slice(b"F5");
        true
    };
    assert!(shortcut_trigger(
        combo_of(ALL_MODIFIERS, key("OBS_KEY_F5")),
        Some(name),
        &mut t
    ));
    assert_eq!(t, b"CTRL+ALT+SHIFT+LOGO+F5");
}

/// A name with bytes after a NUL is copied whole (`dstr_cat_dstr` copies
/// `len` bytes); one that starts with a NUL is empty.
#[test]
fn trigger_name_bytes() {
    let f5 = key("OBS_KEY_F5");
    let mut t = Vec::new();
    let nul_inside = |_key: i32, n: &mut Vec<u8>| {
        n.extend_from_slice(b"a\0b");
        true
    };
    assert!(shortcut_trigger(combo_of(0, f5), Some(nul_inside), &mut t));
    assert_eq!(t, b"a\0b");

    let mut t = b"kept".to_vec();
    let nul_first = |_key: i32, n: &mut Vec<u8>| {
        n.extend_from_slice(b"\0b");
        true
    };
    assert!(!shortcut_trigger(combo_of(0, f5), Some(nul_first), &mut t));
    assert_eq!(t, b"kept");
}

/// The key part is looked up whole: a key name with a `+` in it cannot be
/// an id, and the last `+` is not special.
#[test]
fn combo_splits_on_every_plus() {
    assert_no_combo("ctrl+OBS_KEY_A+");
    assert_no_combo("OBS_KEY_PLUS+");
    assert_combo(
        "super+super+super+OBS_KEY_PLUS",
        INTERACT_COMMAND_KEY,
        key("OBS_KEY_PLUS"),
    );
    // A segment equal to a known name but longer or shorter is unknown.
    assert_no_combo("supe+OBS_KEY_A");
    assert_no_combo("superr+OBS_KEY_A");
}
