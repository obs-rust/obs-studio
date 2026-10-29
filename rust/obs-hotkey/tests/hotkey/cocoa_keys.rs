//! Tier 1: the safe core of `libobs/obs-cocoa-keys.c.h`.
//!
//! The first eight tests port `test/cmocka/test_cocoa_keys_steps.c` one to
//! one, with the same names, inputs and expectations, in the same order.
//! Each starts from the C test's zeroed table: [`Fixture::new`].
//!
//! The cases after those pin the edges of each function and the two fixes
//! (the button bit, and a key outside the table), and the proptests check
//! each function against a model written from the C.

use obs_hotkey::cocoa_keys::{
    self as ck, FLAG_ALTERNATE, FLAG_COMMAND, FLAG_CONTROL, FLAG_SHIFT, INVALID, KeyCode,
    OBS_KEY_ALT, OBS_KEY_CONTROL, OBS_KEY_LAST_VALUE, OBS_KEY_META, OBS_KEY_MOUSE1,
    OBS_KEY_MOUSE29, OBS_KEY_SHIFT,
};
use obs_hotkey::name_map::{KEY_NONE, key_from_name};
use proptest::prelude::*;

const SHIFT: u64 = FLAG_SHIFT;
const CONTROL: u64 = FLAG_CONTROL;
const ALTERNATE: u64 = FLAG_ALTERNATE;
const COMMAND: u64 = FLAG_COMMAND;
const ALL: u64 = SHIFT | CONTROL | ALTERNATE | COMMAND;

/// Some macOS virtual key codes (HIToolbox/Events.h).
const KVK_ANSI_A: i32 = 0x00;
const KVK_ANSI_S: i32 = 0x01;
const KVK_RETURN: i32 = 0x24;
const KVK_ESCAPE: i32 = 0x35;
const KVK_F1: i32 = 0x7A;

/// `OBS_KEY_LAST_VALUE` as a key.
fn last() -> i32 {
    i32::try_from(OBS_KEY_LAST_VALUE).unwrap()
}

/// The code of key `name` (`OBS_KEY_<name>`).
fn k(name: &str) -> i32 {
    let full = std::ffi::CString::new(format!("OBS_KEY_{name}")).unwrap();
    let key = key_from_name(&full);
    assert!(name == "NONE" || key != KEY_NONE, "no key {name}");
    key
}

/// The C test's static table after `setup()`: every entry zeroed.
struct Fixture {
    table: Vec<KeyCode>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            table: vec![KeyCode::default(); OBS_KEY_LAST_VALUE],
        }
    }

    fn set_code(&mut self, key: i32, code: i32, is_valid: bool) {
        self.table[usize::try_from(key).unwrap()] = KeyCode { code, is_valid };
    }

    fn to(&self, key: i32) -> i32 {
        ck::to_virtual(&self.table, key)
    }

    fn from(&self, code: i32) -> i32 {
        ck::from_virtual(&self.table, code)
    }
}

#[test]
fn modifier_each_flag() {
    assert!(ck::modifier(SHIFT, OBS_KEY_SHIFT));
    assert!(!ck::modifier(SHIFT, OBS_KEY_ALT));
    assert!(!ck::modifier(SHIFT, OBS_KEY_META));
    assert!(!ck::modifier(SHIFT, OBS_KEY_CONTROL));

    assert!(ck::modifier(ALTERNATE, OBS_KEY_ALT));
    assert!(!ck::modifier(ALTERNATE, OBS_KEY_SHIFT));
    assert!(!ck::modifier(ALTERNATE, OBS_KEY_META));
    assert!(!ck::modifier(ALTERNATE, OBS_KEY_CONTROL));

    assert!(ck::modifier(COMMAND, OBS_KEY_META));
    assert!(!ck::modifier(COMMAND, OBS_KEY_SHIFT));
    assert!(!ck::modifier(COMMAND, OBS_KEY_ALT));
    assert!(!ck::modifier(COMMAND, OBS_KEY_CONTROL));

    assert!(ck::modifier(CONTROL, OBS_KEY_CONTROL));
    assert!(!ck::modifier(CONTROL, OBS_KEY_SHIFT));
    assert!(!ck::modifier(CONTROL, OBS_KEY_ALT));
    assert!(!ck::modifier(CONTROL, OBS_KEY_META));
}

#[test]
fn modifier_all_and_none() {
    for key in [OBS_KEY_SHIFT, OBS_KEY_ALT, OBS_KEY_META, OBS_KEY_CONTROL] {
        assert!(ck::modifier(ALL, key));
        assert!(!ck::modifier(0, key));
        // Every other bit (caps lock, the numeric pad, the device bits,
        // the high word) sets none of them.
        assert!(!ck::modifier(!ALL, key));
    }
}

