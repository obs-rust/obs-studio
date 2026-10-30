//! Safe core of `libobs/obs-windows-keys.c.h`: the key map and key name
//! route of the Windows hotkey platform (`obs-windows.c`).
//!
//! The Windows hotkey platform (still C) asks this code:
//! - [`virtual_key`] (`get_virtual_key()`), once per key when it fills its
//!   `vk_codes` table (`obs_hotkeys_platform_init()`): a key's virtual key
//!   (`VK_*`), or 0;
//! - [`either_win`], on each hotkey poll (`obs_hotkeys_platform_is_pressed()`):
//!   whether the key is down when either Windows key is (Meta), rather than
//!   when its own virtual key is;
//! - [`name`], [`scan_code`] and [`system_name`], for a key's name in the
//!   hotkey settings (`obs_key_to_str()`): its route, the key code it asks
//!   `GetKeyNameTextW()` for, and whether it asks at all;
//! - [`from_virtual`] and [`to_virtual`] (`obs_key_from_virtual_key()` and
//!   `obs_key_to_virtual_key()`): the key of a virtual key and back;
//! - [`combination`] (`obs_key_combination_to_str()`): the keys a
//!   combination names, in order.
//!
//! It reads the caller's `vk_codes` table and returns keys (`obs_key_t` as
//! `i32`), so the C keeps the table, `GetAsyncKeyState()`,
//! `MapVirtualKey()`, `GetKeyNameTextW()`, the translations and every dstr
//! call. It keeps no state, takes no lock and allocates nothing.
//!
//! Intentional difference from C: [`to_virtual`] gives 0 for a key outside
//! `0 .. OBS_KEY_LAST_VALUE`; C indexes the table with it, past its end.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

pub use crate::name_map::{
    OBS_KEY_ALT, OBS_KEY_CONTROL, OBS_KEY_F13, OBS_KEY_F24, OBS_KEY_LAST_VALUE, OBS_KEY_META,
    OBS_KEY_MOUSE1, OBS_KEY_MOUSE29, OBS_KEY_NONE, OBS_KEY_PAUSE, OBS_KEY_SHIFT, OBS_KEY_VK_CANCEL,
    OBS_KEY_VK_OEM_CLEAR,
};
pub use crate::nix_hotkey_state::{
    INTERACT_ALT_KEY, INTERACT_COMMAND_KEY, INTERACT_CONTROL_KEY, INTERACT_SHIFT_KEY,
};

