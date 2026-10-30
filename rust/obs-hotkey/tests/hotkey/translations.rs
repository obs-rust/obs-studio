//! Tier 1: the safe core of `libobs/obs-hotkey-translations.c.h`.
//!
//! The first sixteen tests mirror `test/cmocka/test_hotkey_translations.c`
//! one to one, with the same names, in the same order, with the same
//! strings and the same expected names. The C test goes through the public
//! API of a real libobs on Linux: `obs_hotkeys_set_translations_s()` stores
//! the names, and `obs_key_to_str()` and `obs_key_combination_to_str()`
//! read them back, with the X11 English names for keys that have none.
//! Here [`Store`] repeats what that C does around the translations: the
//! shim's `min(size, sizeof)` copy of the struct (a `NULL` struct stores
//! nothing; a cut at a member boundary leaves the members after it `NULL`),
//! the store, `obs_nix_x11_key_to_str()` with its defaults, and
//! `obs_key_combination_to_str()`. The C test is Linux and FreeBSD only, so
//! these cases use [`Keypad::Standard`].
//!
//! The C test runs its cases in order against one store for the whole
//! process; each case here starts from an empty store, which is the same,
//! as every case sets what it checks first.
//!
//! The cases after those pin the edges of the safe API.

use std::collections::HashMap;
use std::ffi::{CString, c_char};

use obs_hotkey::name_map::{
    KEY_NONE, OBS_KEY_ALT, OBS_KEY_CONTROL, OBS_KEY_META, OBS_KEY_SHIFT, key_from_name,
};
use obs_hotkey::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
};
use obs_hotkey::translations::{
    Keypad, MEMBER_COUNT, MOUSE_KEYS, Member, NUMPAD_KEYS, Translations, depad, expand,
    for_each_store, has_placeholder, is_padding, named_keys, numpad_template, prepare_template,
    replace_placeholder,
};
use proptest::prelude::*;

/// The size of one member, a `const char *`.
const PTR: usize = size_of::<*const c_char>();
/// `sizeof(struct obs_hotkeys_translations)`.
const STRUCT_SIZE: usize = MEMBER_COUNT * PTR;

/// `offsetof(struct obs_hotkeys_translations, member)`.
fn offset_of(member: Member) -> usize {
    member.index() * PTR
}

/// The key code named `name`.
fn key(name: &str) -> i32 {
    let k = key_from_name(&CString::new(name).unwrap());
    assert_ne!(k, KEY_NONE, "{name} is a key");
    k
}

fn cs(bytes: impl Into<Vec<u8>>) -> CString {
    CString::new(bytes).unwrap()
}

fn show(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The names `obs_nix_x11_key_to_str()` shows for a key with no stored
/// translation, for the keys these tests read.
fn x11_default(key: i32) -> Vec<u8> {
    for (code, digits) in NUMPAD_KEYS {
        if code == key {
            return format!("Numpad {digits}").into_bytes();
        }
    }
    for (code, digits) in MOUSE_KEYS {
        if code == key {
            return format!("Mouse {digits}").into_bytes();
        }
    }
    let defaults = [
        ("OBS_KEY_INSERT", "Insert"),
        ("OBS_KEY_DELETE", "Delete"),
        ("OBS_KEY_HOME", "Home"),
        ("OBS_KEY_END", "End"),
        ("OBS_KEY_PAGEUP", "Page Up"),
        ("OBS_KEY_PAGEDOWN", "Page Down"),
        ("OBS_KEY_NUMLOCK", "Num Lock"),
        ("OBS_KEY_SCROLLLOCK", "Scroll Lock"),
        ("OBS_KEY_CAPSLOCK", "Caps Lock"),
        ("OBS_KEY_BACKSPACE", "Backspace"),
        ("OBS_KEY_TAB", "Tab"),
        ("OBS_KEY_PRINT", "Print"),
        ("OBS_KEY_PAUSE", "Pause"),
        ("OBS_KEY_LEFT", "Left"),
        ("OBS_KEY_RIGHT", "Right"),
        ("OBS_KEY_UP", "Up"),
        ("OBS_KEY_DOWN", "Down"),
        ("OBS_KEY_SHIFT", "Shift"),
        ("OBS_KEY_ALT", "Alt"),
        ("OBS_KEY_CONTROL", "Control"),
        ("OBS_KEY_META", "Super"),
        ("OBS_KEY_MENU", "Menu"),
        ("OBS_KEY_NUMASTERISK", "Numpad *"),
        ("OBS_KEY_NUMPLUS", "Numpad +"),
        ("OBS_KEY_NUMMINUS", "Numpad -"),
        ("OBS_KEY_NUMPERIOD", "Numpad ."),
        ("OBS_KEY_NUMSLASH", "Numpad /"),
        ("OBS_KEY_SPACE", "Space"),
        ("OBS_KEY_ESCAPE", "Escape"),
    ];
    for (name, default) in defaults {
        if self::key(name) == key {
            return default.as_bytes().to_vec();
        }
    }
    panic!("no X11 default modelled for key {key}");
}

/// `obs->hotkeys.translations[]` and the C around it.
#[derive(Default)]
struct Store {
    names: HashMap<i32, Vec<u8>>,
    calls: usize,
}

impl Store {
    /// `obs_hotkeys_set_translations(t)`: the whole struct.
    fn set(&mut self, t: &Translations<'_>) {
        self.set_s(Some(t), STRUCT_SIZE);
    }

    /// `obs_hotkeys_set_translations_s(t, size)`. The shim returns on a
    /// `NULL` struct and copies `min(size, sizeof)` bytes into a zeroed
    /// one, so the members past a cut read as `NULL` (these tests cut only
    /// at member boundaries).
    fn set_s(&mut self, t: Option<&Translations<'_>>, size: usize) {
        let Some(t) = t else { return };
        let copied = size.min(STRUCT_SIZE);
        let mut cut = Translations::new();
        for m in Member::ALL {
            if offset_of(m) + PTR <= copied {
                cut.set(m, t.get(m));
            }
        }
        for_each_store(&cut, Keypad::Standard, |key, name| {
            self.calls += 1;
            self.names.insert(key, name.to_bytes().to_vec());
        });
    }

    /// `obs_key_to_str()` on X11.
    fn key_to_str(&self, key: i32) -> Vec<u8> {
        self.names
            .get(&key)
            .cloned()
            .unwrap_or_else(|| x11_default(key))
    }

    /// `obs_key_combination_to_str()` (`obs-nix.c`): control, meta, alt,
    /// shift, then the key, each left out when its name is empty, joined
    /// by `" + "`.
    fn combination_to_str(&self, modifiers: u32, key: i32) -> Vec<u8> {
        let mut parts = Vec::new();
        for (bit, code) in [
            (INTERACT_CONTROL_KEY, OBS_KEY_CONTROL),
            (INTERACT_COMMAND_KEY, OBS_KEY_META),
            (INTERACT_ALT_KEY, OBS_KEY_ALT),
            (INTERACT_SHIFT_KEY, OBS_KEY_SHIFT),
        ] {
            if modifiers & bit != 0 {
                parts.push(self.key_to_str(code));
            }
        }
        if key != KEY_NONE {
            parts.push(self.key_to_str(key));
        }
        parts.retain(|p| !p.is_empty());
        parts.join(&b" + "[..])
    }

    #[track_caller]
    fn assert_key(&self, key_name: &str, expected: impl AsRef<[u8]>) {
        let got = self.key_to_str(key(key_name));
        let expected = expected.as_ref();
        assert_eq!(
            got,
            expected,
            "{key_name}: got {:?}, expected {:?}",
            show(&got),
            show(expected)
        );
    }
}

/// `struct filled` of the C test: every member set, to "<prefix> <member>"
/// (the templates to "<prefix> <member> %1" and "<prefix> mouse %1").
struct Filled {
    bufs: Vec<CString>,
}

impl Filled {
    fn new(prefix: &str) -> Self {
        let bufs = Member::ALL
            .iter()
            .map(|m| match m {
                Member::NumpadNum | Member::AppleKeypadNum => {
                    cs(format!("{prefix} {} %1", m.name()))
                }
                Member::MouseNum => cs(format!("{prefix} mouse %1")),
                _ => cs(format!("{prefix} {}", m.name())),
            })
            .collect();
        Self { bufs }
    }

    fn t(&self) -> Translations<'_> {
        let mut t = Translations::new();
        for (m, s) in Member::ALL.iter().zip(&self.bufs) {
            t.set(*m, Some(s));
        }
        t
    }
}

