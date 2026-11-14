//! Tier 1: the safe core of `libobs/obs-key-combination-host-posix.c.h`.
//!
//! The first three tests mirror `test/cmocka/test_nix_key_combination.c`
//! one to one, with the same names and cases. Its `names()` sets the hotkey
//! translations that `obs_key_to_str()` returns; here a key name table
//! stands in for them. [`Counted`] is a [`Text`] that counts its live
//! buffers the way `bnum_allocs` counts `bmalloc`s, so each test also checks
//! that every name buffer is freed.
//!
//! The cases after those pin the call order and the edges of the safe API.

use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::{CStr, CString};

use obs_hotkey::key_combination::{MODIFIER_KEYS, Text, combination_to_str};
use obs_hotkey::name_map::{
    KEY_NONE, OBS_KEY_ALT, OBS_KEY_CONTROL, OBS_KEY_INSERT, OBS_KEY_META, OBS_KEY_SHIFT,
};
use obs_hotkey::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
    KeyCombination,
};

const C: u32 = INTERACT_CONTROL_KEY;
const M: u32 = INTERACT_COMMAND_KEY;
const A: u32 = INTERACT_ALT_KEY;
const S: u32 = INTERACT_SHIFT_KEY;

/// `INTERACT_CAPS_KEY` .. `INTERACT_IS_RIGHT` without the four named ones
/// (`obs-interaction.h`), which the formatter ignores.
const OTHER_FLAGS: [u32; 8] = [1, 1 << 4, 1 << 5, 1 << 6, 1 << 8, 1 << 9, 1 << 10, 1 << 11];

thread_local! {
    /// Live [`Counted`] buffers on this thread (`bnum_allocs`).
    static LIVE: Cell<i64> = const { Cell::new(0) };
}

/// A `dstr`: no storage until something is appended, storage freed only by
/// [`Text::free`].
#[derive(Debug, Default)]
struct Counted(Option<Vec<u8>>);

impl Counted {
    fn of(s: &str) -> Self {
        let mut t = Self::default();
        t.cat(&CString::new(s).unwrap());
        t
    }

    fn text(&self) -> &str {
        std::str::from_utf8(self.0.as_deref().unwrap_or_default()).unwrap()
    }

    fn append(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if self.0.is_none() {
            LIVE.with(|l| l.set(l.get() + 1));
        }
        self.0.get_or_insert_with(Vec::new).extend_from_slice(bytes);
    }
}

impl Text for Counted {
    fn free(&mut self) {
        if self.0.take().is_some() {
            LIVE.with(|l| l.set(l.get() - 1));
        }
    }
    fn cat(&mut self, s: &CStr) {
        self.append(s.to_bytes());
    }
    fn cat_text(&mut self, other: &Self) {
        let bytes = other.0.clone().unwrap_or_default();
        self.append(&bytes);
    }
    fn is_empty(&self) -> bool {
        self.0
            .as_ref()
            .is_none_or(|b| b.first().is_none_or(|&c| c == 0))
    }
}

/// `obs_key_to_str` over a name table: the key's name, or nothing.
fn namer<'a>(names: &'a HashMap<i32, &'static str>) -> impl FnMut(i32, &mut Counted) + 'a {
    |key, out| {
        assert!(out.is_empty(), "each name is written into a fresh text");
        if let Some(name) = names.get(&key) {
            out.cat(&CString::new(*name).unwrap());
        }
    }
}

/// `names()` of the C test.
fn names(
    control: &'static str,
    meta: &'static str,
    alt: &'static str,
    shift: &'static str,
    insert: &'static str,
) -> HashMap<i32, &'static str> {
    HashMap::from([
        (OBS_KEY_CONTROL, control),
        (OBS_KEY_META, meta),
        (OBS_KEY_ALT, alt),
        (OBS_KEY_SHIFT, shift),
        (OBS_KEY_INSERT, insert),
    ])
}

/// `expect()` of the C test: formats onto `prefix`, compares, and checks
/// that only the output's own buffer is left.
fn expect(table: &HashMap<i32, &'static str>, modifiers: u32, key: i32, prefix: &str, want: &str) {
    let before = LIVE.with(Cell::get);
    let mut text = Counted::of(prefix);
    combination_to_str(KeyCombination { modifiers, key }, namer(table), &mut text);
    assert_eq!(
        text.text(),
        want,
        "modifiers {modifiers:#x}, key {key}, prefix {prefix:?}"
    );
    text.free();
    assert_eq!(LIVE.with(Cell::get), before, "a name buffer was not freed");
}

