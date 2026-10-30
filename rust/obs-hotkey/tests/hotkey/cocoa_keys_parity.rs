//! Tier 3 parity: the shims against the original
//! `libobs/obs-cocoa-keys.c.h` (oracle/cocoa_keys.c).
//!
//! `test_cocoa_keys_steps` compiles the C into itself, so it never sees the
//! Rust swap (a variant linked against `libobs_rust-static` does not link on
//! its own: see `test/cmocka/tests.d/test_cocoa_keys_steps.cmake`). This
//! file is what compares the two.
//!
//! Both sides get the same tables, laid out as the C's `obs_key_code_t`.
//! The cases compare:
//! - the key values, flag bits, `INVALID_KEY` and the `obs_key_code_t`
//!   layout against the C;
//! - the modifier map of every key with every combination of the four flag
//!   bits, with the other bits clear and set;
//! - the virtual key of every key, and the key of every 7-bit code and of
//!   wide and negative codes, on four tables (the zeroed one, every key
//!   valid with distinct codes, a macOS-like table with repeated codes, and
//!   every other entry invalid);
//! - the mouse range and the key range gate of every key from 0 to twice
//!   `OBS_KEY_LAST_VALUE`, and of the largest keys;
//! - proptests over random flag words, random tables and random keys.
//!
//! Keys are passed from 0 up only: `obs_key_t` is unsigned to gcc and clang
//! (macOS builds it with clang), so a negative `int` reaches the C as a huge
//! key, but MSVC makes it signed, and the oracle is also built there.
//!
//! Intentional differences, excluded here and checked on the Rust side only
//! (`cocoa_keys.rs`, `to_virtual_outside_the_table` and
//! `mouse_pressed_tests_its_own_bit`):
//! - the virtual key of a key outside `0 .. OBS_KEY_LAST_VALUE - 1`: C reads
//!   past the table; Rust gives `INVALID_KEY`;
//! - the button bit of a mouse key: C shifts an `int` by the key's value
//!   less one, which is undefined for every mouse key, so the oracle's
//!   `oracle_obs_cocoa_keys_mouse_pressed` is never called; Rust tests bit
//!   `key - OBS_KEY_MOUSE1`.
//!
//! A null table, which C dereferences, is not passed. Nothing else is
//! excluded.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::{c_int, c_void};

use obs_c_oracle::cocoa_keys::{
    LAYOUT_NAMES, VALUE_NAMES, oracle_cocoa_keys_layout, oracle_cocoa_keys_value,
    oracle_obs_cocoa_keys_from_virtual_key, oracle_obs_cocoa_keys_in_range,
    oracle_obs_cocoa_keys_is_mouse, oracle_obs_cocoa_keys_modifier,
    oracle_obs_cocoa_keys_to_virtual_key,
};
use obs_hotkey::cocoa_keys::{
    FLAG_ALTERNATE, FLAG_COMMAND, FLAG_CONTROL, FLAG_SHIFT, INVALID, KeyCode, OBS_KEY_ALT,
    OBS_KEY_CONTROL, OBS_KEY_LAST_VALUE, OBS_KEY_META, OBS_KEY_MOUSE1, OBS_KEY_MOUSE29,
    OBS_KEY_SHIFT,
};
use obs_hotkey::ffi::cocoa_keys::{
    obs_cocoa_keys_from_virtual_key, obs_cocoa_keys_in_range, obs_cocoa_keys_is_mouse,
    obs_cocoa_keys_modifier, obs_cocoa_keys_to_virtual_key,
};
use obs_hotkey::name_map::KEY_NONE;
use proptest::prelude::*;

const FLAGS: [u64; 4] = [FLAG_SHIFT, FLAG_CONTROL, FLAG_ALTERNATE, FLAG_COMMAND];

fn last() -> c_int {
    c_int::try_from(OBS_KEY_LAST_VALUE).unwrap()
}