/// `named_keys[]` of the C test: the named keys the frontend translates on
/// Linux, with the member that sets each, the arrow keys left out.
const NAMED_KEYS: [(&str, &str); 25] = [
    ("OBS_KEY_INSERT", "insert"),
    ("OBS_KEY_DELETE", "del"),
    ("OBS_KEY_HOME", "home"),
    ("OBS_KEY_END", "end"),
    ("OBS_KEY_PAGEUP", "page_up"),
    ("OBS_KEY_PAGEDOWN", "page_down"),
    ("OBS_KEY_NUMLOCK", "num_lock"),
    ("OBS_KEY_SCROLLLOCK", "scroll_lock"),
    ("OBS_KEY_CAPSLOCK", "caps_lock"),
    ("OBS_KEY_BACKSPACE", "backspace"),
    ("OBS_KEY_TAB", "tab"),
    ("OBS_KEY_PRINT", "print"),
    ("OBS_KEY_PAUSE", "pause"),
    ("OBS_KEY_SHIFT", "shift"),
    ("OBS_KEY_ALT", "alt"),
    ("OBS_KEY_CONTROL", "control"),
    ("OBS_KEY_META", "meta"),
    ("OBS_KEY_MENU", "menu"),
    ("OBS_KEY_SPACE", "space"),
    ("OBS_KEY_ESCAPE", "escape"),
    ("OBS_KEY_NUMSLASH", "numpad_divide"),
    ("OBS_KEY_NUMASTERISK", "numpad_multiply"),
    ("OBS_KEY_NUMMINUS", "numpad_minus"),
    ("OBS_KEY_NUMPLUS", "numpad_plus"),
    ("OBS_KEY_NUMPERIOD", "numpad_decimal"),
];

fn assert_named_keys(s: &Store, prefix: &str) {
    for (k, member) in NAMED_KEYS {
        s.assert_key(k, format!("{prefix} {member}"));
    }
}

fn assert_numbered_keys(s: &Store, prefix: &str) {
    for i in 0..=9 {
        s.assert_key(
            &format!("OBS_KEY_NUM{i}"),
            format!("{prefix} numpad_num {i}"),
        );
    }
    for i in 1..=29 {
        s.assert_key(&format!("OBS_KEY_MOUSE{i}"), format!("{prefix} mouse {i}"));
    }
}

fn assert_numpad_template(tmpl: &[u8], num0: &[u8], num5: &[u8], num9: &[u8]) {
    let mut s = Store::default();
    let tmpl = cs(tmpl);
    s.set(&Translations::new().with(Member::NumpadNum, &tmpl));
    s.assert_key("OBS_KEY_NUM0", num0);
    s.assert_key("OBS_KEY_NUM5", num5);
    s.assert_key("OBS_KEY_NUM9", num9);
}

fn assert_mouse_template(tmpl: &[u8], mouse1: &[u8], mouse10: &[u8], mouse29: &[u8]) {
    let mut s = Store::default();
    let tmpl = cs(tmpl);
    s.set(&Translations::new().with(Member::MouseNum, &tmpl));
    s.assert_key("OBS_KEY_MOUSE1", mouse1);
    s.assert_key("OBS_KEY_MOUSE10", mouse10);
    s.assert_key("OBS_KEY_MOUSE29", mouse29);
}

// ---------------------------------------------------------------------------
// The C test, one to one.