#[test]
fn modifier_other_keys() {
    assert!(!ck::modifier(u64::MAX, k("NONE")));
    assert!(!ck::modifier(u64::MAX, k("A")));
    assert!(!ck::modifier(u64::MAX, k("CAPSLOCK")));
    assert!(!ck::modifier(u64::MAX, OBS_KEY_MOUSE1));
    assert!(!ck::modifier(u64::MAX, last() - 1));
}

#[test]
fn to_virtual() {
    let mut f = Fixture::new();
    f.set_code(k("A"), KVK_ANSI_A, true);
    f.set_code(k("RETURN"), KVK_RETURN, true);
    f.set_code(k("ESCAPE"), KVK_ESCAPE, false);
    f.set_code(k("NONE"), KVK_F1, true);

    // A valid entry gives its code, 0 included.
    assert_eq!(f.to(k("A")), KVK_ANSI_A);
    assert_eq!(f.to(k("RETURN")), KVK_RETURN);
    // An invalid entry gives INVALID_KEY, whatever its code.
    assert_eq!(f.to(k("ESCAPE")), INVALID);
    assert_eq!(f.to(k("B")), INVALID);
    // OBS_KEY_NONE is an entry like any other.
    assert_eq!(f.to(k("NONE")), KVK_F1);
    // The last entry of the table is read.
    f.set_code(last() - 1, 0x42, true);
    assert_eq!(f.to(last() - 1), 0x42);
    // A code is returned as stored, even INVALID_KEY or a negative one.
    f.set_code(k("C"), INVALID, true);
    f.set_code(k("D"), -7, true);
    assert_eq!(f.to(k("C")), INVALID);
    assert_eq!(f.to(k("D")), -7);
}

#[test]
fn from_virtual() {
    let mut f = Fixture::new();
    f.set_code(k("A"), KVK_ANSI_A, true);
    f.set_code(k("S"), KVK_ANSI_S, true);
    f.set_code(k("RETURN"), KVK_RETURN, true);

    assert_eq!(f.from(KVK_ANSI_A), k("A"));
    assert_eq!(f.from(KVK_ANSI_S), k("S"));
    assert_eq!(f.from(KVK_RETURN), k("RETURN"));
    // A code no valid entry has gives none.
    assert_eq!(f.from(KVK_ESCAPE), KEY_NONE);
    assert_eq!(f.from(INVALID), KEY_NONE);
    assert_eq!(f.from(-1), KEY_NONE);
    assert_eq!(f.from(0x1000), KEY_NONE);
}

#[test]
fn from_virtual_first_valid_wins() {
    let mut f = Fixture::new();

    // Two valid entries with the same code: the first wins.
    f.set_code(k("RETURN"), KVK_RETURN, true);
    f.set_code(k("ENTER"), KVK_RETURN, true);
    assert_eq!(f.from(KVK_RETURN), k("RETURN"));

    // An invalid entry is skipped, even with the code.
    f.set_code(k("B"), KVK_ESCAPE, false);
    f.set_code(k("ESCAPE"), KVK_ESCAPE, true);
    assert_eq!(f.from(KVK_ESCAPE), k("ESCAPE"));

    // The zeroed table's invalid entries all have code 0: none is found.
    assert_eq!(f.from(0), KEY_NONE);

    // OBS_KEY_NONE's own entry and the last entry are searched.
    f.set_code(k("NONE"), KVK_F1, true);
    f.set_code(k("F1"), KVK_F1, true);
    assert_eq!(f.from(KVK_F1), k("NONE"));
    f.set_code(last() - 1, 0x77, true);
    assert_eq!(f.from(0x77), last() - 1);
}

#[test]
fn mouse_range() {
    assert!(ck::is_mouse(OBS_KEY_MOUSE1));
    assert!(ck::is_mouse(k("MOUSE2")));
    assert!(ck::is_mouse(OBS_KEY_MOUSE29));
    assert!(!ck::is_mouse(OBS_KEY_MOUSE1 - 1));
    assert!(!ck::is_mouse(OBS_KEY_MOUSE29 + 1));
    assert!(!ck::is_mouse(k("NONE")));
    assert!(!ck::is_mouse(k("A")));
    assert!(!ck::is_mouse(last()));
}

#[test]
fn key_range() {
    assert!(ck::in_range(k("NONE")));
    assert!(ck::in_range(k("A")));
    assert!(ck::in_range(OBS_KEY_MOUSE29));
    assert!(ck::in_range(last() - 1));
    assert!(!ck::in_range(last()));
    assert!(!ck::in_range(last() + 1));
}

// Edges past the C test.

#[test]
fn modifiers_in_order() {
    assert_eq!(ck::modifiers(0), [false; 4]);
    assert_eq!(ck::modifiers(ALL), [true; 4]);
    assert_eq!(ck::modifiers(SHIFT), [true, false, false, false]);
    assert_eq!(ck::modifiers(ALTERNATE), [false, true, false, false]);
    assert_eq!(ck::modifiers(COMMAND), [false, false, true, false]);
    assert_eq!(ck::modifiers(CONTROL), [false, false, false, true]);
    assert_eq!(ck::modifiers(!ALL), [false; 4]);
}