fn rust_to(table: &[KeyCode], key: c_int) -> c_int {
    assert_eq!(table.len(), OBS_KEY_LAST_VALUE);
    // SAFETY: OBS_KEY_LAST_VALUE entries, alive for the call.
    unsafe { obs_cocoa_keys_to_virtual_key(table.as_ptr(), key) }
}

fn c_to(table: &[KeyCode], key: c_int) -> c_int {
    assert_eq!(table.len(), OBS_KEY_LAST_VALUE);
    assert!((0..last()).contains(&key), "C reads past the table");
    // SAFETY: as rust_to; KeyCode is obs_key_code_t (checked below).
    unsafe { oracle_obs_cocoa_keys_to_virtual_key(table.as_ptr().cast::<c_void>(), key) }
}

fn rust_from(table: &[KeyCode], code: c_int) -> c_int {
    assert_eq!(table.len(), OBS_KEY_LAST_VALUE);
    // SAFETY: OBS_KEY_LAST_VALUE entries, alive for the call.
    unsafe { obs_cocoa_keys_from_virtual_key(table.as_ptr(), code) }
}

fn c_from(table: &[KeyCode], code: c_int) -> c_int {
    assert_eq!(table.len(), OBS_KEY_LAST_VALUE);
    // SAFETY: as rust_from.
    unsafe { oracle_obs_cocoa_keys_from_virtual_key(table.as_ptr().cast::<c_void>(), code) }
}

fn c_modifier(flags: u64, key: c_int) -> bool {
    // SAFETY: the oracle reads only its arguments.
    unsafe { oracle_obs_cocoa_keys_modifier(flags, key) }
}

fn c_is_mouse(key: c_int) -> bool {
    // SAFETY: the oracle reads only its argument.
    unsafe { oracle_obs_cocoa_keys_is_mouse(key) }
}

fn c_in_range(key: c_int) -> bool {
    // SAFETY: the oracle reads only its argument.
    unsafe { oracle_obs_cocoa_keys_in_range(key) }
}

/// Compares both lookups on `table`: every key, and every 7-bit code with
/// wide and negative codes.
fn compare_table(table: &[KeyCode]) {
    for key in 0..last() {
        assert_eq!(
            rust_to(table, key),
            c_to(table, key),
            "to_virtual key {key}"
        );
    }
    let wide = [-1, c_int::MIN, 0x80, 0xFF, 0x100, 0x1000, c_int::MAX];
    for code in (0..0x80).chain(wide) {
        assert_eq!(
            rust_from(table, code),
            c_from(table, code),
            "from_virtual code {code}"
        );
    }
}

fn zeroed() -> Vec<KeyCode> {
    vec![KeyCode::default(); OBS_KEY_LAST_VALUE]
}

#[test]
fn values_and_layout_match_the_c() {
    let rust: [i64; 13] = [
        i64::try_from(OBS_KEY_LAST_VALUE).unwrap(),
        i64::from(KEY_NONE),
        i64::from(OBS_KEY_SHIFT),
        i64::from(OBS_KEY_CONTROL),
        i64::from(OBS_KEY_ALT),
        i64::from(OBS_KEY_META),
        i64::from(OBS_KEY_MOUSE1),
        i64::from(OBS_KEY_MOUSE29),
        i64::try_from(FLAG_SHIFT).unwrap(),
        i64::try_from(FLAG_CONTROL).unwrap(),
        i64::try_from(FLAG_ALTERNATE).unwrap(),
        i64::try_from(FLAG_COMMAND).unwrap(),
        i64::from(INVALID),
    ];
    for (which, (name, want)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: the getter reads a table of constants.
        assert_eq!(unsafe { oracle_cocoa_keys_value(which) }, want, "{name}");
    }
    let layout = [
        std::mem::size_of::<KeyCode>(),
        std::mem::align_of::<KeyCode>(),
        std::mem::offset_of!(KeyCode, code),
        std::mem::offset_of!(KeyCode, is_valid),
    ];
    for (which, (name, want)) in LAYOUT_NAMES.iter().zip(layout).enumerate() {
        // SAFETY: the getter reads a table of constants.
        assert_eq!(unsafe { oracle_cocoa_keys_layout(which) }, want, "{name}");
    }
}