/// Before any translation is set, the platform's English names.
#[test]
fn defaults_before_any_set() {
    let s = Store::default();
    s.assert_key("OBS_KEY_INSERT", "Insert");
    s.assert_key("OBS_KEY_PAGEUP", "Page Up");
    s.assert_key("OBS_KEY_LEFT", "Left");
    s.assert_key("OBS_KEY_META", "Super");
    s.assert_key("OBS_KEY_ESCAPE", "Escape");
    s.assert_key("OBS_KEY_NUMSLASH", "Numpad /");
    s.assert_key("OBS_KEY_NUM0", "Numpad 0");
    s.assert_key("OBS_KEY_NUM9", "Numpad 9");
    s.assert_key("OBS_KEY_MOUSE1", "Mouse 1");
    s.assert_key("OBS_KEY_MOUSE29", "Mouse 29");
}

/// An empty or padding-only template, before any other set: skipped like a
/// NULL one, so NUM0-NUM9 and MOUSE1-MOUSE29 keep the English defaults.
#[test]
fn empty_templates_keep_defaults() {
    let mut s = Store::default();
    for empty in [&b""[..], b" ", b" \t\r\n "] {
        let e = cs(empty);
        s.set(
            &Translations::new()
                .with(Member::NumpadNum, &e)
                .with(Member::MouseNum, &e),
        );
        s.assert_key("OBS_KEY_NUM0", "Numpad 0");
        s.assert_key("OBS_KEY_NUM9", "Numpad 9");
        s.assert_key("OBS_KEY_MOUSE1", "Mouse 1");
        s.assert_key("OBS_KEY_MOUSE29", "Mouse 29");
    }
    assert_eq!(s.calls, 0, "nothing stored");
}

/// Every member set: each named key gets its name, NUM0-NUM9 and
/// MOUSE1-MOUSE29 get the expanded templates.
#[test]
fn every_member() {
    let mut s = Store::default();
    let f = Filled::new("A");
    s.set(&f.t());
    assert_named_keys(&s, "A");
    assert_numbered_keys(&s, "A");
}

/// A second set replaces every name it sets.
#[test]
fn set_twice_replaces() {
    let mut s = Store::default();
    let f1 = Filled::new("B1");
    s.set(&f1.t());
    let f2 = Filled::new("B2");
    s.set(&f2.t());
    assert_named_keys(&s, "B2");
    assert_numbered_keys(&s, "B2");
}

/// A NULL member leaves that key's name as it was: a set never clears one.
#[test]
fn null_member_keeps_name() {
    let mut s = Store::default();
    let f = Filled::new("C");
    s.set(&f.t());

    let del = cs("C2 del");
    s.set(&Translations::new().with(Member::Del, &del));
    s.assert_key("OBS_KEY_DELETE", "C2 del");
    s.assert_key("OBS_KEY_INSERT", "C insert");
    s.assert_key("OBS_KEY_ESCAPE", "C escape");
    s.assert_key("OBS_KEY_NUMSLASH", "C numpad_divide");
    s.assert_key("OBS_KEY_NUM3", "C numpad_num 3");
    s.assert_key("OBS_KEY_MOUSE3", "C mouse 3");

    // All NULL changes nothing.
    s.set(&Translations::new());
    s.assert_key("OBS_KEY_DELETE", "C2 del");
    s.assert_key("OBS_KEY_NUM3", "C numpad_num 3");
    s.assert_key("OBS_KEY_MOUSE3", "C mouse 3");
}

/// A NULL struct changes nothing.
#[test]
fn null_struct() {
    let mut s = Store::default();
    let f = Filled::new("D");
    s.set(&f.t());
    s.set_s(None, STRUCT_SIZE);
    s.set_s(None, 0);
    assert_named_keys(&s, "D");
    assert_numbered_keys(&s, "D");
}

/// Only min(size, sizeof) bytes are read, so a struct from an older,
/// shorter header reads as NULL tails, which keep their names; a size of 0
/// reads nothing.
#[test]
fn short_size() {
    let mut s = Store::default();
    let e1 = Filled::new("E1");
    s.set(&e1.t());

    let e2 = Filled::new("E2");
    let t = e2.t();
    s.set_s(Some(&t), 0);
    assert_named_keys(&s, "E1");
    assert_numbered_keys(&s, "E1");

    // insert .. pause
    s.set_s(Some(&t), offset_of(Member::Left));
    s.assert_key("OBS_KEY_INSERT", "E2 insert");
    s.assert_key("OBS_KEY_PAUSE", "E2 pause");
    s.assert_key("OBS_KEY_SHIFT", "E1 shift");
    s.assert_key("OBS_KEY_ESCAPE", "E1 escape");
    s.assert_key("OBS_KEY_NUMSLASH", "E1 numpad_divide");
    s.assert_key("OBS_KEY_NUM1", "E1 numpad_num 1");
    s.assert_key("OBS_KEY_MOUSE1", "E1 mouse 1");

    // .. apple_keypad_equal: the numpad, not the mouse or escape
    s.set_s(Some(&t), offset_of(Member::MouseNum));
    s.assert_key("OBS_KEY_SHIFT", "E2 shift");
    s.assert_key("OBS_KEY_NUMSLASH", "E2 numpad_divide");
    s.assert_key("OBS_KEY_NUM1", "E2 numpad_num 1");
    s.assert_key("OBS_KEY_MOUSE1", "E1 mouse 1");
    s.assert_key("OBS_KEY_ESCAPE", "E1 escape");

    // .. mouse_num: everything but escape, the last member
    s.set_s(Some(&t), offset_of(Member::Escape));
    s.assert_key("OBS_KEY_MOUSE1", "E2 mouse 1");
    s.assert_key("OBS_KEY_ESCAPE", "E1 escape");
}

/// A size larger than the struct reads only the struct.
#[test]
fn larger_size() {
    let mut s = Store::default();
    let f = Filled::new("F");
    s.set_s(Some(&f.t()), STRUCT_SIZE + 64);
    assert_named_keys(&s, "F");
    assert_numbered_keys(&s, "F");
}

