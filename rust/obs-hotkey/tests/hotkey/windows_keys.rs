//! Tier 1: the safe core of `libobs/obs-windows-keys.c.h`.
//!
//! The first nine tests port `test/cmocka/test_windows_keys_steps.c` one to
//! one, with the same names, inputs and expectations, in the same order.
//! The two that read the table start from the C test's `fill()`: the table
//! `obs_hotkeys_platform_init()` builds ([`Fixture::new`]).
//!
//! The cases after those pin the edges of each function and the fix (the
//! virtual key of a key outside the table), and the proptests check each
//! function against a model written from the C.

use std::cell::Cell;

#[allow(clippy::wildcard_imports)]
use obs_hotkey::name_map::*;
use obs_hotkey::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
};
use obs_hotkey::windows_keys::{self as wk, COMBINATION_MAX, EXTENDED, Name, VK_LWIN};
use proptest::prelude::*;

/// Every key the table maps, with its virtual key (winuser.h's `VK_*`
/// value, or the raw code of an `OBS_KEY_0x*` key). Every other key maps
/// to 0.
const MAPPED: &[(i32, i32)] = &[
    (OBS_KEY_RETURN, 0x0D),
    (OBS_KEY_ESCAPE, 0x1B),
    (OBS_KEY_TAB, 0x09),
    (OBS_KEY_BACKTAB, 0xF5),
    (OBS_KEY_BACKSPACE, 0x08),
    (OBS_KEY_INSERT, 0x2D),
    (OBS_KEY_DELETE, 0x2E),
    (OBS_KEY_PAUSE, 0x13),
    (OBS_KEY_PRINT, 0x2C),
    (OBS_KEY_CLEAR, 0x0C),
    (OBS_KEY_HOME, 0x24),
    (OBS_KEY_END, 0x23),
    (OBS_KEY_LEFT, 0x25),
    (OBS_KEY_UP, 0x26),
    (OBS_KEY_RIGHT, 0x27),
    (OBS_KEY_DOWN, 0x28),
    (OBS_KEY_PAGEUP, 0x21),
    (OBS_KEY_PAGEDOWN, 0x22),
    (OBS_KEY_SHIFT, 0x10),
    (OBS_KEY_CONTROL, 0x11),
    (OBS_KEY_ALT, 0x12),
    (OBS_KEY_CAPSLOCK, 0x14),
    (OBS_KEY_NUMLOCK, 0x90),
    (OBS_KEY_SCROLLLOCK, 0x91),
    (OBS_KEY_F1, 0x70),
    (OBS_KEY_F2, 0x71),
    (OBS_KEY_F3, 0x72),
    (OBS_KEY_F4, 0x73),
    (OBS_KEY_F5, 0x74),
    (OBS_KEY_F6, 0x75),
    (OBS_KEY_F7, 0x76),
    (OBS_KEY_F8, 0x77),
    (OBS_KEY_F9, 0x78),
    (OBS_KEY_F10, 0x79),
    (OBS_KEY_F11, 0x7A),
    (OBS_KEY_F12, 0x7B),
    (OBS_KEY_F13, 0x7C),
    (OBS_KEY_F14, 0x7D),
    (OBS_KEY_F15, 0x7E),
    (OBS_KEY_F16, 0x7F),
    (OBS_KEY_F17, 0x80),
    (OBS_KEY_F18, 0x81),
    (OBS_KEY_F19, 0x82),
    (OBS_KEY_F20, 0x83),
    (OBS_KEY_F21, 0x84),
    (OBS_KEY_F22, 0x85),
    (OBS_KEY_F23, 0x86),
    (OBS_KEY_F24, 0x87),
    (OBS_KEY_SPACE, 0x20),
    (OBS_KEY_APOSTROPHE, 0xDE),
    (OBS_KEY_PLUS, 0xBB),
    (OBS_KEY_COMMA, 0xBC),
    (OBS_KEY_MINUS, 0xBD),
    (OBS_KEY_PERIOD, 0xBE),
    (OBS_KEY_SLASH, 0xBF),
    (OBS_KEY_0, 0x30),
    (OBS_KEY_1, 0x31),
    (OBS_KEY_2, 0x32),
    (OBS_KEY_3, 0x33),
    (OBS_KEY_4, 0x34),
    (OBS_KEY_5, 0x35),
    (OBS_KEY_6, 0x36),
    (OBS_KEY_7, 0x37),
    (OBS_KEY_8, 0x38),
    (OBS_KEY_9, 0x39),
    (OBS_KEY_NUMASTERISK, 0x6A),
    (OBS_KEY_NUMPLUS, 0x6B),
    (OBS_KEY_NUMMINUS, 0x6D),
    (OBS_KEY_NUMPERIOD, 0x6E),
    (OBS_KEY_NUMSLASH, 0x6F),
    (OBS_KEY_NUM0, 0x60),
    (OBS_KEY_NUM1, 0x61),
    (OBS_KEY_NUM2, 0x62),
    (OBS_KEY_NUM3, 0x63),
    (OBS_KEY_NUM4, 0x64),
    (OBS_KEY_NUM5, 0x65),
    (OBS_KEY_NUM6, 0x66),
    (OBS_KEY_NUM7, 0x67),
    (OBS_KEY_NUM8, 0x68),
    (OBS_KEY_NUM9, 0x69),
    (OBS_KEY_SEMICOLON, 0xBA),
    (OBS_KEY_A, 0x41),
    (OBS_KEY_B, 0x42),
    (OBS_KEY_C, 0x43),
    (OBS_KEY_D, 0x44),
    (OBS_KEY_E, 0x45),
    (OBS_KEY_F, 0x46),
    (OBS_KEY_G, 0x47),
    (OBS_KEY_H, 0x48),
    (OBS_KEY_I, 0x49),
    (OBS_KEY_J, 0x4A),
    (OBS_KEY_K, 0x4B),
    (OBS_KEY_L, 0x4C),
    (OBS_KEY_M, 0x4D),
    (OBS_KEY_N, 0x4E),
    (OBS_KEY_O, 0x4F),
    (OBS_KEY_P, 0x50),
    (OBS_KEY_Q, 0x51),
    (OBS_KEY_R, 0x52),
    (OBS_KEY_S, 0x53),
    (OBS_KEY_T, 0x54),
    (OBS_KEY_U, 0x55),
    (OBS_KEY_V, 0x56),
    (OBS_KEY_W, 0x57),
    (OBS_KEY_X, 0x58),
    (OBS_KEY_Y, 0x59),
    (OBS_KEY_Z, 0x5A),
    (OBS_KEY_BRACKETLEFT, 0xDB),
    (OBS_KEY_BACKSLASH, 0xDC),
    (OBS_KEY_BRACKETRIGHT, 0xDD),
    (OBS_KEY_ASCIITILDE, 0xC0),
    (OBS_KEY_KANJI, 0x19),
    (OBS_KEY_MUHENKAN, 0x1D),
    (OBS_KEY_HENKAN, 0x1C),
    (OBS_KEY_TOUROKU, 0x94),
    (OBS_KEY_MASSYO, 0x93),
    (OBS_KEY_HANGUL, 0x15),
    (OBS_KEY_MOUSE1, 0x01),
    (OBS_KEY_MOUSE2, 0x02),
    (OBS_KEY_MOUSE3, 0x04),
    (OBS_KEY_MOUSE4, 0x05),
    (OBS_KEY_MOUSE5, 0x06),
    (OBS_KEY_BACKSLASH_RT102, 0xE2),
    (OBS_KEY_VK_CANCEL, 0x03),
    (OBS_KEY_0x07, 0x07),
    (OBS_KEY_0x0A, 0x0A),
    (OBS_KEY_0x0B, 0x0B),
    (OBS_KEY_0x0E, 0x0E),
    (OBS_KEY_0x0F, 0x0F),
    (OBS_KEY_0x16, 0x16),
    (OBS_KEY_VK_JUNJA, 0x17),
    (OBS_KEY_VK_FINAL, 0x18),
    (OBS_KEY_0x1A, 0x1A),
    (OBS_KEY_VK_ACCEPT, 0x1E),
    (OBS_KEY_VK_MODECHANGE, 0x1F),
    (OBS_KEY_VK_SELECT, 0x29),
    (OBS_KEY_VK_PRINT, 0x2A),
    (OBS_KEY_VK_EXECUTE, 0x2B),
    (OBS_KEY_VK_HELP, 0x2F),
    (OBS_KEY_0x30, 0x30),
    (OBS_KEY_0x31, 0x31),
    (OBS_KEY_0x32, 0x32),
    (OBS_KEY_0x33, 0x33),
    (OBS_KEY_0x34, 0x34),
    (OBS_KEY_0x35, 0x35),
    (OBS_KEY_0x36, 0x36),
    (OBS_KEY_0x37, 0x37),
    (OBS_KEY_0x38, 0x38),
    (OBS_KEY_0x39, 0x39),
    (OBS_KEY_0x3A, 0x3A),
    (OBS_KEY_0x3B, 0x3B),
    (OBS_KEY_0x3C, 0x3C),
    (OBS_KEY_0x3D, 0x3D),
    (OBS_KEY_0x3E, 0x3E),
    (OBS_KEY_0x3F, 0x3F),
    (OBS_KEY_0x40, 0x40),
    (OBS_KEY_0x41, 0x41),
    (OBS_KEY_0x42, 0x42),
    (OBS_KEY_0x43, 0x43),
    (OBS_KEY_0x44, 0x44),
    (OBS_KEY_0x45, 0x45),
    (OBS_KEY_0x46, 0x46),
    (OBS_KEY_0x47, 0x47),
    (OBS_KEY_0x48, 0x48),
    (OBS_KEY_0x49, 0x49),
    (OBS_KEY_0x4A, 0x4A),
    (OBS_KEY_0x4B, 0x4B),
    (OBS_KEY_0x4C, 0x4C),
    (OBS_KEY_0x4D, 0x4D),
    (OBS_KEY_0x4E, 0x4E),
    (OBS_KEY_0x4F, 0x4F),
    (OBS_KEY_0x50, 0x50),
    (OBS_KEY_0x51, 0x51),
    (OBS_KEY_0x52, 0x52),
    (OBS_KEY_0x53, 0x53),
    (OBS_KEY_0x54, 0x54),
    (OBS_KEY_0x55, 0x55),
    (OBS_KEY_0x56, 0x56),
    (OBS_KEY_0x57, 0x57),
    (OBS_KEY_0x58, 0x58),
    (OBS_KEY_0x59, 0x59),
    (OBS_KEY_0x5A, 0x5A),
    (OBS_KEY_VK_LWIN, 0x5B),
    (OBS_KEY_VK_RWIN, 0x5C),
    (OBS_KEY_VK_APPS, 0x5D),
    (OBS_KEY_0x5E, 0x5E),
    (OBS_KEY_VK_SLEEP, 0x5F),
    (OBS_KEY_VK_SEPARATOR, 0x6C),
    (OBS_KEY_0x88, 0x88),
    (OBS_KEY_0x89, 0x89),
    (OBS_KEY_0x8A, 0x8A),
    (OBS_KEY_0x8B, 0x8B),
    (OBS_KEY_0x8C, 0x8C),
    (OBS_KEY_0x8D, 0x8D),
    (OBS_KEY_0x8E, 0x8E),
    (OBS_KEY_0x8F, 0x8F),
    (OBS_KEY_VK_OEM_FJ_JISHO, 0x92),
    (OBS_KEY_VK_OEM_FJ_LOYA, 0x95),
    (OBS_KEY_VK_OEM_FJ_ROYA, 0x96),
    (OBS_KEY_0x97, 0x97),
    (OBS_KEY_0x98, 0x98),
    (OBS_KEY_0x99, 0x99),
    (OBS_KEY_0x9A, 0x9A),
    (OBS_KEY_0x9B, 0x9B),
    (OBS_KEY_0x9C, 0x9C),
    (OBS_KEY_0x9D, 0x9D),
    (OBS_KEY_0x9E, 0x9E),
    (OBS_KEY_0x9F, 0x9F),
    (OBS_KEY_VK_LSHIFT, 0xA0),
    (OBS_KEY_VK_RSHIFT, 0xA1),
    (OBS_KEY_VK_LCONTROL, 0xA2),
    (OBS_KEY_VK_RCONTROL, 0xA3),
    (OBS_KEY_VK_LMENU, 0xA4),
    (OBS_KEY_VK_RMENU, 0xA5),
    (OBS_KEY_VK_BROWSER_BACK, 0xA6),
    (OBS_KEY_VK_BROWSER_FORWARD, 0xA7),
    (OBS_KEY_VK_BROWSER_REFRESH, 0xA8),
    (OBS_KEY_VK_BROWSER_STOP, 0xA9),
    (OBS_KEY_VK_BROWSER_SEARCH, 0xAA),
    (OBS_KEY_VK_BROWSER_FAVORITES, 0xAB),
    (OBS_KEY_VK_BROWSER_HOME, 0xAC),
    (OBS_KEY_VK_VOLUME_MUTE, 0xAD),
    (OBS_KEY_VK_VOLUME_DOWN, 0xAE),
    (OBS_KEY_VK_VOLUME_UP, 0xAF),
    (OBS_KEY_VK_MEDIA_NEXT_TRACK, 0xB0),
    (OBS_KEY_VK_MEDIA_PREV_TRACK, 0xB1),
    (OBS_KEY_VK_MEDIA_STOP, 0xB2),
    (OBS_KEY_VK_MEDIA_PLAY_PAUSE, 0xB3),
    (OBS_KEY_VK_LAUNCH_MAIL, 0xB4),
    (OBS_KEY_VK_LAUNCH_MEDIA_SELECT, 0xB5),
    (OBS_KEY_VK_LAUNCH_APP1, 0xB6),
    (OBS_KEY_VK_LAUNCH_APP2, 0xB7),
    (OBS_KEY_0xB8, 0xB8),
    (OBS_KEY_0xB9, 0xB9),
    (OBS_KEY_0xC1, 0xC1),
    (OBS_KEY_0xC2, 0xC2),
    (OBS_KEY_0xC3, 0xC3),
    (OBS_KEY_0xC4, 0xC4),
    (OBS_KEY_0xC5, 0xC5),
    (OBS_KEY_0xC6, 0xC6),
    (OBS_KEY_0xC7, 0xC7),
    (OBS_KEY_0xC8, 0xC8),
    (OBS_KEY_0xC9, 0xC9),
    (OBS_KEY_0xCA, 0xCA),
    (OBS_KEY_0xCB, 0xCB),
    (OBS_KEY_0xCC, 0xCC),
    (OBS_KEY_0xCD, 0xCD),
    (OBS_KEY_0xCE, 0xCE),
    (OBS_KEY_0xCF, 0xCF),
    (OBS_KEY_0xD0, 0xD0),
    (OBS_KEY_0xD1, 0xD1),
    (OBS_KEY_0xD2, 0xD2),
    (OBS_KEY_0xD3, 0xD3),
    (OBS_KEY_0xD4, 0xD4),
    (OBS_KEY_0xD5, 0xD5),
    (OBS_KEY_0xD6, 0xD6),
    (OBS_KEY_0xD7, 0xD7),
    (OBS_KEY_0xD8, 0xD8),
    (OBS_KEY_0xD9, 0xD9),
    (OBS_KEY_0xDA, 0xDA),
    (OBS_KEY_VK_OEM_8, 0xDF),
    (OBS_KEY_0xE0, 0xE0),
    (OBS_KEY_VK_OEM_AX, 0xE1),
    (OBS_KEY_VK_ICO_HELP, 0xE3),
    (OBS_KEY_VK_ICO_00, 0xE4),
    (OBS_KEY_VK_PROCESSKEY, 0xE5),
    (OBS_KEY_VK_ICO_CLEAR, 0xE6),
    (OBS_KEY_VK_PACKET, 0xE7),
    (OBS_KEY_0xE8, 0xE8),
    (OBS_KEY_VK_OEM_RESET, 0xE9),
    (OBS_KEY_VK_OEM_JUMP, 0xEA),
    (OBS_KEY_VK_OEM_PA1, 0xEB),
    (OBS_KEY_VK_OEM_PA2, 0xEC),
    (OBS_KEY_VK_OEM_PA3, 0xED),
    (OBS_KEY_VK_OEM_WSCTRL, 0xEE),
    (OBS_KEY_VK_OEM_CUSEL, 0xEF),
    (OBS_KEY_VK_OEM_ATTN, 0xF0),
    (OBS_KEY_VK_OEM_FINISH, 0xF1),
    (OBS_KEY_VK_OEM_COPY, 0xF2),
    (OBS_KEY_VK_OEM_AUTO, 0xF3),
    (OBS_KEY_VK_OEM_ENLW, 0xF4),
    (OBS_KEY_VK_ATTN, 0xF6),
    (OBS_KEY_VK_CRSEL, 0xF7),
    (OBS_KEY_VK_EXSEL, 0xF8),
    (OBS_KEY_VK_EREOF, 0xF9),
    (OBS_KEY_VK_PLAY, 0xFA),
    (OBS_KEY_VK_ZOOM, 0xFB),
    (OBS_KEY_VK_NONAME, 0xFC),
    (OBS_KEY_VK_PA1, 0xFD),
    (OBS_KEY_VK_OEM_CLEAR, 0xFE),
];