#[test]
fn every_modifier_subset() {
    let t = names("C", "M", "A", "S", "K");
    let k = OBS_KEY_INSERT;
    expect(&t, 0, k, "", "K");
    expect(&t, C, k, "", "C + K");
    expect(&t, M, k, "", "M + K");
    expect(&t, A, k, "", "A + K");
    expect(&t, S, k, "", "S + K");
    expect(&t, C | M, k, "", "C + M + K");
    expect(&t, C | A, k, "", "C + A + K");
    expect(&t, C | S, k, "", "C + S + K");
    expect(&t, M | A, k, "", "M + A + K");
    expect(&t, M | S, k, "", "M + S + K");
    expect(&t, A | S, k, "", "A + S + K");
    expect(&t, C | M | A, k, "", "C + M + A + K");
    expect(&t, C | M | S, k, "", "C + M + S + K");
    expect(&t, C | A | S, k, "", "C + A + S + K");
    expect(&t, M | A | S, k, "", "M + A + S + K");
    expect(&t, C | M | A | S, k, "", "C + M + A + S + K");
}

#[test]
fn appends_and_keeps_duplicate_modifiers() {
    let t = names("Ctl", "Meta", "Alt", "Shift", "Ins");
    expect(&t, 0, KEY_NONE, "existing", "existing");
    expect(&t, 0, KEY_NONE, "", "");
    expect(&t, 0, OBS_KEY_INSERT, "existing", "existing + Ins");
    expect(&t, C, KEY_NONE, "existing + ", "existing +  + Ctl");
    // Characterized, not endorsed: modifier keys are appended twice.
    expect(&t, C, OBS_KEY_CONTROL, "", "Ctl + Ctl");
    expect(
        &t,
        C | M | A | S,
        KEY_NONE,
        "existing",
        "existing + Ctl + Meta + Alt + Shift",
    );
    let known = C | M | A | S;
    expect(&t, !known, OBS_KEY_INSERT, "", "Ins");
    expect(
        &t,
        u32::MAX,
        OBS_KEY_INSERT,
        "",
        "Ctl + Meta + Alt + Shift + Ins",
    );
}

#[test]
fn empty_names_do_not_add_separators() {
    let all = C | M | A | S;
    let t = names("", "Meta", "", "Shift", "");
    expect(&t, all, OBS_KEY_INSERT, "", "Meta + Shift");
    expect(&t, all, OBS_KEY_INSERT, "prefix", "prefix + Meta + Shift");
    let t = names("", "", "", "", "");
    expect(&t, all, OBS_KEY_INSERT, "prefix", "prefix");
    expect(&t, all, OBS_KEY_INSERT, "", "");
    // Whitespace and UTF-8 names are opaque bytes, not trimmed or normalized.
    let t = names(" ", "\u{e9}", "", "", "K");
    expect(&t, all, OBS_KEY_INSERT, "", "  + \u{e9} + K");
}

#[test]
fn each_unknown_bit_is_ignored() {
    let all = names("C", "M", "A", "S", "K");
    for flag in OTHER_FLAGS {
        expect(&all, flag, OBS_KEY_INSERT, "", "K");
        expect(&all, flag | C, KEY_NONE, "", "C");
    }
    expect(
        &all,
        OTHER_FLAGS.iter().fold(0, |a, f| a | f),
        KEY_NONE,
        "",
        "",
    );
}

#[test]
fn names_modifiers_then_key_in_order() {
    let mut calls = Vec::new();
    let mut text = Counted::default();
    combination_to_str(
        KeyCombination {
            modifiers: u32::MAX,
            key: OBS_KEY_INSERT,
        },
        |key, out: &mut Counted| {
            calls.push(key);
            out.cat(c"x");
        },
        &mut text,
    );
    assert_eq!(
        calls,
        [
            OBS_KEY_CONTROL,
            OBS_KEY_META,
            OBS_KEY_ALT,
            OBS_KEY_SHIFT,
            OBS_KEY_INSERT
        ]
    );
    assert_eq!(text.text(), "x + x + x + x + x");
    text.free();
}

#[test]
fn key_none_is_not_named() {
    let mut calls = Vec::new();
    let mut text = Counted::default();
    combination_to_str(
        KeyCombination {
            modifiers: 0,
            key: KEY_NONE,
        },
        |key, _: &mut Counted| calls.push(key),
        &mut text,
    );
    assert!(calls.is_empty());
    assert!(text.0.is_none(), "no storage when nothing is named");
}

#[test]
fn any_other_key_is_named_even_out_of_range() {
    for key in [-1, 1, i32::MAX, i32::MIN] {
        let mut calls = Vec::new();
        let mut text = Counted::default();
        combination_to_str(
            KeyCombination { modifiers: 0, key },
            |k, _: &mut Counted| calls.push(k),
            &mut text,
        );
        assert_eq!(calls, [key]);
    }
}

#[test]
fn modifier_table_order() {
    assert_eq!(
        MODIFIER_KEYS,
        [
            (C, OBS_KEY_CONTROL),
            (M, OBS_KEY_META),
            (A, OBS_KEY_ALT),
            (S, OBS_KEY_SHIFT)
        ]
    );
}