/// An empty name is stored as is, so the key shows as nothing, and a
/// combination leaves it out.
#[test]
fn empty_named_key() {
    let mut s = Store::default();
    let (empty, ctl) = (cs(""), cs("Ctl"));
    let mut t = Translations::new()
        .with(Member::Insert, &empty)
        .with(Member::Shift, &empty)
        .with(Member::NumpadPlus, &empty)
        .with(Member::Control, &ctl);
    s.set(&t);
    s.assert_key("OBS_KEY_INSERT", "");
    s.assert_key("OBS_KEY_SHIFT", "");
    s.assert_key("OBS_KEY_NUMPLUS", "");

    assert_eq!(
        show(&s.combination_to_str(
            INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY,
            key("OBS_KEY_INSERT")
        )),
        "Ctl"
    );

    let (ins, shf, nplus) = (cs("Ins"), cs("Shf"), cs("N+"));
    t.set(Member::Insert, Some(&ins));
    t.set(Member::Shift, Some(&shf));
    t.set(Member::NumpadPlus, Some(&nplus));
    s.set(&t);
    s.assert_key("OBS_KEY_INSERT", "Ins");
}

/// The numpad template: depadded (only ' ', '\t', '\n' and '\r'), " %1"
/// appended when there is no "%1", then every "%1" replaced, left to right,
/// without rescanning.
#[test]
fn numpad_templates() {
    let t = assert_numpad_template;
    t(b"Numpad %1", b"Numpad 0", b"Numpad 5", b"Numpad 9");
    t(b"%1 (Keypad)", b"0 (Keypad)", b"5 (Keypad)", b"9 (Keypad)");
    t(b"Pad", b"Pad 0", b"Pad 5", b"Pad 9");
    t(b"%1", b"0", b"5", b"9");
    t(b"%1-%1", b"0-0", b"5-5", b"9-9");
    t(b"%1%1", b"00", b"55", b"99");
    t(b"  Pad %1\t", b"Pad 0", b"Pad 5", b"Pad 9");
    t(b" \t\r\nPad\r\n", b"Pad 0", b"Pad 5", b"Pad 9");
    t(b"  %1  ", b"0", b"5", b"9");
    t(
        b"\x0bPad\x0b",
        b"\x0bPad\x0b 0",
        b"\x0bPad\x0b 5",
        b"\x0bPad\x0b 9",
    );
    t(b"Pad  %1", b"Pad  0", b"Pad  5", b"Pad  9");
    t(b"%%1", b"%0", b"%5", b"%9");
    t(b"%11", b"01", b"51", b"91");
    t(b"%2", b"%2 0", b"%2 5", b"%2 9");
    t(b"% 1", b"% 1 0", b"% 1 5", b"% 1 9");
    t(b"100%", b"100% 0", b"100% 5", b"100% 9");
    t(b"%d %s", b"%d %s 0", b"%d %s 5", b"%d %s 9");
    t(
        b"Ziffernblock %1",
        b"Ziffernblock 0",
        b"Ziffernblock 5",
        b"Ziffernblock 9",
    );
    t(
        b"\xe3\x83\x86\xe3\x83\xb3\xe3\x82\xad\xe3\x83\xbc %1",
        b"\xe3\x83\x86\xe3\x83\xb3\xe3\x82\xad\xe3\x83\xbc 0",
        b"\xe3\x83\x86\xe3\x83\xb3\xe3\x82\xad\xe3\x83\xbc 5",
        b"\xe3\x83\x86\xe3\x83\xb3\xe3\x82\xad\xe3\x83\xbc 9",
    );
    t(b"\xff\xfe %1", b"\xff\xfe 0", b"\xff\xfe 5", b"\xff\xfe 9");
    t(b"x", b"x 0", b"x 5", b"x 9");
}

/// The mouse template: the same rules, with 1 to 29.
#[test]
fn mouse_templates() {
    let t = assert_mouse_template;
    t(b"Mouse %1", b"Mouse 1", b"Mouse 10", b"Mouse 29");
    t(b"Maus", b"Maus 1", b"Maus 10", b"Maus 29");
    t(b"%1", b"1", b"10", b"29");
    t(
        b"Button %1 (%1)",
        b"Button 1 (1)",
        b"Button 10 (10)",
        b"Button 29 (29)",
    );
    t(b"%1%1", b"11", b"1010", b"2929");
    t(b"\tM %1 \n", b"M 1", b"M 10", b"M 29");
    t(b"%11", b"11", b"101", b"291");
    t(b"%%1", b"%1", b"%10", b"%29");
    t(b"%2", b"%2 1", b"%2 10", b"%2 29");
    t(
        b"\xd8\xa7\xd9\x84\xd9\x81\xd8\xa3\xd8\xb1\xd8\xa9 %1",
        b"\xd8\xa7\xd9\x84\xd9\x81\xd8\xa3\xd8\xb1\xd8\xa9 1",
        b"\xd8\xa7\xd9\x84\xd9\x81\xd8\xa3\xd8\xb1\xd8\xa9 10",
        b"\xd8\xa7\xd9\x84\xd9\x81\xd8\xa3\xd8\xb1\xd8\xa9 29",
    );
}

/// The two templates are independent: setting one leaves the other's names.
#[test]
fn templates_independent() {
    let mut s = Store::default();
    let (num, mouse) = (cs("G num %1"), cs("G mouse %1"));
    s.set(
        &Translations::new()
            .with(Member::NumpadNum, &num)
            .with(Member::MouseNum, &mouse),
    );

    let num2 = cs("G2 num %1");
    s.set(&Translations::new().with(Member::NumpadNum, &num2));
    s.assert_key("OBS_KEY_NUM4", "G2 num 4");
    s.assert_key("OBS_KEY_MOUSE4", "G mouse 4");

    let mouse2 = cs("G2 mouse %1");
    s.set(&Translations::new().with(Member::MouseNum, &mouse2));
    s.assert_key("OBS_KEY_NUM4", "G2 num 4");
    s.assert_key("OBS_KEY_MOUSE4", "G2 mouse 4");
}