#[test]
fn modifier_flag_values() {
    // The CoreGraphics masks (NX_SHIFTMASK and its neighbours).
    assert_eq!(FLAG_SHIFT, 1 << 17);
    assert_eq!(FLAG_CONTROL, 1 << 18);
    assert_eq!(FLAG_ALTERNATE, 1 << 19);
    assert_eq!(FLAG_COMMAND, 1 << 20);
    assert_eq!(INVALID, 0xFF);
}

#[test]
fn modifier_negative_and_wide_keys() {
    for key in [-1, i32::MIN, last(), i32::MAX] {
        assert!(!ck::modifier(u64::MAX, key));
    }
}

#[test]
fn to_virtual_outside_the_table() {
    // Intentional difference: C reads past the table.
    let mut f = Fixture::new();
    f.set_code(last() - 1, 0x10, true);
    for key in [last(), last() + 1, i32::MAX, -1, i32::MIN] {
        assert_eq!(f.to(key), INVALID, "key {key}");
    }
    // A shorter table ends where it ends.
    assert_eq!(ck::to_virtual(&[], k("NONE")), INVALID);
    let one = [KeyCode {
        code: 3,
        is_valid: true,
    }];
    assert_eq!(ck::to_virtual(&one, 0), 3);
    assert_eq!(ck::to_virtual(&one, 1), INVALID);
}

#[test]
fn from_virtual_short_and_full_tables() {
    assert_eq!(ck::from_virtual(&[], 0), KEY_NONE);
    let table = [
        KeyCode {
            code: 9,
            is_valid: false,
        },
        KeyCode {
            code: 9,
            is_valid: true,
        },
    ];
    assert_eq!(ck::from_virtual(&table, 9), 1);
    // Every entry valid with its own code: each code finds its key.
    let all: Vec<KeyCode> = (0..last())
        .map(|code| KeyCode {
            code,
            is_valid: true,
        })
        .collect();
    for code in 0..last() {
        assert_eq!(ck::from_virtual(&all, code), code);
    }
    assert_eq!(ck::from_virtual(&all, last()), KEY_NONE);
}

#[test]
fn mouse_bit_per_button() {
    // MOUSE1 is the left button, bit 0; MOUSE29 bit 28.
    for (bit, key) in (OBS_KEY_MOUSE1..=OBS_KEY_MOUSE29).enumerate() {
        assert_eq!(ck::mouse_bit(key), Some(u32::try_from(bit).unwrap()));
    }
    assert_eq!(ck::mouse_bit(OBS_KEY_MOUSE1), Some(0));
    assert_eq!(ck::mouse_bit(k("MOUSE2")), Some(1));
    assert_eq!(ck::mouse_bit(k("MOUSE3")), Some(2));
    assert_eq!(ck::mouse_bit(OBS_KEY_MOUSE29), Some(28));
    for key in [
        OBS_KEY_MOUSE1 - 1,
        OBS_KEY_MOUSE29 + 1,
        k("NONE"),
        k("A"),
        -1,
        i32::MAX,
    ] {
        assert_eq!(ck::mouse_bit(key), None, "key {key}");
    }
}

#[test]
fn mouse_pressed_tests_its_own_bit() {
    // Intentional difference: C tests bit key - 1 of an int (undefined).
    for key in OBS_KEY_MOUSE1..=OBS_KEY_MOUSE29 {
        let bit = ck::mouse_bit(key).unwrap();
        assert!(ck::mouse_pressed(key, 1 << bit), "key {key}");
        assert!(!ck::mouse_pressed(key, !(1u64 << bit)), "key {key}");
        assert!(ck::mouse_pressed(key, u64::MAX));
        assert!(!ck::mouse_pressed(key, 0));
    }
    // The left button down: MOUSE1 only.
    assert!(ck::mouse_pressed(OBS_KEY_MOUSE1, 1));
    assert!(!ck::mouse_pressed(k("MOUSE2"), 1));
    // The right button (bit 1): MOUSE2.
    assert!(ck::mouse_pressed(k("MOUSE2"), 2));
    assert!(!ck::mouse_pressed(OBS_KEY_MOUSE1, 2));
}

#[test]
fn mouse_pressed_ignores_bits_past_mouse29() {
    let high = !((1u64 << 29) - 1);
    for key in OBS_KEY_MOUSE1..=OBS_KEY_MOUSE29 {
        assert!(!ck::mouse_pressed(key, high), "key {key}");
    }
}