/// `INTERACT_CAPS_KEY`, `INTERACT_MOUSE_*` and `INTERACT_NUMLOCK_KEY`.
const INTERACT_CAPS_KEY: u32 = 1;
const INTERACT_MOUSE_LEFT: u32 = 1 << 4;
const INTERACT_MOUSE_MIDDLE: u32 = 1 << 5;
const INTERACT_MOUSE_RIGHT: u32 = 1 << 6;
const INTERACT_NUMLOCK_KEY: u32 = 1 << 8;

/// `OBS_KEY_LAST_VALUE` as a key.
fn last() -> i32 {
    i32::try_from(OBS_KEY_LAST_VALUE).unwrap()
}

/// The C test's static `vk_codes` after `fill()`, and its `table()`
/// getter's call count.
struct Fixture {
    vk_codes: Vec<i32>,
    asked: Cell<u32>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            vk_codes: (0..last()).map(wk::virtual_key).collect(),
            asked: Cell::new(0),
        }
    }

    fn set(&mut self, key: i32, vk: i32) {
        self.vk_codes[usize::try_from(key).unwrap()] = vk;
    }

    fn entry(&self, key: i32) -> i32 {
        self.vk_codes[usize::try_from(key).unwrap()]
    }

    fn table(&self) -> &[i32] {
        self.asked.set(self.asked.get() + 1);
        &self.vk_codes
    }

    fn to(&self, key: i32) -> i32 {
        wk::to_virtual(key, || self.table())
    }

    fn from(&self, code: i32) -> i32 {
        wk::from_virtual(&self.vk_codes, code)
    }
}