/// The apple_* members are for macOS only; elsewhere they change nothing.
#[test]
fn apple_members_ignored() {
    let mut s = Store::default();
    let f = Filled::new("H");
    s.set(&f.t());

    let strs = [
        (Member::AppleKeypadNum, cs("H2 apple %1")),
        (Member::AppleKeypadDivide, cs("H2 apple divide")),
        (Member::AppleKeypadMultiply, cs("H2 apple multiply")),
        (Member::AppleKeypadMinus, cs("H2 apple minus")),
        (Member::AppleKeypadPlus, cs("H2 apple plus")),
        (Member::AppleKeypadDecimal, cs("H2 apple decimal")),
        (Member::AppleKeypadEqual, cs("H2 apple equal")),
    ];
    let mut t = Translations::new();
    for (m, v) in &strs {
        t.set(*m, Some(v));
    }
    s.set(&t);
    assert_named_keys(&s, "H");
    assert_numbered_keys(&s, "H");
}

/// A combination shows the translated modifiers and key, in the order
/// control, meta, alt, shift, key.
#[test]
fn combination_string() {
    let mut s = Store::default();
    let strs = [
        (Member::Control, cs("Strg")),
        (Member::Meta, cs("Super-Taste")),
        (Member::Alt, cs("Alt Gr")),
        (Member::Shift, cs("Umschalt")),
        (Member::NumpadNum, cs("Ziffernblock %1")),
        (Member::MouseNum, cs("Maustaste %1")),
    ];
    let mut t = Translations::new();
    for (m, v) in &strs {
        t.set(*m, Some(v));
    }
    s.set(&t);

    assert_eq!(
        show(&s.combination_to_str(
            INTERACT_SHIFT_KEY | INTERACT_CONTROL_KEY,
            key("OBS_KEY_NUM5")
        )),
        "Strg + Umschalt + Ziffernblock 5"
    );
    assert_eq!(
        show(&s.combination_to_str(
            INTERACT_SHIFT_KEY | INTERACT_CONTROL_KEY | INTERACT_ALT_KEY | INTERACT_COMMAND_KEY,
            key("OBS_KEY_MOUSE4")
        )),
        "Strg + Super-Taste + Alt Gr + Umschalt + Maustaste 4"
    );
}

/// The arrow keys take their names from left, right, up and down, like the
/// other named keys.
#[test]
fn arrow_keys_translated() {
    let mut s = Store::default();
    let (l, r, u, d) = (cs("Links"), cs("Rechts"), cs("Hoch"), cs("Runter"));
    s.set(
        &Translations::new()
            .with(Member::Left, &l)
            .with(Member::Right, &r)
            .with(Member::Up, &u)
            .with(Member::Down, &d),
    );
    s.assert_key("OBS_KEY_LEFT", "Links");
    s.assert_key("OBS_KEY_RIGHT", "Rechts");
    s.assert_key("OBS_KEY_UP", "Hoch");
    s.assert_key("OBS_KEY_DOWN", "Runter");
}

/// An empty or padding-only template after a set keeps the names it set;
/// the other template and the named keys in the same struct still apply.
#[test]
fn empty_templates_keep_names() {
    let mut s = Store::default();
    let (num, mouse) = (cs("K num %1"), cs("K mouse %1"));
    s.set(
        &Translations::new()
            .with(Member::NumpadNum, &num)
            .with(Member::MouseNum, &mouse),
    );

    for (i, empty) in [&b""[..], b"  ", b"\t", b"\r\n"].into_iter().enumerate() {
        let empty = cs(empty);
        let num = cs(format!("K{i} num %1"));
        let mouse = cs(format!("K{i} mouse %1"));
        let ins = cs(format!("K{i} insert"));

        s.set(
            &Translations::new()
                .with(Member::NumpadNum, &empty)
                .with(Member::MouseNum, &mouse)
                .with(Member::Insert, &ins),
        );
        s.assert_key("OBS_KEY_MOUSE7", format!("K{i} mouse 7"));
        s.assert_key("OBS_KEY_INSERT", ins.as_bytes());
        // The numpad names of the previous round.
        let prev = if i > 0 {
            format!("K{} num 7", i - 1)
        } else {
            "K num 7".to_owned()
        };
        s.assert_key("OBS_KEY_NUM7", prev);

        s.set(
            &Translations::new()
                .with(Member::NumpadNum, &num)
                .with(Member::MouseNum, &empty),
        );
        s.assert_key("OBS_KEY_NUM7", format!("K{i} num 7"));
        s.assert_key("OBS_KEY_MOUSE7", format!("K{i} mouse 7"));
    }
}

// ---------------------------------------------------------------------------
// Edge cases of the safe API.

/// `Member::ALL` is the struct's member order, as `obs-hotkey.h` declares
/// it, and each member's index is its position.
#[test]
fn members_in_struct_order() {
    let header = [
        "insert",
        "del",
        "home",
        "end",
        "page_up",
        "page_down",
        "num_lock",
        "scroll_lock",
        "caps_lock",
        "backspace",
        "tab",
        "print",
        "pause",
        "left",
        "right",
        "up",
        "down",
        "shift",
        "alt",
        "control",
        "meta",
        "menu",
        "space",
        "numpad_num",
        "numpad_divide",
        "numpad_multiply",
        "numpad_minus",
        "numpad_plus",
        "numpad_decimal",
        "apple_keypad_num",
        "apple_keypad_divide",
        "apple_keypad_multiply",
        "apple_keypad_minus",
        "apple_keypad_plus",
        "apple_keypad_decimal",
        "apple_keypad_equal",
        "mouse_num",
        "escape",
    ];
    assert_eq!(MEMBER_COUNT, header.len());
    for (i, (m, name)) in Member::ALL.iter().zip(header).enumerate() {
        assert_eq!(m.index(), i);
        assert_eq!(m.name(), name);
    }
    // get and set address each member alone.
    let v = cs("v");
    for m in Member::ALL {
        let t = Translations::new().with(m, &v);
        for other in Member::ALL {
            assert_eq!(t.get(other).is_some(), other == m, "{}", other.name());
        }
        assert_eq!(t.members()[m.index()], Some(v.as_c_str()));
    }
}