#[test]
fn mouse_pressed_never_for_other_keys() {
    for key in [
        k("NONE"),
        k("A"),
        OBS_KEY_SHIFT,
        OBS_KEY_MOUSE29 + 1,
        last(),
        -1,
    ] {
        assert!(!ck::mouse_pressed(key, u64::MAX), "key {key}");
    }
}

#[test]
fn key_range_negative() {
    // obs_key_t is unsigned to gcc and clang, so C's key < LAST is false for
    // these too.
    for key in [-1, i32::MIN, i32::MAX] {
        assert!(!ck::in_range(key));
        assert!(!ck::is_mouse(key));
    }
}

#[test]
fn key_code_layout() {
    // obs_key_code_t: { int code; bool is_valid; }.
    assert_eq!(std::mem::size_of::<KeyCode>(), 8);
    assert_eq!(std::mem::align_of::<KeyCode>(), 4);
    assert_eq!(std::mem::offset_of!(KeyCode, code), 0);
    assert_eq!(std::mem::offset_of!(KeyCode, is_valid), 4);
}

// Models written from the C.

fn model_modifier(flags: u64, key: i32) -> bool {
    let mask = if key == OBS_KEY_SHIFT {
        FLAG_SHIFT
    } else if key == OBS_KEY_ALT {
        FLAG_ALTERNATE
    } else if key == OBS_KEY_META {
        FLAG_COMMAND
    } else if key == OBS_KEY_CONTROL {
        FLAG_CONTROL
    } else {
        return false;
    };
    flags & mask != 0
}

fn model_from(table: &[KeyCode], code: i32) -> i32 {
    for (i, entry) in table.iter().enumerate() {
        if entry.is_valid && entry.code == code {
            return i32::try_from(i).unwrap();
        }
    }
    KEY_NONE
}

fn entry() -> impl Strategy<Value = KeyCode> {
    (
        prop_oneof![4 => 0..0x80i32, 1 => any::<i32>()],
        any::<bool>(),
    )
        .prop_map(|(code, is_valid)| KeyCode { code, is_valid })
}

fn key() -> impl Strategy<Value = i32> {
    prop_oneof![
        8 => 0..i32::try_from(OBS_KEY_LAST_VALUE).unwrap() + 8,
        1 => any::<i32>(),
    ]
}

proptest! {
    #[test]
    fn modifier_matches_model(flags in any::<u64>(), key in key()) {
        prop_assert_eq!(ck::modifier(flags, key), model_modifier(flags, key));
    }

    #[test]
    fn modifiers_match_modifier(flags in any::<u64>()) {
        let want = [OBS_KEY_SHIFT, OBS_KEY_ALT, OBS_KEY_META, OBS_KEY_CONTROL]
            .map(|key| ck::modifier(flags, key));
        prop_assert_eq!(ck::modifiers(flags), want);
    }

    #[test]
    fn to_virtual_matches_model(
        table in proptest::collection::vec(entry(), OBS_KEY_LAST_VALUE),
        key in key(),
    ) {
        let want = usize::try_from(key)
            .ok()
            .and_then(|i| table.get(i))
            .filter(|e| e.is_valid)
            .map_or(INVALID, |e| e.code);
        prop_assert_eq!(ck::to_virtual(&table, key), want);
    }

    #[test]
    fn from_virtual_matches_model(
        table in proptest::collection::vec(entry(), 0..=OBS_KEY_LAST_VALUE),
        code in prop_oneof![4 => 0..0x80i32, 1 => any::<i32>()],
    ) {
        prop_assert_eq!(ck::from_virtual(&table, code), model_from(&table, code));
    }

    #[test]
    fn from_virtual_round_trips(
        table in proptest::collection::vec(entry(), OBS_KEY_LAST_VALUE),
        key in 0..i32::try_from(OBS_KEY_LAST_VALUE).unwrap(),
    ) {
        // A valid key's code finds that key or an earlier one with the code.
        let code = ck::to_virtual(&table, key);
        let found = ck::from_virtual(&table, code);
        if table[usize::try_from(key).unwrap()].is_valid {
            prop_assert!(found <= key);
            prop_assert_eq!(ck::to_virtual(&table, found), code);
        }
    }

    #[test]
    fn mouse_pressed_matches_model(key in key(), buttons in any::<u64>()) {
        let want = (OBS_KEY_MOUSE1..=OBS_KEY_MOUSE29).contains(&key)
            && buttons & (1u64 << (key - OBS_KEY_MOUSE1)) != 0;
        prop_assert_eq!(ck::mouse_pressed(key, buttons), want);
        prop_assert_eq!(ck::is_mouse(key), ck::mouse_bit(key).is_some());
    }

    #[test]
    fn in_range_matches_model(key in key()) {
        let want = usize::try_from(key).is_ok_and(|k| k < OBS_KEY_LAST_VALUE);
        prop_assert_eq!(ck::in_range(key), want);
    }
}