/// The C test's `name_route` helper: the route, and the number it writes
/// to a `number` starting at `-1`.
fn route(key: i32) -> (Name, i32) {
    let name = wk::name(key);
    let number = match name {
        Name::F(n) | Name::Mouse(n) => n,
        _ => -1,
    };
    (name, number)
}

/// VK_HOME, VK_END, VK_LEFT, VK_UP, VK_RIGHT, VK_DOWN, VK_PRIOR, VK_NEXT,
/// VK_INSERT, VK_DELETE and VK_NUMLOCK.
const EXTENDED_VKS: [i32; 11] = [
    0x24, 0x23, 0x25, 0x26, 0x27, 0x28, 0x21, 0x22, 0x2D, 0x2E, 0x90,
];

#[test]
fn virtual_key_table() {
    assert_eq!(MAPPED.len(), 290);
    for &(key, vk) in MAPPED {
        assert_eq!(wk::virtual_key(key), vk, "key {key}");
    }

    // Every key not listed is 0.
    let nonzero = (0..last()).filter(|&key| wk::virtual_key(key) != 0).count();
    assert_eq!(nonzero, MAPPED.len());
}

#[test]
fn virtual_key_named() {
    // The mouse buttons: VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1
    // and VK_XBUTTON2. Mouse 6 to 29 have none.
    assert_eq!(wk::virtual_key(OBS_KEY_MOUSE1), 0x01);
    assert_eq!(wk::virtual_key(OBS_KEY_MOUSE2), 0x02);
    assert_eq!(wk::virtual_key(OBS_KEY_MOUSE3), 0x04);
    assert_eq!(wk::virtual_key(OBS_KEY_MOUSE4), 0x05);
    assert_eq!(wk::virtual_key(OBS_KEY_MOUSE5), 0x06);
    assert_eq!(wk::virtual_key(OBS_KEY_MOUSE6), 0);
    assert_eq!(wk::virtual_key(OBS_KEY_MOUSE29), 0);

    // A letter and its raw code share a virtual key.
    assert_eq!(wk::virtual_key(OBS_KEY_A), 0x41);
    assert_eq!(wk::virtual_key(OBS_KEY_0x41), 0x41);

    // Meta has none of its own (characterized, not endorsed): the pressed
    // check and obs_key_to_virtual_key() treat it apart.
    assert_eq!(wk::virtual_key(OBS_KEY_META), 0);
    assert_eq!(wk::virtual_key(OBS_KEY_VK_LWIN), 0x5B);
    assert_eq!(wk::virtual_key(OBS_KEY_NONE), 0);
    assert_eq!(wk::virtual_key(OBS_KEY_F24), 0x87);
    assert_eq!(wk::virtual_key(OBS_KEY_VK_OEM_CLEAR), 0xFE);
}