/// The named keys, in the C's `ADD_TRANSLATION` order, for each keypad.
#[test]
fn named_key_tables() {
    let common = [
        ("OBS_KEY_INSERT", Member::Insert),
        ("OBS_KEY_DELETE", Member::Del),
        ("OBS_KEY_HOME", Member::Home),
        ("OBS_KEY_END", Member::End),
        ("OBS_KEY_PAGEUP", Member::PageUp),
        ("OBS_KEY_PAGEDOWN", Member::PageDown),
        ("OBS_KEY_NUMLOCK", Member::NumLock),
        ("OBS_KEY_SCROLLLOCK", Member::ScrollLock),
        ("OBS_KEY_CAPSLOCK", Member::CapsLock),
        ("OBS_KEY_BACKSPACE", Member::Backspace),
        ("OBS_KEY_TAB", Member::Tab),
        ("OBS_KEY_PRINT", Member::Print),
        ("OBS_KEY_PAUSE", Member::Pause),
        ("OBS_KEY_LEFT", Member::Left),
        ("OBS_KEY_RIGHT", Member::Right),
        ("OBS_KEY_UP", Member::Up),
        ("OBS_KEY_DOWN", Member::Down),
        ("OBS_KEY_SHIFT", Member::Shift),
        ("OBS_KEY_ALT", Member::Alt),
        ("OBS_KEY_CONTROL", Member::Control),
        ("OBS_KEY_META", Member::Meta),
        ("OBS_KEY_MENU", Member::Menu),
        ("OBS_KEY_SPACE", Member::Space),
        ("OBS_KEY_ESCAPE", Member::Escape),
    ];
    let standard = [
        ("OBS_KEY_NUMSLASH", Member::NumpadDivide),
        ("OBS_KEY_NUMASTERISK", Member::NumpadMultiply),
        ("OBS_KEY_NUMMINUS", Member::NumpadMinus),
        ("OBS_KEY_NUMPLUS", Member::NumpadPlus),
        ("OBS_KEY_NUMPERIOD", Member::NumpadDecimal),
    ];
    let apple = [
        ("OBS_KEY_NUMSLASH", Member::AppleKeypadDivide),
        ("OBS_KEY_NUMASTERISK", Member::AppleKeypadMultiply),
        ("OBS_KEY_NUMMINUS", Member::AppleKeypadMinus),
        ("OBS_KEY_NUMPLUS", Member::AppleKeypadPlus),
        ("OBS_KEY_NUMPERIOD", Member::AppleKeypadDecimal),
        ("OBS_KEY_NUMEQUAL", Member::AppleKeypadEqual),
    ];
    for (keypad, own) in [
        (Keypad::Standard, &standard[..]),
        (Keypad::Apple, &apple[..]),
    ] {
        let want: Vec<(i32, Member)> = common
            .iter()
            .chain(own)
            .map(|(k, m)| (key(k), *m))
            .collect();
        assert_eq!(named_keys(keypad).collect::<Vec<_>>(), want, "{keypad:?}");
    }
    assert_eq!(numpad_template(Keypad::Standard), Member::NumpadNum);
    assert_eq!(numpad_template(Keypad::Apple), Member::AppleKeypadNum);
}

/// NUM0-NUM9 with "0" to "9", and MOUSE1-MOUSE29 with "1" to "29".
#[test]
fn numbered_key_tables() {
    assert_eq!(NUMPAD_KEYS.len(), 10);
    for (i, (code, digits)) in NUMPAD_KEYS.iter().enumerate() {
        assert_eq!(*code, key(&format!("OBS_KEY_NUM{i}")));
        assert_eq!(*digits, i.to_string());
    }
    assert_eq!(MOUSE_KEYS.len(), 29);
    for (i, (code, digits)) in MOUSE_KEYS.iter().enumerate() {
        assert_eq!(*code, key(&format!("OBS_KEY_MOUSE{}", i + 1)));
        assert_eq!(*digits, (i + 1).to_string());
    }
}

/// Only ' ', '\t', '\n' and '\r' are padding, on either end; inner padding
/// stays.
#[test]
fn depad_only_four_bytes() {
    for b in 1..=255u8 {
        let pad = matches!(b, b' ' | b'\t' | b'\n' | b'\r');
        assert_eq!(is_padding(b), pad, "{b:#04x}");
        let s = [b, b, b'x', b, b'y', b, b];
        let want: &[u8] = if pad { &s[2..5] } else { &s };
        assert_eq!(depad(&s), want, "{b:#04x}");
    }
    assert_eq!(depad(b""), b"");
    assert_eq!(depad(b" \t\r\n \t"), b"");
    assert_eq!(depad(b"\x0b\x0c"), b"\x0b\x0c");
    // A non-breaking space (U+00A0) is not padding.
    assert_eq!(
        depad("\u{a0}x\u{a0}".as_bytes()),
        "\u{a0}x\u{a0}".as_bytes()
    );
}

/// The template the C expands: depadded, `" %1"` appended when there is
/// none, `None` when nothing is left.
#[test]
fn prepare_template_cases() {
    let cases: [(&[u8], Option<&[u8]>); 13] = [
        (b"", None),
        (b" ", None),
        (b"\t", None),
        (b" \t\r\n ", None),
        (b"%1", Some(b"%1")),
        (b"Pad", Some(b"Pad %1")),
        (b" Pad %1 ", Some(b"Pad %1")),
        (b"%", Some(b"% %1")),
        (b"1", Some(b"1 %1")),
        (b"% 1", Some(b"% 1 %1")),
        (b"1%", Some(b"1% %1")),
        (b"%%11", Some(b"%%11")),
        (b"\x0b", Some(b"\x0b %1")),
    ];
    for (input, want) in cases {
        assert_eq!(
            prepare_template(input).as_deref(),
            want,
            "{:?}",
            show(input)
        );
    }
    assert!(has_placeholder(b"a%1"));
    assert!(!has_placeholder(b"%"));
    assert!(!has_placeholder(b"1%"));
    assert!(!has_placeholder(b""));
}