#[test]
fn modifier_of_every_key_and_flag_combination() {
    for combo in 0..16u32 {
        let mut flags = 0;
        for (bit, flag) in FLAGS.iter().enumerate() {
            if combo & (1 << bit) != 0 {
                flags |= flag;
            }
        }
        let all = FLAGS.iter().fold(0, |acc, f| acc | f);
        for word in [flags, flags | !all] {
            for key in 0..last() + 8 {
                assert_eq!(
                    obs_cocoa_keys_modifier(word, key),
                    c_modifier(word, key),
                    "flags {word:#x} key {key}"
                );
            }
        }
    }
}

#[test]
fn lookups_on_fixed_tables() {
    // The zeroed table: every entry invalid with code 0.
    compare_table(&zeroed());

    // Every key valid, with its own code.
    let distinct: Vec<KeyCode> = (0..last())
        .map(|key| KeyCode {
            code: key,
            is_valid: true,
        })
        .collect();
    compare_table(&distinct);

    // macOS-like: the first 128 keys take the 7-bit codes, the next 128 the
    // same codes again (the first copy wins), the rest none.
    let repeated: Vec<KeyCode> = (0..last())
        .map(|key| KeyCode {
            code: key % 0x80,
            is_valid: key < 0x100,
        })
        .collect();
    compare_table(&repeated);

    // Every other entry invalid, with codes that repeat every 50 keys.
    let alternating: Vec<KeyCode> = (0..last())
        .map(|key| KeyCode {
            code: key % 50,
            is_valid: key % 2 == 1,
        })
        .collect();
    compare_table(&alternating);
}

#[test]
fn ranges_of_every_key() {
    let large = [c_int::MAX, c_int::MAX - 1, 1 << 20, 0x7FFF];
    for key in (0..2 * last()).chain(large) {
        assert_eq!(
            obs_cocoa_keys_is_mouse(key),
            c_is_mouse(key),
            "is_mouse {key}"
        );
        assert_eq!(
            obs_cocoa_keys_in_range(key),
            c_in_range(key),
            "in_range {key}"
        );
    }
}

fn entry() -> impl Strategy<Value = KeyCode> {
    (
        prop_oneof![8 => 0..0x80, 1 => any::<c_int>()],
        any::<bool>(),
    )
        .prop_map(|(code, is_valid)| KeyCode { code, is_valid })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(64)))]

    #[test]
    fn modifier_matches_the_c(flags in any::<u64>(), key in 0..last() + 8) {
        prop_assert_eq!(obs_cocoa_keys_modifier(flags, key), c_modifier(flags, key));
    }

    #[test]
    fn lookups_match_the_c(
        table in proptest::collection::vec(entry(), OBS_KEY_LAST_VALUE),
        keys in proptest::collection::vec(0..last(), 32),
        codes in proptest::collection::vec(prop_oneof![8 => 0..0x80, 1 => any::<c_int>()], 32),
    ) {
        for key in keys {
            prop_assert_eq!(rust_to(&table, key), c_to(&table, key));
        }
        for code in codes {
            prop_assert_eq!(rust_from(&table, code), c_from(&table, code));
        }
        // A code taken from the table itself is always found.
        for entry in table.iter().filter(|e| e.is_valid).take(8) {
            prop_assert_eq!(rust_from(&table, entry.code), c_from(&table, entry.code));
        }
    }

    #[test]
    fn ranges_match_the_c(key in 0..=c_int::MAX) {
        prop_assert_eq!(obs_cocoa_keys_is_mouse(key), c_is_mouse(key));
        prop_assert_eq!(obs_cocoa_keys_in_range(key), c_in_range(key));
    }
}