#[test]
fn either_win() {
    for key in 0..last() {
        assert_eq!(wk::either_win(key), key == OBS_KEY_META, "key {key}");
    }
    assert!(!wk::either_win(OBS_KEY_VK_LWIN));
    assert!(!wk::either_win(last()));
}

#[test]
fn name_route() {
    assert_eq!(route(OBS_KEY_NONE), (Name::None, -1));

    assert_eq!(route(OBS_KEY_F13), (Name::F(13), 13));
    assert_eq!(route(OBS_KEY_F24), (Name::F(24), 24));

    assert_eq!(route(OBS_KEY_MOUSE1), (Name::Mouse(1), 1));
    assert_eq!(route(OBS_KEY_MOUSE29), (Name::Mouse(29), 29));

    // Only F and Mouse write the number.
    assert_eq!(route(OBS_KEY_PAUSE), (Name::Pause, -1));
    assert_eq!(route(OBS_KEY_META), (Name::Meta, -1));
    // F1 to F12 and F25 on are named by the system.
    assert_eq!(route(OBS_KEY_F12), (Name::System, -1));
    assert_eq!(route(OBS_KEY_F25), (Name::System, -1));
    assert_eq!(route(OBS_KEY_A), (Name::System, -1));
    assert_eq!(route(OBS_KEY_VK_LWIN), (Name::System, -1));
    assert_eq!(route(OBS_KEY_VK_CANCEL), (Name::System, -1));
    assert_eq!(route(OBS_KEY_MOUSE29 + 1), (Name::System, -1));

    // Every F13 to F24 and Mouse key, by number.
    for key in OBS_KEY_F13..=OBS_KEY_F24 {
        assert_eq!(
            route(key),
            (Name::F(key - OBS_KEY_F13 + 13), key - OBS_KEY_F13 + 13)
        );
    }
    for key in OBS_KEY_MOUSE1..=OBS_KEY_MOUSE29 {
        assert_eq!(
            route(key),
            (
                Name::Mouse(key - OBS_KEY_MOUSE1 + 1),
                key - OBS_KEY_MOUSE1 + 1
            )
        );
    }
}