/// Every `%1`, left to right, without rescanning what was put in.
#[test]
fn replace_placeholder_cases() {
    let cases: [(&[u8], &[u8], &[u8]); 12] = [
        (b"", b"5", b""),
        (b"%1", b"5", b"5"),
        (b"%1%1%1", b"29", b"292929"),
        (b"%%1", b"1", b"%1"),
        (b"%%11", b"1", b"%11"),
        (b"%1%", b"7", b"7%"),
        (b"%%", b"7", b"%%"),
        (b"1%1", b"1", b"11"),
        (b"%11%1", b"10", b"10110"),
        (b"%", b"7", b"%"),
        (b"a%1b%1c", b"", b"abc"),
        (b"%1", b"%1", b"%1"),
    ];
    for (template, digits, want) in cases {
        assert_eq!(
            replace_placeholder(template, digits),
            want,
            "{:?} with {:?}",
            show(template),
            show(digits)
        );
    }
}

/// printf conversions stay literal, with and without `%1`: nothing is ever
/// formatted.
#[test]
fn printf_text_stays_literal() {
    for t in ["%s %n %d %x %p %% %*s %ls", "%s%1%n", "%1$s %1$n"] {
        let got = expand(t.as_bytes(), "29").unwrap();
        let want = if t.contains("%1") {
            t.replace("%1", "29")
        } else {
            format!("{t} 29")
        };
        assert_eq!(show(&got), want);
    }
}

/// Many placeholders, and a large template.
#[test]
fn many_placeholders_and_large_templates() {
    let many = b"%1".repeat(10_000);
    assert_eq!(expand(&many, "29").unwrap(), b"29".repeat(10_000));

    let mut big = vec![b'a'; 4 << 20];
    big.extend_from_slice(b"%1");
    let got = expand(&big, "7").unwrap();
    assert_eq!(got.len(), big.len() - 1);
    assert!(got.ends_with(b"a7"));

    let padded = [&b" ".repeat(1 << 20)[..], b"x", &b"\t".repeat(1 << 20)].concat();
    assert_eq!(expand(&padded, "3").unwrap(), b"x 3");
    assert_eq!(expand(&b" ".repeat(1 << 20), "3"), None);
}

/// Multi-byte UTF-8 next to `%1`, and broken sequences around it, keep
/// their bytes.
#[test]
fn utf8_and_non_utf8_bytes() {
    let cases: [(&[u8], &[u8]); 4] = [
        ("€%1€".as_bytes(), "€10€".as_bytes()),
        ("%1\u{1f600}".as_bytes(), "10\u{1f600}".as_bytes()),
        (b"\xe2\x82%1\xac", b"\xe2\x8210\xac"),
        (b"\xff%1\xfe", b"\xff10\xfe"),
    ];
    for (t, want) in cases {
        assert_eq!(expand(t, "10").unwrap(), want, "{:?}", show(t));
    }
    // Every byte but NUL, around a placeholder.
    let all: Vec<u8> = (1..=255u8).collect();
    let t = [&all[..], b"%1", &all[..]].concat();
    let want = [depad(&all), b"9", depad(&all)].concat();
    // depad only touches the ends, which here are 0x01 and 0xff.
    assert_eq!(depad(&t), &t[..]);
    assert_eq!(expand(&t, "9").unwrap(), want);
}

fn calls(t: &Translations<'_>, keypad: Keypad) -> Vec<(i32, Vec<u8>)> {
    let mut out = Vec::new();
    for_each_store(t, keypad, |k, s| out.push((k, s.to_bytes().to_vec())));
    out
}

/// Every member set: the named keys, then NUM0-NUM9, then MOUSE1-MOUSE29,
/// each once, in that order.
#[test]
fn store_order_and_count() {
    let f = Filled::new("Z");
    for (keypad, named, num_member) in [
        (Keypad::Standard, 29, "numpad_num"),
        (Keypad::Apple, 30, "apple_keypad_num"),
    ] {
        let got = calls(&f.t(), keypad);
        let mut want: Vec<(i32, Vec<u8>)> = named_keys(keypad)
            .map(|(k, m)| (k, format!("Z {}", m.name()).into_bytes()))
            .collect();
        assert_eq!(want.len(), named);
        for (k, d) in NUMPAD_KEYS {
            want.push((k, format!("Z {num_member} {d}").into_bytes()));
        }
        for (k, d) in MOUSE_KEYS {
            want.push((k, format!("Z mouse {d}").into_bytes()));
        }
        assert_eq!(got, want, "{keypad:?}");
    }
}

/// macOS: the apple_* members name the keypad, `=` included, and the
/// numpad_* members are ignored.
#[test]
fn apple_keypad() {
    let f = Filled::new("M");
    let got: HashMap<i32, Vec<u8>> = calls(&f.t(), Keypad::Apple).into_iter().collect();
    for (k, want) in [
        ("OBS_KEY_NUMSLASH", "M apple_keypad_divide"),
        ("OBS_KEY_NUMASTERISK", "M apple_keypad_multiply"),
        ("OBS_KEY_NUMMINUS", "M apple_keypad_minus"),
        ("OBS_KEY_NUMPLUS", "M apple_keypad_plus"),
        ("OBS_KEY_NUMPERIOD", "M apple_keypad_decimal"),
        ("OBS_KEY_NUMEQUAL", "M apple_keypad_equal"),
        ("OBS_KEY_NUM4", "M apple_keypad_num 4"),
        ("OBS_KEY_LEFT", "M left"),
    ] {
        assert_eq!(show(&got[&key(k)]), want, "{k}");
    }
    let standard: HashMap<i32, Vec<u8>> = calls(&f.t(), Keypad::Standard).into_iter().collect();
    assert!(!standard.contains_key(&key("OBS_KEY_NUMEQUAL")));

    // Only the numpad_* members set: macOS stores none of the keypad.
    let mut t = Translations::new();
    for m in Member::ALL {
        if m.name().starts_with("numpad_") {
            t.set(m, f.t().get(m));
        }
    }
    assert!(calls(&t, Keypad::Apple).is_empty());
    assert_eq!(calls(&t, Keypad::Standard).len(), 5 + 10);
}