// The VK_* virtual keys of winuser.h that the key map uses, by value.
// obs-windows.c checks each against winuser.h.
/// `VK_LBUTTON`.
pub const VK_LBUTTON: i32 = 0x01;
/// `VK_RBUTTON`.
pub const VK_RBUTTON: i32 = 0x02;
/// `VK_CANCEL`.
pub const VK_CANCEL: i32 = 0x03;
/// `VK_MBUTTON`.
pub const VK_MBUTTON: i32 = 0x04;
/// `VK_XBUTTON1`.
pub const VK_XBUTTON1: i32 = 0x05;
/// `VK_XBUTTON2`.
pub const VK_XBUTTON2: i32 = 0x06;
/// `VK_BACK`.
pub const VK_BACK: i32 = 0x08;
/// `VK_TAB`.
pub const VK_TAB: i32 = 0x09;
/// `VK_CLEAR`.
pub const VK_CLEAR: i32 = 0x0C;
/// `VK_RETURN`.
pub const VK_RETURN: i32 = 0x0D;
/// `VK_SHIFT`.
pub const VK_SHIFT: i32 = 0x10;
/// `VK_CONTROL`.
pub const VK_CONTROL: i32 = 0x11;
/// `VK_MENU`.
pub const VK_MENU: i32 = 0x12;
/// `VK_PAUSE`.
pub const VK_PAUSE: i32 = 0x13;
/// `VK_CAPITAL`.
pub const VK_CAPITAL: i32 = 0x14;
/// `VK_HANGUL`.
pub const VK_HANGUL: i32 = 0x15;
/// `VK_JUNJA`.
pub const VK_JUNJA: i32 = 0x17;
/// `VK_FINAL`.
pub const VK_FINAL: i32 = 0x18;
/// `VK_KANJI`.
pub const VK_KANJI: i32 = 0x19;
/// `VK_ESCAPE`.
pub const VK_ESCAPE: i32 = 0x1B;
/// `VK_CONVERT`.
pub const VK_CONVERT: i32 = 0x1C;
/// `VK_NONCONVERT`.
pub const VK_NONCONVERT: i32 = 0x1D;
/// `VK_ACCEPT`.
pub const VK_ACCEPT: i32 = 0x1E;
/// `VK_MODECHANGE`.
pub const VK_MODECHANGE: i32 = 0x1F;
/// `VK_SPACE`.
pub const VK_SPACE: i32 = 0x20;
/// `VK_PRIOR`.
pub const VK_PRIOR: i32 = 0x21;
/// `VK_NEXT`.
pub const VK_NEXT: i32 = 0x22;
/// `VK_END`.
pub const VK_END: i32 = 0x23;
/// `VK_HOME`.
pub const VK_HOME: i32 = 0x24;
/// `VK_LEFT`.
pub const VK_LEFT: i32 = 0x25;
/// `VK_UP`.
pub const VK_UP: i32 = 0x26;
/// `VK_RIGHT`.
pub const VK_RIGHT: i32 = 0x27;
/// `VK_DOWN`.
pub const VK_DOWN: i32 = 0x28;
/// `VK_SELECT`.
pub const VK_SELECT: i32 = 0x29;
/// `VK_PRINT`.
pub const VK_PRINT: i32 = 0x2A;
/// `VK_EXECUTE`.
pub const VK_EXECUTE: i32 = 0x2B;
/// `VK_SNAPSHOT`.
pub const VK_SNAPSHOT: i32 = 0x2C;
/// `VK_INSERT`.
pub const VK_INSERT: i32 = 0x2D;
/// `VK_DELETE`.
pub const VK_DELETE: i32 = 0x2E;
/// `VK_HELP`.
pub const VK_HELP: i32 = 0x2F;
/// `VK_LWIN`.
pub const VK_LWIN: i32 = 0x5B;
/// `VK_RWIN`.
pub const VK_RWIN: i32 = 0x5C;
/// `VK_APPS`.
pub const VK_APPS: i32 = 0x5D;
/// `VK_SLEEP`.
pub const VK_SLEEP: i32 = 0x5F;
/// `VK_NUMPAD0`.
pub const VK_NUMPAD0: i32 = 0x60;
/// `VK_NUMPAD1`.
pub const VK_NUMPAD1: i32 = 0x61;
/// `VK_NUMPAD2`.
pub const VK_NUMPAD2: i32 = 0x62;
/// `VK_NUMPAD3`.
pub const VK_NUMPAD3: i32 = 0x63;
/// `VK_NUMPAD4`.
pub const VK_NUMPAD4: i32 = 0x64;
/// `VK_NUMPAD5`.
pub const VK_NUMPAD5: i32 = 0x65;
/// `VK_NUMPAD6`.
pub const VK_NUMPAD6: i32 = 0x66;
/// `VK_NUMPAD7`.
pub const VK_NUMPAD7: i32 = 0x67;
/// `VK_NUMPAD8`.
pub const VK_NUMPAD8: i32 = 0x68;
/// `VK_NUMPAD9`.
pub const VK_NUMPAD9: i32 = 0x69;
/// `VK_MULTIPLY`.
pub const VK_MULTIPLY: i32 = 0x6A;
/// `VK_ADD`.
pub const VK_ADD: i32 = 0x6B;
/// `VK_SEPARATOR`.
pub const VK_SEPARATOR: i32 = 0x6C;
/// `VK_SUBTRACT`.
pub const VK_SUBTRACT: i32 = 0x6D;
/// `VK_DECIMAL`.
pub const VK_DECIMAL: i32 = 0x6E;
/// `VK_DIVIDE`.
pub const VK_DIVIDE: i32 = 0x6F;
/// `VK_F1`.
pub const VK_F1: i32 = 0x70;
/// `VK_F2`.
pub const VK_F2: i32 = 0x71;
/// `VK_F3`.
pub const VK_F3: i32 = 0x72;
/// `VK_F4`.
pub const VK_F4: i32 = 0x73;
/// `VK_F5`.
pub const VK_F5: i32 = 0x74;
/// `VK_F6`.
pub const VK_F6: i32 = 0x75;
/// `VK_F7`.
pub const VK_F7: i32 = 0x76;
/// `VK_F8`.
pub const VK_F8: i32 = 0x77;
/// `VK_F9`.
pub const VK_F9: i32 = 0x78;
/// `VK_F10`.
pub const VK_F10: i32 = 0x79;
/// `VK_F11`.
pub const VK_F11: i32 = 0x7A;
/// `VK_F12`.
pub const VK_F12: i32 = 0x7B;
/// `VK_F13`.
pub const VK_F13: i32 = 0x7C;
/// `VK_F14`.
pub const VK_F14: i32 = 0x7D;
/// `VK_F15`.
pub const VK_F15: i32 = 0x7E;
/// `VK_F16`.
pub const VK_F16: i32 = 0x7F;
/// `VK_F17`.
pub const VK_F17: i32 = 0x80;
/// `VK_F18`.
pub const VK_F18: i32 = 0x81;
/// `VK_F19`.
pub const VK_F19: i32 = 0x82;
/// `VK_F20`.
pub const VK_F20: i32 = 0x83;
/// `VK_F21`.
pub const VK_F21: i32 = 0x84;
/// `VK_F22`.
pub const VK_F22: i32 = 0x85;
/// `VK_F23`.
pub const VK_F23: i32 = 0x86;
/// `VK_F24`.
pub const VK_F24: i32 = 0x87;
/// `VK_NUMLOCK`.
pub const VK_NUMLOCK: i32 = 0x90;
/// `VK_SCROLL`.
pub const VK_SCROLL: i32 = 0x91;
/// `VK_OEM_FJ_JISHO`.
pub const VK_OEM_FJ_JISHO: i32 = 0x92;
/// `VK_OEM_FJ_MASSHOU`.
pub const VK_OEM_FJ_MASSHOU: i32 = 0x93;
/// `VK_OEM_FJ_TOUROKU`.
pub const VK_OEM_FJ_TOUROKU: i32 = 0x94;
/// `VK_OEM_FJ_LOYA`.
pub const VK_OEM_FJ_LOYA: i32 = 0x95;
/// `VK_OEM_FJ_ROYA`.
pub const VK_OEM_FJ_ROYA: i32 = 0x96;
/// `VK_LSHIFT`.
pub const VK_LSHIFT: i32 = 0xA0;
/// `VK_RSHIFT`.
pub const VK_RSHIFT: i32 = 0xA1;
/// `VK_LCONTROL`.
pub const VK_LCONTROL: i32 = 0xA2;
/// `VK_RCONTROL`.
pub const VK_RCONTROL: i32 = 0xA3;
/// `VK_LMENU`.
pub const VK_LMENU: i32 = 0xA4;
/// `VK_RMENU`.
pub const VK_RMENU: i32 = 0xA5;
/// `VK_BROWSER_BACK`.
pub const VK_BROWSER_BACK: i32 = 0xA6;
/// `VK_BROWSER_FORWARD`.
pub const VK_BROWSER_FORWARD: i32 = 0xA7;
/// `VK_BROWSER_REFRESH`.
pub const VK_BROWSER_REFRESH: i32 = 0xA8;
/// `VK_BROWSER_STOP`.
pub const VK_BROWSER_STOP: i32 = 0xA9;
/// `VK_BROWSER_SEARCH`.
pub const VK_BROWSER_SEARCH: i32 = 0xAA;
/// `VK_BROWSER_FAVORITES`.
pub const VK_BROWSER_FAVORITES: i32 = 0xAB;
/// `VK_BROWSER_HOME`.
pub const VK_BROWSER_HOME: i32 = 0xAC;
/// `VK_VOLUME_MUTE`.
pub const VK_VOLUME_MUTE: i32 = 0xAD;
/// `VK_VOLUME_DOWN`.
pub const VK_VOLUME_DOWN: i32 = 0xAE;
/// `VK_VOLUME_UP`.
pub const VK_VOLUME_UP: i32 = 0xAF;
/// `VK_MEDIA_NEXT_TRACK`.
pub const VK_MEDIA_NEXT_TRACK: i32 = 0xB0;
/// `VK_MEDIA_PREV_TRACK`.
pub const VK_MEDIA_PREV_TRACK: i32 = 0xB1;
/// `VK_MEDIA_STOP`.
pub const VK_MEDIA_STOP: i32 = 0xB2;
/// `VK_MEDIA_PLAY_PAUSE`.
pub const VK_MEDIA_PLAY_PAUSE: i32 = 0xB3;
/// `VK_LAUNCH_MAIL`.
pub const VK_LAUNCH_MAIL: i32 = 0xB4;
/// `VK_LAUNCH_MEDIA_SELECT`.
pub const VK_LAUNCH_MEDIA_SELECT: i32 = 0xB5;
/// `VK_LAUNCH_APP1`.
pub const VK_LAUNCH_APP1: i32 = 0xB6;
/// `VK_LAUNCH_APP2`.
pub const VK_LAUNCH_APP2: i32 = 0xB7;
/// `VK_OEM_1`.
pub const VK_OEM_1: i32 = 0xBA;
/// `VK_OEM_PLUS`.
pub const VK_OEM_PLUS: i32 = 0xBB;
/// `VK_OEM_COMMA`.
pub const VK_OEM_COMMA: i32 = 0xBC;
/// `VK_OEM_MINUS`.
pub const VK_OEM_MINUS: i32 = 0xBD;
/// `VK_OEM_PERIOD`.
pub const VK_OEM_PERIOD: i32 = 0xBE;
/// `VK_OEM_2`.
pub const VK_OEM_2: i32 = 0xBF;
/// `VK_OEM_3`.
pub const VK_OEM_3: i32 = 0xC0;
/// `VK_OEM_4`.
pub const VK_OEM_4: i32 = 0xDB;
/// `VK_OEM_5`.
pub const VK_OEM_5: i32 = 0xDC;
/// `VK_OEM_6`.
pub const VK_OEM_6: i32 = 0xDD;
/// `VK_OEM_7`.
pub const VK_OEM_7: i32 = 0xDE;
/// `VK_OEM_8`.
pub const VK_OEM_8: i32 = 0xDF;
/// `VK_OEM_AX`.
pub const VK_OEM_AX: i32 = 0xE1;
/// `VK_OEM_102`.
pub const VK_OEM_102: i32 = 0xE2;
/// `VK_ICO_HELP`.
pub const VK_ICO_HELP: i32 = 0xE3;
/// `VK_ICO_00`.
pub const VK_ICO_00: i32 = 0xE4;
/// `VK_PROCESSKEY`.
pub const VK_PROCESSKEY: i32 = 0xE5;
/// `VK_ICO_CLEAR`.
pub const VK_ICO_CLEAR: i32 = 0xE6;
/// `VK_PACKET`.
pub const VK_PACKET: i32 = 0xE7;
/// `VK_OEM_RESET`.
pub const VK_OEM_RESET: i32 = 0xE9;
/// `VK_OEM_JUMP`.
pub const VK_OEM_JUMP: i32 = 0xEA;
/// `VK_OEM_PA1`.
pub const VK_OEM_PA1: i32 = 0xEB;
/// `VK_OEM_PA2`.
pub const VK_OEM_PA2: i32 = 0xEC;
/// `VK_OEM_PA3`.
pub const VK_OEM_PA3: i32 = 0xED;
/// `VK_OEM_WSCTRL`.
pub const VK_OEM_WSCTRL: i32 = 0xEE;
/// `VK_OEM_CUSEL`.
pub const VK_OEM_CUSEL: i32 = 0xEF;
/// `VK_OEM_ATTN`.
pub const VK_OEM_ATTN: i32 = 0xF0;
/// `VK_OEM_FINISH`.
pub const VK_OEM_FINISH: i32 = 0xF1;
/// `VK_OEM_COPY`.
pub const VK_OEM_COPY: i32 = 0xF2;
/// `VK_OEM_AUTO`.
pub const VK_OEM_AUTO: i32 = 0xF3;
/// `VK_OEM_ENLW`.
pub const VK_OEM_ENLW: i32 = 0xF4;
/// `VK_OEM_BACKTAB`.
pub const VK_OEM_BACKTAB: i32 = 0xF5;
/// `VK_ATTN`.
pub const VK_ATTN: i32 = 0xF6;
/// `VK_CRSEL`.
pub const VK_CRSEL: i32 = 0xF7;
/// `VK_EXSEL`.
pub const VK_EXSEL: i32 = 0xF8;
/// `VK_EREOF`.
pub const VK_EREOF: i32 = 0xF9;
/// `VK_PLAY`.
pub const VK_PLAY: i32 = 0xFA;
/// `VK_ZOOM`.
pub const VK_ZOOM: i32 = 0xFB;
/// `VK_NONAME`.
pub const VK_NONAME: i32 = 0xFC;
/// `VK_PA1`.
pub const VK_PA1: i32 = 0xFD;
/// `VK_OEM_CLEAR`.
pub const VK_OEM_CLEAR: i32 = 0xFE;