#[test]
fn scan_code() {
    // The scan code in bits 16 and up; Enter is not extended.
    assert_eq!(wk::scan_code(0x0D, 0x1C), 0x001C_0000);
    assert_eq!(wk::scan_code(0x0D, 0), 0);
    // Home is: bit 24 is set, even without a scan code.
    assert_eq!(wk::scan_code(0x24, 0x47), 0x0147_0000);
    assert_eq!(wk::scan_code(0x24, 0), 0x0100_0000);
    // The scan code's high bits shift out; bit 24 overrides its own.
    assert_eq!(wk::scan_code(0x0D, 0xFFFF_0001), 0x0001_0000);
    assert_eq!(wk::scan_code(0x0D, 0x0100), 0x0100_0000);
    assert_eq!(wk::scan_code(0x90, 0x0100), 0x0100_0000);

    // Exactly the 11 extended keys, of every virtual key and a few beyond.
    for vk in -2..0x200 {
        let extended = if EXTENDED_VKS.contains(&vk) {
            0x0100_0000
        } else {
            0
        };
        assert_eq!(wk::scan_code(vk, 0x45), 0x0045_0000 | extended, "vk {vk}");
    }
}

#[test]
fn system_name() {
    assert!(wk::system_name(OBS_KEY_A, 0x001E_0000));
    assert!(!wk::system_name(OBS_KEY_A, 0));
    assert!(wk::system_name(OBS_KEY_A, 0x0100_0000));
    assert!(wk::system_name(OBS_KEY_NONE, 1));

    // The OBS_KEY_VK_* keys, OBS_KEY_VK_CANCEL to OBS_KEY_VK_OEM_CLEAR, never.
    assert!(wk::system_name(OBS_KEY_VK_CANCEL - 1, 1));
    assert!(!wk::system_name(OBS_KEY_VK_CANCEL, 1));
    assert!(!wk::system_name(OBS_KEY_VK_LWIN, 1));
    assert!(!wk::system_name(OBS_KEY_VK_OEM_CLEAR, 1));
    assert!(wk::system_name(OBS_KEY_VK_OEM_CLEAR + 1, 1));
    assert!(wk::system_name(last(), 1));
}