/// All members NULL: nothing is stored.
#[test]
fn nothing_set_stores_nothing() {
    for keypad in [Keypad::Standard, Keypad::Apple] {
        assert!(calls(&Translations::new(), keypad).is_empty());
    }
}

/// A named key's string is stored exactly as given: not depadded, not
/// expanded, empty included.
#[test]
fn named_strings_stored_as_is() {
    for v in [&b" %1 \t"[..], b"", b"%s", b"\xff", b"  "] {
        let v = cs(v);
        for m in Member::ALL {
            if matches!(
                m,
                Member::NumpadNum | Member::AppleKeypadNum | Member::MouseNum
            ) {
                continue;
            }
            for keypad in [Keypad::Standard, Keypad::Apple] {
                let got = calls(&Translations::new().with(m, &v), keypad);
                match named_keys(keypad).find(|(_, nm)| *nm == m) {
                    Some((k, _)) => assert_eq!(got, vec![(k, v.as_bytes().to_vec())]),
                    None => assert!(got.is_empty(), "{} ignored on {keypad:?}", m.name()),
                }
            }
        }
    }
}

/// An empty or padding-only template stores exactly what a NULL one does.
#[test]
fn empty_template_is_null() {
    let f = Filled::new("Q");
    for empty in [&b""[..], b" ", b"\r\n", b" \t \n"] {
        let empty = cs(empty);
        for template in [Member::NumpadNum, Member::AppleKeypadNum, Member::MouseNum] {
            let mut with_empty = f.t();
            with_empty.set(template, Some(&empty));
            let mut with_null = f.t();
            with_null.set(template, None);
            for keypad in [Keypad::Standard, Keypad::Apple] {
                assert_eq!(calls(&with_empty, keypad), calls(&with_null, keypad));
            }
        }
    }
}

/// A plain reference for the expansion: split at each `%1`, join with the
/// digits.
fn reference_expand(template: &[u8], digits: &[u8]) -> Option<Vec<u8>> {
    let pad = |b: &u8| matches!(b, b' ' | b'\t' | b'\n' | b'\r');
    let start = template.iter().position(|b| !pad(b))?;
    let end = template.iter().rposition(|b| !pad(b))? + 1;
    let mut t = template[start..end].to_vec();
    if !t.windows(2).any(|w| w == b"%1") {
        t.extend_from_slice(b" %1");
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < t.len() {
        if t[i..].starts_with(b"%1") {
            out.extend_from_slice(digits);
            i += 2;
        } else {
            out.push(t[i]);
            i += 1;
        }
    }
    Some(out)
}

fn template_bytes() -> impl Strategy<Value = Vec<u8>> {
    let piece = prop_oneof![
        Just(b"%1".to_vec()),
        Just(b"%".to_vec()),
        Just(b"1".to_vec()),
        Just(b" ".to_vec()),
        Just(b"\t".to_vec()),
        Just(b"\r\n".to_vec()),
        Just(b"\x0b".to_vec()),
        Just("é".as_bytes().to_vec()),
        Just("ミ".as_bytes().to_vec()),
        Just("\u{1f600}".as_bytes().to_vec()),
        Just(b"%s%n".to_vec()),
        (1..=255u8).prop_map(|b| vec![b]),
    ];
    prop::collection::vec(piece, 0..24).prop_map(|p| p.concat())
}

fn member_value() -> impl Strategy<Value = Option<Vec<u8>>> {
    prop_oneof![Just(None), template_bytes().prop_map(Some)]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(2000)))]

    /// Any bytes: no panic, the reference expansion, no NUL.
    #[test]
    fn expand_matches_reference(t in template_bytes(), i in 0usize..29) {
        let digits = MOUSE_KEYS[i].1;
        let got = expand(&t, digits);
        prop_assert_eq!(&got, &reference_expand(&t, digits.as_bytes()));
        if let Some(got) = got {
            prop_assert!(!got.contains(&0));
        }
    }

    /// Any members: no panic, and the store calls follow the members.
    #[test]
    fn store_calls_follow_members(values in prop::collection::vec(member_value(), MEMBER_COUNT)) {
        let owned: Vec<Option<CString>> =
            values.iter().map(|v| v.as_ref().map(|b| cs(b.clone()))).collect();
        let mut members = [None; MEMBER_COUNT];
        for (slot, v) in members.iter_mut().zip(&owned) {
            *slot = v.as_deref();
        }
        let t = Translations::from_members(members);
        for keypad in [Keypad::Standard, Keypad::Apple] {
            let mut want = Vec::new();
            for (k, m) in named_keys(keypad) {
                if let Some(v) = t.get(m) {
                    want.push((k, v.to_bytes().to_vec()));
                }
            }
            let tmpl = |m: Member| t.get(m).map(|v| v.to_bytes().to_vec());
            if let Some(n) = tmpl(numpad_template(keypad)) {
                for (k, d) in NUMPAD_KEYS {
                    if let Some(e) = reference_expand(&n, d.as_bytes()) {
                        want.push((k, e));
                    }
                }
            }
            if let Some(n) = tmpl(Member::MouseNum) {
                for (k, d) in MOUSE_KEYS {
                    if let Some(e) = reference_expand(&n, d.as_bytes()) {
                        want.push((k, e));
                    }
                }
            }
            prop_assert_eq!(calls(&t, keypad), want);
        }
    }
}
