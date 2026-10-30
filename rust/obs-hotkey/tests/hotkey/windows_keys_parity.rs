//! Tier 3: the Windows key map shims against the original C
//! (`libobs/obs-windows-keys.c.h`, built as `oracle_*` by obs-c-oracle).
//!
//! Every case calls the `#[no_mangle]` shim and the oracle with the same
//! inputs and compares the results, the number and keys written, and how
//! often the table is asked for:
//! - the values both sides use, and every virtual key by name;
//! - the virtual key, the Meta rule, the name route and the system name
//!   gate of every key from -2 to past `OBS_KEY_LAST_VALUE`, and of the
//!   largest and smallest;
//! - the scan code of every virtual key from -2 to 0x1FF on scan codes with
//!   and without high bits;
//! - the key of every virtual key from -2 to 0x1FF, and the virtual key of
//!   every key, on four tables (the platform's, zeroed, every key distinct,
//!   and one with repeats);
//! - the combination of every 9-bit modifier mask with no key, a modifier
//!   key, an ordinary key and the last key;
//! - proptests over random keys, virtual keys, scan codes, modifier words
//!   and tables.
//!
//! Negative keys go to both sides: `obs_key_t` is unsigned to GCC and
//! Clang and signed to MSVC, which also builds the oracle, and every
//! function compared here gives the same answer either way.
//!
//! Intentional difference, excluded and checked on the Rust side only: the
//! virtual key of a key outside `0 .. OBS_KEY_LAST_VALUE`, which C reads
//! past the table for and Rust gives 0.

use std::cell::{Cell, RefCell};
use std::ffi::c_int;

use obs_c_oracle::windows_keys::{
    VALUE_NAMES, VK_NAMES, oracle_obs_windows_keys_combination, oracle_obs_windows_keys_either_win,
    oracle_obs_windows_keys_from_virtual_key, oracle_obs_windows_keys_name,
    oracle_obs_windows_keys_scan_code, oracle_obs_windows_keys_system_name,
    oracle_obs_windows_keys_to_virtual_key, oracle_obs_windows_keys_virtual_key,
    oracle_windows_keys_value, oracle_windows_keys_vk,
};
use obs_hotkey::ffi::windows_keys::{
    obs_windows_keys_combination, obs_windows_keys_either_win, obs_windows_keys_from_virtual_key,
    obs_windows_keys_name, obs_windows_keys_scan_code, obs_windows_keys_system_name,
    obs_windows_keys_to_virtual_key, obs_windows_keys_virtual_key,
};
#[allow(clippy::wildcard_imports)]
use obs_hotkey::name_map::*;
use obs_hotkey::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
};
use obs_hotkey::windows_keys::{self as wk, COMBINATION_MAX, EXTENDED};
use proptest::prelude::*;

/// `OBS_KEY_LAST_VALUE` as a key.
fn last() -> i32 {
    i32::try_from(OBS_KEY_LAST_VALUE).unwrap()
}

/// Every key from -2 to two past the last, and the extremes.
fn keys() -> impl Iterator<Item = i32> {
    (-2..last() + 2).chain([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX])
}

/// Every virtual key from -2 to 0x1FF, and the extremes.
fn codes() -> impl Iterator<Item = i32> {
    (-2..0x200).chain([i32::MIN, i32::MAX])
}

thread_local! {
    /// The table the getter returns, and how often it was asked for.
    static TABLE: RefCell<Vec<c_int>> = const { RefCell::new(Vec::new()) };
    static ASKED: Cell<u32> = const { Cell::new(0) };
}

/// `obs_windows_keys_table_fn` over [`TABLE`].
unsafe extern "C" fn table() -> *const c_int {
    ASKED.with(|a| a.set(a.get() + 1));
    TABLE.with(|t| t.borrow().as_ptr())
}

/// Calls `f` with `vk_codes` as the getter's table, and returns its result
/// and how often the table was asked for.
fn with_table<R>(vk_codes: &[c_int], f: impl FnOnce() -> R) -> (R, u32) {
    TABLE.with(|t| *t.borrow_mut() = vk_codes.to_vec());
    ASKED.with(|a| a.set(0));
    let result = f();
    (result, ASKED.with(Cell::get))
}

/// The four tables: the platform's, zeroed, every key its own index, and
/// codes 0 to 7 repeating.
fn tables() -> Vec<Vec<c_int>> {
    vec![
        (0..last()).map(wk::virtual_key).collect(),
        vec![0; OBS_KEY_LAST_VALUE],
        (0..last()).collect(),
        (0..last()).map(|k| k % 8).collect(),
    ]
}