#[test]
fn from_virtual_key() {
    let mut f = Fixture::new();

    assert_eq!(f.from(0x0D), OBS_KEY_RETURN);
    assert_eq!(f.from(0x01), OBS_KEY_MOUSE1);
    assert_eq!(f.from(0xFE), OBS_KEY_VK_OEM_CLEAR);
    // Two keys share 'A': the first, the letter, wins.
    const { assert!(OBS_KEY_A < OBS_KEY_0x41) };
    assert_eq!(f.from(0x41), OBS_KEY_A);
    // VK_LWIN is OBS_KEY_VK_LWIN, not Meta (characterized, not endorsed:
    // obs_key_to_virtual_key(OBS_KEY_META) is VK_LWIN, so Meta doesn't
    // round-trip).
    assert_eq!(f.from(0x5B), OBS_KEY_VK_LWIN);
    // 0 is the first unmapped key, OBS_KEY_NONE itself.
    assert_eq!(f.from(0), OBS_KEY_NONE);
    // No key has these.
    assert_eq!(f.from(0xFF), OBS_KEY_NONE);
    assert_eq!(f.from(-1), OBS_KEY_NONE);
    assert_eq!(f.from(0x100), OBS_KEY_NONE);

    // The last entry is searched too.
    f.set(last() - 1, 0x7777);
    assert_eq!(f.from(0x7777), last() - 1);
}

#[test]
fn to_virtual_key() {
    let mut f = Fixture::new();

    // Meta is VK_LWIN, without asking for the table.
    assert_eq!(f.to(OBS_KEY_META), 0x5B);
    assert_eq!(f.asked.get(), 0);

    // Any other key is its entry, asked for once.
    assert_eq!(f.to(OBS_KEY_RETURN), 0x0D);
    assert_eq!(f.asked.get(), 1);
    f.set(OBS_KEY_RETURN, 0x1234);
    assert_eq!(f.to(OBS_KEY_RETURN), 0x1234);
    assert_eq!(f.to(OBS_KEY_NONE), 0);
    assert_eq!(f.to(OBS_KEY_VK_LWIN), 0x5B);
    assert_eq!(f.to(last() - 1), f.entry(last() - 1));
    assert_eq!(f.asked.get(), 5);
}