/// The extended-key flag of a `GetKeyNameTextW()` key code (bit 24 of the
/// `lParam` of a keyboard message).
pub const EXTENDED: u32 = 0x0100_0000;

/// The most keys a [`Combination`] holds: four modifiers and the key.
pub const COMBINATION_MAX: usize = 5;

/// `get_virtual_key()`: the virtual key of `key`. The mouse buttons and the
/// keys of a US keyboard have their `VK_*`, the `OBS_KEY_VK_*` and
/// `OBS_KEY_0x*` keys theirs, and any other key 0.
pub fn virtual_key(key: i32) -> i32 {
    let _ = key;
    todo!()
}

/// `obs_hotkeys_platform_is_pressed()`'s Meta rule: whether `key` is down
/// when either Windows key is ([`OBS_KEY_META`]), rather than when its own
/// virtual key is.
pub fn either_win(key: i32) -> bool {
    let _ = key;
    todo!()
}

/// How `obs_key_to_str()` names a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Name {
    /// `OBS_KEY_NONE`: no name.
    None,
    /// `OBS_KEY_F13` to `OBS_KEY_F24`: "F" and the number, 13 to 24.
    F(i32),
    /// `OBS_KEY_MOUSE1` to `OBS_KEY_MOUSE29`: the key's translation if it has
    /// one, else "Mouse " and the number, 1 to 29.
    Mouse(i32),
    /// `OBS_KEY_PAUSE`: the "Pause" translation.
    Pause,
    /// `OBS_KEY_META`: the "Windows" translation.
    Meta,
    /// Any other key: the system's name of its scan code ([`system_name`]),
    /// or else its `obs_key_to_name()` name.
    System,
}