/// The route and the number each side writes, from a sentinel of -7.
fn names(key: i32) -> ((c_int, c_int), (c_int, c_int)) {
    let mut ours = -7;
    let mut theirs = -7;
    // SAFETY: both write at most one int to a live local.
    let a = unsafe { obs_windows_keys_name(key, &mut ours) };
    // SAFETY: as above.
    let b = unsafe { oracle_obs_windows_keys_name(key, &mut theirs) };
    ((a, ours), (b, theirs))
}

/// The keys each side names for a combination, from a sentinel table.
fn combos(modifiers: u32, key: i32) -> (Vec<c_int>, Vec<c_int>) {
    let mut ours = [-7; COMBINATION_MAX];
    let mut theirs = [-7; COMBINATION_MAX];
    // SAFETY: both write at most COMBINATION_MAX keys.
    let a = unsafe { obs_windows_keys_combination(modifiers, key, ours.as_mut_ptr()) };
    // SAFETY: as above.
    let b = unsafe { oracle_obs_windows_keys_combination(modifiers, key, theirs.as_mut_ptr()) };
    (ours[..a].to_vec(), theirs[..b].to_vec())
}

/// Every virtual key the Rust side names, by name, in [`VK_NAMES`]' order.
const RUST_VKS: &[(&str, i32)] = &[
    ("VK_ACCEPT", wk::VK_ACCEPT),
    ("VK_ADD", wk::VK_ADD),
    ("VK_APPS", wk::VK_APPS),
    ("VK_ATTN", wk::VK_ATTN),
    ("VK_BACK", wk::VK_BACK),
    ("VK_BROWSER_BACK", wk::VK_BROWSER_BACK),
    ("VK_BROWSER_FAVORITES", wk::VK_BROWSER_FAVORITES),
    ("VK_BROWSER_FORWARD", wk::VK_BROWSER_FORWARD),
    ("VK_BROWSER_HOME", wk::VK_BROWSER_HOME),
    ("VK_BROWSER_REFRESH", wk::VK_BROWSER_REFRESH),
    ("VK_BROWSER_SEARCH", wk::VK_BROWSER_SEARCH),
    ("VK_BROWSER_STOP", wk::VK_BROWSER_STOP),
    ("VK_CANCEL", wk::VK_CANCEL),
    ("VK_CAPITAL", wk::VK_CAPITAL),
    ("VK_CLEAR", wk::VK_CLEAR),
    ("VK_CONTROL", wk::VK_CONTROL),
    ("VK_CONVERT", wk::VK_CONVERT),
    ("VK_CRSEL", wk::VK_CRSEL),
    ("VK_DECIMAL", wk::VK_DECIMAL),
    ("VK_DELETE", wk::VK_DELETE),
    ("VK_DIVIDE", wk::VK_DIVIDE),
    ("VK_DOWN", wk::VK_DOWN),
    ("VK_END", wk::VK_END),
    ("VK_EREOF", wk::VK_EREOF),
    ("VK_ESCAPE", wk::VK_ESCAPE),
    ("VK_EXECUTE", wk::VK_EXECUTE),
    ("VK_EXSEL", wk::VK_EXSEL),
    ("VK_F10", wk::VK_F10),
    ("VK_F1", wk::VK_F1),
    ("VK_F11", wk::VK_F11),
    ("VK_F12", wk::VK_F12),
    ("VK_F13", wk::VK_F13),
    ("VK_F14", wk::VK_F14),
    ("VK_F15", wk::VK_F15),
    ("VK_F16", wk::VK_F16),
    ("VK_F17", wk::VK_F17),
    ("VK_F18", wk::VK_F18),
    ("VK_F19", wk::VK_F19),
    ("VK_F20", wk::VK_F20),
    ("VK_F2", wk::VK_F2),
    ("VK_F21", wk::VK_F21),
    ("VK_F22", wk::VK_F22),
    ("VK_F23", wk::VK_F23),
    ("VK_F24", wk::VK_F24),
    ("VK_F3", wk::VK_F3),
    ("VK_F4", wk::VK_F4),
    ("VK_F5", wk::VK_F5),
    ("VK_F6", wk::VK_F6),
    ("VK_F7", wk::VK_F7),
    ("VK_F8", wk::VK_F8),
    ("VK_F9", wk::VK_F9),
    ("VK_FINAL", wk::VK_FINAL),
    ("VK_HANGUL", wk::VK_HANGUL),
    ("VK_HELP", wk::VK_HELP),
    ("VK_HOME", wk::VK_HOME),
    ("VK_ICO_00", wk::VK_ICO_00),
    ("VK_ICO_CLEAR", wk::VK_ICO_CLEAR),
    ("VK_ICO_HELP", wk::VK_ICO_HELP),
    ("VK_INSERT", wk::VK_INSERT),
    ("VK_JUNJA", wk::VK_JUNJA),
    ("VK_KANJI", wk::VK_KANJI),
    ("VK_LAUNCH_APP1", wk::VK_LAUNCH_APP1),
    ("VK_LAUNCH_APP2", wk::VK_LAUNCH_APP2),
    ("VK_LAUNCH_MAIL", wk::VK_LAUNCH_MAIL),
    ("VK_LAUNCH_MEDIA_SELECT", wk::VK_LAUNCH_MEDIA_SELECT),
    ("VK_LBUTTON", wk::VK_LBUTTON),
    ("VK_LCONTROL", wk::VK_LCONTROL),
    ("VK_LEFT", wk::VK_LEFT),
    ("VK_LMENU", wk::VK_LMENU),
    ("VK_LSHIFT", wk::VK_LSHIFT),
    ("VK_LWIN", wk::VK_LWIN),
    ("VK_MBUTTON", wk::VK_MBUTTON),
    ("VK_MEDIA_NEXT_TRACK", wk::VK_MEDIA_NEXT_TRACK),
    ("VK_MEDIA_PLAY_PAUSE", wk::VK_MEDIA_PLAY_PAUSE),
    ("VK_MEDIA_PREV_TRACK", wk::VK_MEDIA_PREV_TRACK),
    ("VK_MEDIA_STOP", wk::VK_MEDIA_STOP),
    ("VK_MENU", wk::VK_MENU),
    ("VK_MODECHANGE", wk::VK_MODECHANGE),
    ("VK_MULTIPLY", wk::VK_MULTIPLY),
    ("VK_NEXT", wk::VK_NEXT),
    ("VK_NONAME", wk::VK_NONAME),
    ("VK_NONCONVERT", wk::VK_NONCONVERT),
    ("VK_NUMLOCK", wk::VK_NUMLOCK),
    ("VK_NUMPAD0", wk::VK_NUMPAD0),
    ("VK_NUMPAD1", wk::VK_NUMPAD1),
    ("VK_NUMPAD2", wk::VK_NUMPAD2),
    ("VK_NUMPAD3", wk::VK_NUMPAD3),
    ("VK_NUMPAD4", wk::VK_NUMPAD4),
    ("VK_NUMPAD5", wk::VK_NUMPAD5),
    ("VK_NUMPAD6", wk::VK_NUMPAD6),
    ("VK_NUMPAD7", wk::VK_NUMPAD7),
    ("VK_NUMPAD8", wk::VK_NUMPAD8),
    ("VK_NUMPAD9", wk::VK_NUMPAD9),
    ("VK_OEM_102", wk::VK_OEM_102),
    ("VK_OEM_1", wk::VK_OEM_1),
    ("VK_OEM_2", wk::VK_OEM_2),
    ("VK_OEM_3", wk::VK_OEM_3),
    ("VK_OEM_4", wk::VK_OEM_4),
    ("VK_OEM_5", wk::VK_OEM_5),
    ("VK_OEM_6", wk::VK_OEM_6),
    ("VK_OEM_7", wk::VK_OEM_7),
    ("VK_OEM_8", wk::VK_OEM_8),
    ("VK_OEM_ATTN", wk::VK_OEM_ATTN),
    ("VK_OEM_AUTO", wk::VK_OEM_AUTO),
    ("VK_OEM_AX", wk::VK_OEM_AX),
    ("VK_OEM_BACKTAB", wk::VK_OEM_BACKTAB),
    ("VK_OEM_CLEAR", wk::VK_OEM_CLEAR),
    ("VK_OEM_COMMA", wk::VK_OEM_COMMA),
    ("VK_OEM_COPY", wk::VK_OEM_COPY),
    ("VK_OEM_CUSEL", wk::VK_OEM_CUSEL),
    ("VK_OEM_ENLW", wk::VK_OEM_ENLW),
    ("VK_OEM_FINISH", wk::VK_OEM_FINISH),
    ("VK_OEM_FJ_JISHO", wk::VK_OEM_FJ_JISHO),
    ("VK_OEM_FJ_LOYA", wk::VK_OEM_FJ_LOYA),
    ("VK_OEM_FJ_MASSHOU", wk::VK_OEM_FJ_MASSHOU),
    ("VK_OEM_FJ_ROYA", wk::VK_OEM_FJ_ROYA),
    ("VK_OEM_FJ_TOUROKU", wk::VK_OEM_FJ_TOUROKU),
    ("VK_OEM_JUMP", wk::VK_OEM_JUMP),
    ("VK_OEM_MINUS", wk::VK_OEM_MINUS),
    ("VK_OEM_PA1", wk::VK_OEM_PA1),
    ("VK_OEM_PA2", wk::VK_OEM_PA2),
    ("VK_OEM_PA3", wk::VK_OEM_PA3),
    ("VK_OEM_PERIOD", wk::VK_OEM_PERIOD),
    ("VK_OEM_PLUS", wk::VK_OEM_PLUS),
    ("VK_OEM_RESET", wk::VK_OEM_RESET),
    ("VK_OEM_WSCTRL", wk::VK_OEM_WSCTRL),
    ("VK_PA1", wk::VK_PA1),
    ("VK_PACKET", wk::VK_PACKET),
    ("VK_PAUSE", wk::VK_PAUSE),
    ("VK_PLAY", wk::VK_PLAY),
    ("VK_PRINT", wk::VK_PRINT),
    ("VK_PRIOR", wk::VK_PRIOR),
    ("VK_PROCESSKEY", wk::VK_PROCESSKEY),
    ("VK_RBUTTON", wk::VK_RBUTTON),
    ("VK_RCONTROL", wk::VK_RCONTROL),
    ("VK_RETURN", wk::VK_RETURN),
    ("VK_RIGHT", wk::VK_RIGHT),
    ("VK_RMENU", wk::VK_RMENU),
    ("VK_RSHIFT", wk::VK_RSHIFT),
    ("VK_RWIN", wk::VK_RWIN),
    ("VK_SCROLL", wk::VK_SCROLL),
    ("VK_SELECT", wk::VK_SELECT),
    ("VK_SEPARATOR", wk::VK_SEPARATOR),
    ("VK_SHIFT", wk::VK_SHIFT),
    ("VK_SLEEP", wk::VK_SLEEP),
    ("VK_SNAPSHOT", wk::VK_SNAPSHOT),
    ("VK_SPACE", wk::VK_SPACE),
    ("VK_SUBTRACT", wk::VK_SUBTRACT),
    ("VK_TAB", wk::VK_TAB),
    ("VK_UP", wk::VK_UP),
    ("VK_VOLUME_DOWN", wk::VK_VOLUME_DOWN),
    ("VK_VOLUME_MUTE", wk::VK_VOLUME_MUTE),
    ("VK_VOLUME_UP", wk::VK_VOLUME_UP),
    ("VK_XBUTTON1", wk::VK_XBUTTON1),
    ("VK_XBUTTON2", wk::VK_XBUTTON2),
    ("VK_ZOOM", wk::VK_ZOOM),
];