#[test]
fn combination() {
    assert_eq!(wk::combination(0, OBS_KEY_NONE).keys(), &[] as &[i32]);

    assert_eq!(wk::combination(0, OBS_KEY_A).keys(), &[OBS_KEY_A]);

    // Control, Meta (Command), Alt, Shift, then the key.
    let all = INTERACT_CONTROL_KEY | INTERACT_COMMAND_KEY | INTERACT_ALT_KEY | INTERACT_SHIFT_KEY;
    assert_eq!(
        wk::combination(all, OBS_KEY_A).keys(),
        &[
            OBS_KEY_CONTROL,
            OBS_KEY_META,
            OBS_KEY_ALT,
            OBS_KEY_SHIFT,
            OBS_KEY_A
        ]
    );

    // Each of the 16 masks, with and without a key, and with the other
    // bits (Caps Lock, Num Lock, the mouse buttons) set, which add nothing.
    let bits = [
        INTERACT_CONTROL_KEY,
        INTERACT_COMMAND_KEY,
        INTERACT_ALT_KEY,
        INTERACT_SHIFT_KEY,
    ];
    let order = [OBS_KEY_CONTROL, OBS_KEY_META, OBS_KEY_ALT, OBS_KEY_SHIFT];
    let other = INTERACT_CAPS_KEY
        | INTERACT_NUMLOCK_KEY
        | INTERACT_MOUSE_LEFT
        | INTERACT_MOUSE_MIDDLE
        | INTERACT_MOUSE_RIGHT
        | 0xFFFF_FE00;

    for mask in 0..16u32 {
        for with_key in [false, true] {
            for with_other in [false, true] {
                let mut modifiers = if with_other { other } else { 0 };
                let mut expected = Vec::new();
                for (b, (&bit, &key)) in bits.iter().zip(&order).enumerate() {
                    if mask & (1 << b) != 0 {
                        modifiers |= bit;
                        expected.push(key);
                    }
                }
                let key = if with_key { OBS_KEY_F13 } else { OBS_KEY_NONE };
                if with_key {
                    expected.push(key);
                }

                assert_eq!(wk::combination(modifiers, key).keys(), expected);
            }
        }
    }

    // A modifier key itself is the key, after the modifiers.
    assert_eq!(
        wk::combination(INTERACT_SHIFT_KEY, OBS_KEY_SHIFT).keys(),
        &[OBS_KEY_SHIFT, OBS_KEY_SHIFT]
    );
}

// Beyond the C test.

#[test]
fn vk_values() {
    // A few against winuser.h; obs-windows.c checks all of them.
    assert_eq!(wk::VK_LBUTTON, 0x01);
    assert_eq!(wk::VK_RETURN, 0x0D);
    assert_eq!(VK_LWIN, 0x5B);
    assert_eq!(wk::VK_RWIN, 0x5C);
    assert_eq!(wk::VK_F24, 0x87);
    assert_eq!(wk::VK_OEM_CLEAR, 0xFE);
    assert_eq!(EXTENDED, 1 << 24);
    assert_eq!(COMBINATION_MAX, 5);
}

#[test]
fn virtual_key_outside_the_keys() {
    for key in [-1, i32::MIN, last(), last() + 1, i32::MAX] {
        assert_eq!(wk::virtual_key(key), 0, "key {key}");
    }
}

#[test]
fn virtual_key_named_keys() {
    // The modifier keys and both sides of each.
    assert_eq!(wk::virtual_key(OBS_KEY_SHIFT), wk::VK_SHIFT);
    assert_eq!(wk::virtual_key(OBS_KEY_CONTROL), wk::VK_CONTROL);
    assert_eq!(wk::virtual_key(OBS_KEY_ALT), wk::VK_MENU);
    assert_eq!(wk::virtual_key(OBS_KEY_VK_LSHIFT), wk::VK_LSHIFT);
    assert_eq!(wk::virtual_key(OBS_KEY_VK_RSHIFT), wk::VK_RSHIFT);
    assert_eq!(wk::virtual_key(OBS_KEY_VK_RWIN), wk::VK_RWIN);
    // The digits, letters and number pad.
    for (i, key) in (OBS_KEY_0..=OBS_KEY_9).enumerate() {
        assert_eq!(wk::virtual_key(key), 0x30 + i32::try_from(i).unwrap());
    }
    for (i, key) in (OBS_KEY_A..=OBS_KEY_Z).enumerate() {
        assert_eq!(wk::virtual_key(key), 0x41 + i32::try_from(i).unwrap());
    }
    for (i, key) in (OBS_KEY_NUM0..=OBS_KEY_NUM9).enumerate() {
        assert_eq!(
            wk::virtual_key(key),
            wk::VK_NUMPAD0 + i32::try_from(i).unwrap()
        );
    }
    // F1 to F24.
    for (i, key) in (OBS_KEY_F1..=OBS_KEY_F24).enumerate() {
        assert_eq!(wk::virtual_key(key), wk::VK_F1 + i32::try_from(i).unwrap());
    }
}

#[test]
fn name_route_edges() {
    assert_eq!(wk::name(OBS_KEY_F13 - 1), Name::System);
    assert_eq!(wk::name(OBS_KEY_MOUSE1 - 1), Name::System);
    assert_eq!(wk::name(-1), Name::System);
    assert_eq!(wk::name(i32::MIN), Name::System);
    assert_eq!(wk::name(last()), Name::System);
    assert_eq!(wk::name(i32::MAX), Name::System);
}

#[test]
fn system_name_outside_the_keys() {
    // C compares obs_key_t, unsigned to GCC and Clang and signed to MSVC; a
    // key outside the enum is outside the VK range either way.
    assert!(wk::system_name(-1, 1));
    assert!(wk::system_name(i32::MIN, 1));
    assert!(wk::system_name(i32::MAX, 1));
    assert!(!wk::system_name(-1, 0));
}

#[test]
fn from_virtual_short_and_empty_tables() {
    assert_eq!(wk::from_virtual(&[], 0), OBS_KEY_NONE);
    assert_eq!(wk::from_virtual(&[5, 7, 7], 7), 1);
    assert_eq!(wk::from_virtual(&[5, 7, 7], 8), OBS_KEY_NONE);
}