/// `obs_key_to_str()`'s route for `key`.
pub fn name(key: i32) -> Name {
    let _ = key;
    todo!()
}

/// `obs_key_to_str()`'s `GetKeyNameTextW()` key code of virtual key `vk`,
/// whose `MapVirtualKey()` scan code is `mapped`: `mapped` in bits 16 and
/// up (its high bits shifted out), with [`EXTENDED`] for the 11 extended
/// keys (Home, End, the arrows, Page Up and Down, Insert, Delete and Num
/// Lock).
pub fn scan_code(vk: i32, mapped: u32) -> u32 {
    let _ = (vk, mapped);
    todo!()
}

/// `obs_key_to_str()`'s gate before `GetKeyNameTextW()`: `key` is not an
/// `OBS_KEY_VK_*` key ([`OBS_KEY_VK_CANCEL`] to [`OBS_KEY_VK_OEM_CLEAR`])
/// and `scan_code` is not 0.
pub fn system_name(key: i32, scan_code: u32) -> bool {
    let _ = (key, scan_code);
    todo!()
}

/// `obs_key_from_virtual_key()`: the first key whose entry in `vk_codes` is
/// `code`, or `OBS_KEY_NONE`.
pub fn from_virtual(vk_codes: &[i32], code: i32) -> i32 {
    let _ = (vk_codes, code);
    todo!()
}

/// `obs_key_to_virtual_key()`: [`VK_LWIN`] for [`OBS_KEY_META`]; any other
/// key's entry in the table `vk_codes` returns, which is asked for only
/// then, or 0 for a key outside it.
pub fn to_virtual<'a>(key: i32, vk_codes: impl FnOnce() -> &'a [i32]) -> i32 {
    let _ = (key, vk_codes);
    todo!()
}

/// The keys `obs_key_combination_to_str()` names, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Combination {
    keys: [i32; COMBINATION_MAX],
    len: usize,
}

impl Combination {
    /// The keys, in the order they are named.
    pub fn keys(&self) -> &[i32] {
        self.keys.get(..self.len).unwrap_or_default()
    }
}

/// `obs_key_combination_to_str()`'s order for `modifiers` (`INTERACT_*`
/// bits) and `key`: [`OBS_KEY_CONTROL`], [`OBS_KEY_META`], [`OBS_KEY_ALT`]
/// and [`OBS_KEY_SHIFT`] for their bits, then `key` unless it is
/// `OBS_KEY_NONE`.
pub fn combination(modifiers: u32, key: i32) -> Combination {
    let _ = (modifiers, key);
    todo!()
}