#[test]
fn values_match_the_c() {
    let rust: [i64; 30] = [
        i64::try_from(OBS_KEY_LAST_VALUE).unwrap(),
        OBS_KEY_NONE.into(),
        OBS_KEY_SHIFT.into(),
        OBS_KEY_CONTROL.into(),
        OBS_KEY_ALT.into(),
        OBS_KEY_META.into(),
        OBS_KEY_PAUSE.into(),
        OBS_KEY_F13.into(),
        OBS_KEY_F24.into(),
        OBS_KEY_MOUSE1.into(),
        OBS_KEY_MOUSE29.into(),
        OBS_KEY_VK_CANCEL.into(),
        OBS_KEY_VK_OEM_CLEAR.into(),
        1,
        INTERACT_SHIFT_KEY.into(),
        INTERACT_CONTROL_KEY.into(),
        INTERACT_ALT_KEY.into(),
        INTERACT_COMMAND_KEY.into(),
        1 << 8,
        EXTENDED.into(),
        i64::try_from(COMBINATION_MAX).unwrap(),
        obs_hotkey::ffi::windows_keys::OBS_WINDOWS_KEYS_NAME_NONE.into(),
        obs_hotkey::ffi::windows_keys::OBS_WINDOWS_KEYS_NAME_F.into(),
        obs_hotkey::ffi::windows_keys::OBS_WINDOWS_KEYS_NAME_MOUSE.into(),
        obs_hotkey::ffi::windows_keys::OBS_WINDOWS_KEYS_NAME_PAUSE.into(),
        obs_hotkey::ffi::windows_keys::OBS_WINDOWS_KEYS_NAME_META.into(),
        obs_hotkey::ffi::windows_keys::OBS_WINDOWS_KEYS_NAME_SYSTEM.into(),
        4,
        4,
        i64::try_from(size_of::<usize>()).unwrap(),
    ];
    for (i, (name, value)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter.
        assert_eq!(unsafe { oracle_windows_keys_value(i) }, value, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(unsafe { oracle_windows_keys_value(VALUE_NAMES.len()) }, -1);
}

#[test]
fn virtual_keys_match_the_c() {
    assert_eq!(RUST_VKS.len(), VK_NAMES.len());
    for (i, (&(name, value), &c_name)) in RUST_VKS.iter().zip(&VK_NAMES).enumerate() {
        assert_eq!(name, c_name);
        // SAFETY: a plain getter.
        assert_eq!(unsafe { oracle_windows_keys_vk(i) }, value, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(unsafe { oracle_windows_keys_vk(VK_NAMES.len()) }, -1);
}

#[test]
fn every_key() {
    for key in keys() {
        // SAFETY: neither reads memory.
        let (vk, c_vk) = unsafe {
            (
                obs_windows_keys_virtual_key(key),
                oracle_obs_windows_keys_virtual_key(key),
            )
        };
        assert_eq!(vk, c_vk, "virtual key of {key}");
        // SAFETY: as above.
        let (win, c_win) = unsafe {
            (
                obs_windows_keys_either_win(key),
                oracle_obs_windows_keys_either_win(key),
            )
        };
        assert_eq!(win, c_win, "Meta rule of {key}");
        let (ours, theirs) = names(key);
        assert_eq!(ours, theirs, "name of {key}");
        for scan in [0, 1, 0x0045_0000, EXTENDED, u32::MAX] {
            // SAFETY: neither reads memory.
            let (gate, c_gate) = unsafe {
                (
                    obs_windows_keys_system_name(key, scan),
                    oracle_obs_windows_keys_system_name(key, scan),
                )
            };
            assert_eq!(gate, c_gate, "gate of {key}, {scan:#x}");
        }
    }
}

#[test]
fn every_scan_code() {
    for vk in codes() {
        for mapped in [
            0,
            1,
            0x1C,
            0x45,
            0xFF,
            0x100,
            0xFFFF,
            0x1_0000,
            0xFFFF_0001,
            u32::MAX,
        ] {
            // SAFETY: neither reads memory.
            let (ours, theirs) = unsafe {
                (
                    obs_windows_keys_scan_code(vk, mapped),
                    oracle_obs_windows_keys_scan_code(vk, mapped),
                )
            };
            assert_eq!(ours, theirs, "vk {vk}, scan code {mapped:#x}");
        }
    }
}

#[test]
fn every_lookup_on_four_tables() {
    for vk_codes in tables() {
        for code in codes() {
            // SAFETY: both read OBS_KEY_LAST_VALUE ints of a live table.
            let (ours, theirs) = unsafe {
                (
                    obs_windows_keys_from_virtual_key(vk_codes.as_ptr(), code),
                    oracle_obs_windows_keys_from_virtual_key(vk_codes.as_ptr(), code),
                )
            };
            assert_eq!(ours, theirs, "key of {code}");
        }
        // Keys inside the table only: C reads past it for any other.
        for key in 0..last() {
            // SAFETY: the getter returns the live table, and the key is
            // inside it.
            let ours = with_table(&vk_codes, || unsafe {
                obs_windows_keys_to_virtual_key(key, Some(table))
            });
            // SAFETY: as above.
            let theirs = with_table(&vk_codes, || unsafe {
                oracle_obs_windows_keys_to_virtual_key(key, Some(table))
            });
            assert_eq!(ours, theirs, "virtual key of {key}");
        }
    }
}

#[test]
fn to_virtual_outside_the_table() {
    // The intentional difference, on the Rust side only: 0, with the table
    // asked for once, as C asks for it before reading past it.
    let vk_codes = tables().remove(0);
    for key in [-2, -1, i32::MIN, last(), last() + 1, i32::MAX] {
        // SAFETY: the getter returns the live table; the shim bounds the key.
        let ours = with_table(&vk_codes, || unsafe {
            obs_windows_keys_to_virtual_key(key, Some(table))
        });
        assert_eq!(ours, (0, 1), "key {key}");
    }
    // A null getter reads as an empty table (C calls it); Meta needs none.
    // SAFETY: a null getter is never called.
    unsafe {
        assert_eq!(obs_windows_keys_to_virtual_key(OBS_KEY_A, None), 0);
        assert_eq!(obs_windows_keys_to_virtual_key(OBS_KEY_META, None), 0x5B);
        assert_eq!(obs_windows_keys_from_virtual_key(std::ptr::null(), 0), 0);
        assert_eq!(obs_windows_keys_name(OBS_KEY_F13, std::ptr::null_mut()), 1);
        assert_eq!(
            obs_windows_keys_combination(!0, OBS_KEY_A, std::ptr::null_mut()),
            0
        );
    }
}

#[test]
fn every_combination() {
    for modifiers in 0..0x200u32 {
        for key in [OBS_KEY_NONE, OBS_KEY_SHIFT, OBS_KEY_A, last() - 1] {
            let (ours, theirs) = combos(modifiers, key);
            assert_eq!(ours, theirs, "modifiers {modifiers:#x}, key {key}");
        }
    }
    let (ours, theirs) = combos(u32::MAX, OBS_KEY_F13);
    assert_eq!(ours, theirs);
}

/// A key: any `i32`, one around the enum, or one at an edge of a range.
fn key() -> impl Strategy<Value = i32> {
    prop_oneof![
        any::<i32>(),
        -2..last() + 2,
        prop::sample::select(vec![
            OBS_KEY_NONE,
            OBS_KEY_META,
            OBS_KEY_PAUSE,
            OBS_KEY_F13 - 1,
            OBS_KEY_F13,
            OBS_KEY_F24,
            OBS_KEY_F24 + 1,
            OBS_KEY_MOUSE1 - 1,
            OBS_KEY_MOUSE1,
            OBS_KEY_MOUSE29,
            OBS_KEY_MOUSE29 + 1,
            OBS_KEY_VK_CANCEL - 1,
            OBS_KEY_VK_CANCEL,
            OBS_KEY_VK_OEM_CLEAR,
            OBS_KEY_VK_OEM_CLEAR + 1,
        ]),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn random_keys(key in key(), scan in any::<u32>(), vk in any::<i32>(), modifiers in any::<u32>()) {
        // SAFETY: none of these read memory.
        unsafe {
            prop_assert_eq!(
                obs_windows_keys_virtual_key(key),
                oracle_obs_windows_keys_virtual_key(key)
            );
            prop_assert_eq!(
                obs_windows_keys_either_win(key),
                oracle_obs_windows_keys_either_win(key)
            );
            prop_assert_eq!(
                obs_windows_keys_system_name(key, scan),
                oracle_obs_windows_keys_system_name(key, scan)
            );
            prop_assert_eq!(
                obs_windows_keys_scan_code(vk, scan),
                oracle_obs_windows_keys_scan_code(vk, scan)
            );
        }
        let (ours, theirs) = names(key);
        prop_assert_eq!(ours, theirs);
        let (ours, theirs) = combos(modifiers, key);
        prop_assert_eq!(ours, theirs);
    }

    #[test]
    fn random_tables(
        vk_codes in proptest::collection::vec(-2i32..6, OBS_KEY_LAST_VALUE),
        code in -3i32..7,
        key in 0..last(),
    ) {
        // SAFETY: both read OBS_KEY_LAST_VALUE ints of a live table.
        let (ours, theirs) = unsafe {
            (
                obs_windows_keys_from_virtual_key(vk_codes.as_ptr(), code),
                oracle_obs_windows_keys_from_virtual_key(vk_codes.as_ptr(), code),
            )
        };
        prop_assert_eq!(ours, theirs);
        // SAFETY: the getter returns the live table; the key is inside it.
        let ours = with_table(&vk_codes, || unsafe {
            obs_windows_keys_to_virtual_key(key, Some(table))
        });
        // SAFETY: as above.
        let theirs = with_table(&vk_codes, || unsafe {
            oracle_obs_windows_keys_to_virtual_key(key, Some(table))
        });
        prop_assert_eq!(ours, theirs);
    }
}