#[test]
fn to_virtual_outside_the_table() {
    // The fix: 0 for a key outside the table, where C reads past it. The
    // table is still asked for, as C does for any key but Meta.
    let f = Fixture::new();
    for key in [-1, i32::MIN, last(), last() + 1, i32::MAX] {
        assert_eq!(f.to(key), 0, "key {key}");
    }
    assert_eq!(f.asked.get(), 5);
    // Meta is VK_LWIN with any table.
    assert_eq!(wk::to_virtual(OBS_KEY_META, || &[]), VK_LWIN);
    assert_eq!(wk::to_virtual(OBS_KEY_A, || &[]), 0);
}

#[test]
fn combination_fills_at_most_five() {
    let all = u32::MAX;
    let combo = wk::combination(all, OBS_KEY_A);
    assert_eq!(combo.keys().len(), COMBINATION_MAX);
    assert_eq!(wk::combination(all, OBS_KEY_NONE).keys().len(), 4);
}

/// Any key, a key of the enum, or a raw `VK_*` code.
fn key() -> impl Strategy<Value = i32> {
    prop_oneof![
        any::<i32>(),
        -2..last() + 2,
        Just(OBS_KEY_META),
        OBS_KEY_F13 - 1..=OBS_KEY_F24 + 1,
        OBS_KEY_MOUSE1 - 1..=OBS_KEY_MOUSE29 + 1,
        OBS_KEY_VK_CANCEL - 1..=OBS_KEY_VK_OEM_CLEAR + 1,
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn name_matches_the_c(key in key()) {
        // obs_key_to_str()'s if/else chain, as written.
        let expected = if key == OBS_KEY_NONE {
            Name::None
        } else if (OBS_KEY_F13..=OBS_KEY_F24).contains(&key) {
            Name::F(key - OBS_KEY_F13 + 13)
        } else if (OBS_KEY_MOUSE1..=OBS_KEY_MOUSE29).contains(&key) {
            Name::Mouse(key - OBS_KEY_MOUSE1 + 1)
        } else if key == OBS_KEY_PAUSE {
            Name::Pause
        } else if key == OBS_KEY_META {
            Name::Meta
        } else {
            Name::System
        };
        prop_assert_eq!(wk::name(key), expected);
    }

    #[test]
    fn scan_code_matches_the_c(vk in prop_oneof![any::<i32>(), -2..0x200], mapped in any::<u32>()) {
        let mut expected = mapped << 16;
        if EXTENDED_VKS.contains(&vk) {
            expected |= 0x0100_0000;
        }
        prop_assert_eq!(wk::scan_code(vk, mapped), expected);
    }

    // The C comparison, as written.
    #[allow(clippy::manual_range_contains)]
    #[test]
    fn system_name_matches_the_c(key in key(), scan in prop_oneof![Just(0u32), any::<u32>()]) {
        let expected = (key < OBS_KEY_VK_CANCEL || key > OBS_KEY_VK_OEM_CLEAR) && scan != 0;
        prop_assert_eq!(wk::system_name(key, scan), expected);
    }

    #[test]
    fn from_virtual_matches_the_c(
        table in proptest::collection::vec(-2i32..8, 0..64),
        code in -3i32..9,
    ) {
        let expected = table
            .iter()
            .position(|&vk| vk == code)
            .map_or(OBS_KEY_NONE, |i| i32::try_from(i).unwrap());
        prop_assert_eq!(wk::from_virtual(&table, code), expected);
    }

    #[test]
    fn to_virtual_matches_the_c(key in key()) {
        let f = Fixture::new();
        let expected = if key == OBS_KEY_META {
            VK_LWIN
        } else {
            usize::try_from(key)
                .ok()
                .and_then(|k| f.vk_codes.get(k).copied())
                .unwrap_or(0)
        };
        prop_assert_eq!(f.to(key), expected);
        prop_assert_eq!(f.asked.get(), u32::from(key != OBS_KEY_META));
    }

    #[test]
    fn combination_matches_the_c(modifiers in any::<u32>(), key in key()) {
        let mut expected = Vec::new();
        for (bit, k) in [
            (INTERACT_CONTROL_KEY, OBS_KEY_CONTROL),
            (INTERACT_COMMAND_KEY, OBS_KEY_META),
            (INTERACT_ALT_KEY, OBS_KEY_ALT),
            (INTERACT_SHIFT_KEY, OBS_KEY_SHIFT),
        ] {
            if modifiers & bit != 0 {
                expected.push(k);
            }
        }
        if key != OBS_KEY_NONE {
            expected.push(key);
        }
        let combo = wk::combination(modifiers, key);
        prop_assert_eq!(combo.keys(), expected);
    }

    #[test]
    fn either_win_is_meta(key in key()) {
        prop_assert_eq!(wk::either_win(key), key == OBS_KEY_META);
    }
}
