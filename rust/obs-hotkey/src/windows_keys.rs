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
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

#[allow(clippy::wildcard_imports)]
use crate::name_map::*;

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
// The raw codes (`OBS_KEY_0x07` ...) have a lowercase x.
#[allow(non_upper_case_globals)]
#[expect(clippy::too_many_lines, reason = "one arm per key, as the C switch")]
pub fn virtual_key(key: i32) -> i32 {
    match key {
        OBS_KEY_RETURN => VK_RETURN,
        OBS_KEY_ESCAPE => VK_ESCAPE,
        OBS_KEY_TAB => VK_TAB,
        OBS_KEY_BACKTAB => VK_OEM_BACKTAB,
        OBS_KEY_BACKSPACE => VK_BACK,
        OBS_KEY_INSERT => VK_INSERT,
        OBS_KEY_DELETE => VK_DELETE,
        OBS_KEY_PAUSE => VK_PAUSE,
        OBS_KEY_PRINT => VK_SNAPSHOT,
        OBS_KEY_CLEAR => VK_CLEAR,
        OBS_KEY_HOME => VK_HOME,
        OBS_KEY_END => VK_END,
        OBS_KEY_LEFT => VK_LEFT,
        OBS_KEY_UP => VK_UP,
        OBS_KEY_RIGHT => VK_RIGHT,
        OBS_KEY_DOWN => VK_DOWN,
        OBS_KEY_PAGEUP => VK_PRIOR,
        OBS_KEY_PAGEDOWN => VK_NEXT,

        OBS_KEY_SHIFT => VK_SHIFT,
        OBS_KEY_CONTROL => VK_CONTROL,
        OBS_KEY_ALT => VK_MENU,
        OBS_KEY_CAPSLOCK => VK_CAPITAL,
        OBS_KEY_NUMLOCK => VK_NUMLOCK,
        OBS_KEY_SCROLLLOCK => VK_SCROLL,

        OBS_KEY_F1 => VK_F1,
        OBS_KEY_F2 => VK_F2,
        OBS_KEY_F3 => VK_F3,
        OBS_KEY_F4 => VK_F4,
        OBS_KEY_F5 => VK_F5,
        OBS_KEY_F6 => VK_F6,
        OBS_KEY_F7 => VK_F7,
        OBS_KEY_F8 => VK_F8,
        OBS_KEY_F9 => VK_F9,
        OBS_KEY_F10 => VK_F10,
        OBS_KEY_F11 => VK_F11,
        OBS_KEY_F12 => VK_F12,
        OBS_KEY_F13 => VK_F13,
        OBS_KEY_F14 => VK_F14,
        OBS_KEY_F15 => VK_F15,
        OBS_KEY_F16 => VK_F16,
        OBS_KEY_F17 => VK_F17,
        OBS_KEY_F18 => VK_F18,
        OBS_KEY_F19 => VK_F19,
        OBS_KEY_F20 => VK_F20,
        OBS_KEY_F21 => VK_F21,
        OBS_KEY_F22 => VK_F22,
        OBS_KEY_F23 => VK_F23,
        OBS_KEY_F24 => VK_F24,

        OBS_KEY_SPACE => VK_SPACE,

        OBS_KEY_APOSTROPHE => VK_OEM_7,
        OBS_KEY_PLUS => VK_OEM_PLUS,
        OBS_KEY_COMMA => VK_OEM_COMMA,
        OBS_KEY_MINUS => VK_OEM_MINUS,
        OBS_KEY_PERIOD => VK_OEM_PERIOD,
        OBS_KEY_SLASH => VK_OEM_2,
        OBS_KEY_0 => i32::from(b'0'),
        OBS_KEY_1 => i32::from(b'1'),
        OBS_KEY_2 => i32::from(b'2'),
        OBS_KEY_3 => i32::from(b'3'),
        OBS_KEY_4 => i32::from(b'4'),
        OBS_KEY_5 => i32::from(b'5'),
        OBS_KEY_6 => i32::from(b'6'),
        OBS_KEY_7 => i32::from(b'7'),
        OBS_KEY_8 => i32::from(b'8'),
        OBS_KEY_9 => i32::from(b'9'),
        OBS_KEY_NUMASTERISK => VK_MULTIPLY,
        OBS_KEY_NUMPLUS => VK_ADD,
        OBS_KEY_NUMMINUS => VK_SUBTRACT,
        OBS_KEY_NUMPERIOD => VK_DECIMAL,
        OBS_KEY_NUMSLASH => VK_DIVIDE,
        OBS_KEY_NUM0 => VK_NUMPAD0,
        OBS_KEY_NUM1 => VK_NUMPAD1,
        OBS_KEY_NUM2 => VK_NUMPAD2,
        OBS_KEY_NUM3 => VK_NUMPAD3,
        OBS_KEY_NUM4 => VK_NUMPAD4,
        OBS_KEY_NUM5 => VK_NUMPAD5,
        OBS_KEY_NUM6 => VK_NUMPAD6,
        OBS_KEY_NUM7 => VK_NUMPAD7,
        OBS_KEY_NUM8 => VK_NUMPAD8,
        OBS_KEY_NUM9 => VK_NUMPAD9,
        OBS_KEY_SEMICOLON => VK_OEM_1,
        OBS_KEY_A => i32::from(b'A'),
        OBS_KEY_B => i32::from(b'B'),
        OBS_KEY_C => i32::from(b'C'),
        OBS_KEY_D => i32::from(b'D'),
        OBS_KEY_E => i32::from(b'E'),
        OBS_KEY_F => i32::from(b'F'),
        OBS_KEY_G => i32::from(b'G'),
        OBS_KEY_H => i32::from(b'H'),
        OBS_KEY_I => i32::from(b'I'),
        OBS_KEY_J => i32::from(b'J'),
        OBS_KEY_K => i32::from(b'K'),
        OBS_KEY_L => i32::from(b'L'),
        OBS_KEY_M => i32::from(b'M'),
        OBS_KEY_N => i32::from(b'N'),
        OBS_KEY_O => i32::from(b'O'),
        OBS_KEY_P => i32::from(b'P'),
        OBS_KEY_Q => i32::from(b'Q'),
        OBS_KEY_R => i32::from(b'R'),
        OBS_KEY_S => i32::from(b'S'),
        OBS_KEY_T => i32::from(b'T'),
        OBS_KEY_U => i32::from(b'U'),
        OBS_KEY_V => i32::from(b'V'),
        OBS_KEY_W => i32::from(b'W'),
        OBS_KEY_X => i32::from(b'X'),
        OBS_KEY_Y => i32::from(b'Y'),
        OBS_KEY_Z => i32::from(b'Z'),
        OBS_KEY_BRACKETLEFT => VK_OEM_4,
        OBS_KEY_BACKSLASH => VK_OEM_5,
        OBS_KEY_BRACKETRIGHT => VK_OEM_6,
        OBS_KEY_ASCIITILDE => VK_OEM_3,

        OBS_KEY_HENKAN => VK_CONVERT,
        OBS_KEY_MUHENKAN => VK_NONCONVERT,
        OBS_KEY_KANJI => VK_KANJI,
        OBS_KEY_TOUROKU => VK_OEM_FJ_TOUROKU,
        OBS_KEY_MASSYO => VK_OEM_FJ_MASSHOU,

        OBS_KEY_HANGUL => VK_HANGUL,

        OBS_KEY_BACKSLASH_RT102 => VK_OEM_102,

        OBS_KEY_MOUSE1 => VK_LBUTTON,
        OBS_KEY_MOUSE2 => VK_RBUTTON,
        OBS_KEY_MOUSE3 => VK_MBUTTON,
        OBS_KEY_MOUSE4 => VK_XBUTTON1,
        OBS_KEY_MOUSE5 => VK_XBUTTON2,

        OBS_KEY_VK_CANCEL => VK_CANCEL,
        OBS_KEY_0x07 => 0x07,
        OBS_KEY_0x0A => 0x0A,
        OBS_KEY_0x0B => 0x0B,
        OBS_KEY_0x0E => 0x0E,
        OBS_KEY_0x0F => 0x0F,
        OBS_KEY_0x16 => 0x16,
        OBS_KEY_VK_JUNJA => VK_JUNJA,
        OBS_KEY_VK_FINAL => VK_FINAL,
        OBS_KEY_0x1A => 0x1A,
        OBS_KEY_VK_ACCEPT => VK_ACCEPT,
        OBS_KEY_VK_MODECHANGE => VK_MODECHANGE,
        OBS_KEY_VK_SELECT => VK_SELECT,
        OBS_KEY_VK_PRINT => VK_PRINT,
        OBS_KEY_VK_EXECUTE => VK_EXECUTE,
        OBS_KEY_VK_HELP => VK_HELP,
        OBS_KEY_0x30 => 0x30,
        OBS_KEY_0x31 => 0x31,
        OBS_KEY_0x32 => 0x32,
        OBS_KEY_0x33 => 0x33,
        OBS_KEY_0x34 => 0x34,
        OBS_KEY_0x35 => 0x35,
        OBS_KEY_0x36 => 0x36,
        OBS_KEY_0x37 => 0x37,
        OBS_KEY_0x38 => 0x38,
        OBS_KEY_0x39 => 0x39,
        OBS_KEY_0x3A => 0x3A,
        OBS_KEY_0x3B => 0x3B,
        OBS_KEY_0x3C => 0x3C,
        OBS_KEY_0x3D => 0x3D,
        OBS_KEY_0x3E => 0x3E,
        OBS_KEY_0x3F => 0x3F,
        OBS_KEY_0x40 => 0x40,
        OBS_KEY_0x41 => 0x41,
        OBS_KEY_0x42 => 0x42,
        OBS_KEY_0x43 => 0x43,
        OBS_KEY_0x44 => 0x44,
        OBS_KEY_0x45 => 0x45,
        OBS_KEY_0x46 => 0x46,
        OBS_KEY_0x47 => 0x47,
        OBS_KEY_0x48 => 0x48,
        OBS_KEY_0x49 => 0x49,
        OBS_KEY_0x4A => 0x4A,
        OBS_KEY_0x4B => 0x4B,
        OBS_KEY_0x4C => 0x4C,
        OBS_KEY_0x4D => 0x4D,
        OBS_KEY_0x4E => 0x4E,
        OBS_KEY_0x4F => 0x4F,
        OBS_KEY_0x50 => 0x50,
        OBS_KEY_0x51 => 0x51,
        OBS_KEY_0x52 => 0x52,
        OBS_KEY_0x53 => 0x53,
        OBS_KEY_0x54 => 0x54,
        OBS_KEY_0x55 => 0x55,
        OBS_KEY_0x56 => 0x56,
        OBS_KEY_0x57 => 0x57,
        OBS_KEY_0x58 => 0x58,
        OBS_KEY_0x59 => 0x59,
        OBS_KEY_0x5A => 0x5A,
        OBS_KEY_VK_LWIN => VK_LWIN,
        OBS_KEY_VK_RWIN => VK_RWIN,
        OBS_KEY_VK_APPS => VK_APPS,
        OBS_KEY_0x5E => 0x5E,
        OBS_KEY_VK_SLEEP => VK_SLEEP,
        OBS_KEY_VK_SEPARATOR => VK_SEPARATOR,
        OBS_KEY_0x88 => 0x88,
        OBS_KEY_0x89 => 0x89,
        OBS_KEY_0x8A => 0x8A,
        OBS_KEY_0x8B => 0x8B,
        OBS_KEY_0x8C => 0x8C,
        OBS_KEY_0x8D => 0x8D,
        OBS_KEY_0x8E => 0x8E,
        OBS_KEY_0x8F => 0x8F,
        OBS_KEY_VK_OEM_FJ_JISHO => VK_OEM_FJ_JISHO,
        OBS_KEY_VK_OEM_FJ_LOYA => VK_OEM_FJ_LOYA,
        OBS_KEY_VK_OEM_FJ_ROYA => VK_OEM_FJ_ROYA,
        OBS_KEY_0x97 => 0x97,
        OBS_KEY_0x98 => 0x98,
        OBS_KEY_0x99 => 0x99,
        OBS_KEY_0x9A => 0x9A,
        OBS_KEY_0x9B => 0x9B,
        OBS_KEY_0x9C => 0x9C,
        OBS_KEY_0x9D => 0x9D,
        OBS_KEY_0x9E => 0x9E,
        OBS_KEY_0x9F => 0x9F,
        OBS_KEY_VK_LSHIFT => VK_LSHIFT,
        OBS_KEY_VK_RSHIFT => VK_RSHIFT,
        OBS_KEY_VK_LCONTROL => VK_LCONTROL,
        OBS_KEY_VK_RCONTROL => VK_RCONTROL,
        OBS_KEY_VK_LMENU => VK_LMENU,
        OBS_KEY_VK_RMENU => VK_RMENU,
        OBS_KEY_VK_BROWSER_BACK => VK_BROWSER_BACK,
        OBS_KEY_VK_BROWSER_FORWARD => VK_BROWSER_FORWARD,
        OBS_KEY_VK_BROWSER_REFRESH => VK_BROWSER_REFRESH,
        OBS_KEY_VK_BROWSER_STOP => VK_BROWSER_STOP,
        OBS_KEY_VK_BROWSER_SEARCH => VK_BROWSER_SEARCH,
        OBS_KEY_VK_BROWSER_FAVORITES => VK_BROWSER_FAVORITES,
        OBS_KEY_VK_BROWSER_HOME => VK_BROWSER_HOME,
        OBS_KEY_VK_VOLUME_MUTE => VK_VOLUME_MUTE,
        OBS_KEY_VK_VOLUME_DOWN => VK_VOLUME_DOWN,
        OBS_KEY_VK_VOLUME_UP => VK_VOLUME_UP,
        OBS_KEY_VK_MEDIA_NEXT_TRACK => VK_MEDIA_NEXT_TRACK,
        OBS_KEY_VK_MEDIA_PREV_TRACK => VK_MEDIA_PREV_TRACK,
        OBS_KEY_VK_MEDIA_STOP => VK_MEDIA_STOP,
        OBS_KEY_VK_MEDIA_PLAY_PAUSE => VK_MEDIA_PLAY_PAUSE,
        OBS_KEY_VK_LAUNCH_MAIL => VK_LAUNCH_MAIL,
        OBS_KEY_VK_LAUNCH_MEDIA_SELECT => VK_LAUNCH_MEDIA_SELECT,
        OBS_KEY_VK_LAUNCH_APP1 => VK_LAUNCH_APP1,
        OBS_KEY_VK_LAUNCH_APP2 => VK_LAUNCH_APP2,
        OBS_KEY_0xB8 => 0xB8,
        OBS_KEY_0xB9 => 0xB9,
        OBS_KEY_0xC1 => 0xC1,
        OBS_KEY_0xC2 => 0xC2,
        OBS_KEY_0xC3 => 0xC3,
        OBS_KEY_0xC4 => 0xC4,
        OBS_KEY_0xC5 => 0xC5,
        OBS_KEY_0xC6 => 0xC6,
        OBS_KEY_0xC7 => 0xC7,
        OBS_KEY_0xC8 => 0xC8,
        OBS_KEY_0xC9 => 0xC9,
        OBS_KEY_0xCA => 0xCA,
        OBS_KEY_0xCB => 0xCB,
        OBS_KEY_0xCC => 0xCC,
        OBS_KEY_0xCD => 0xCD,
        OBS_KEY_0xCE => 0xCE,
        OBS_KEY_0xCF => 0xCF,
        OBS_KEY_0xD0 => 0xD0,
        OBS_KEY_0xD1 => 0xD1,
        OBS_KEY_0xD2 => 0xD2,
        OBS_KEY_0xD3 => 0xD3,
        OBS_KEY_0xD4 => 0xD4,
        OBS_KEY_0xD5 => 0xD5,
        OBS_KEY_0xD6 => 0xD6,
        OBS_KEY_0xD7 => 0xD7,
        OBS_KEY_0xD8 => 0xD8,
        OBS_KEY_0xD9 => 0xD9,
        OBS_KEY_0xDA => 0xDA,
        OBS_KEY_VK_OEM_8 => VK_OEM_8,
        OBS_KEY_0xE0 => 0xE0,
        OBS_KEY_VK_OEM_AX => VK_OEM_AX,
        OBS_KEY_VK_ICO_HELP => VK_ICO_HELP,
        OBS_KEY_VK_ICO_00 => VK_ICO_00,
        OBS_KEY_VK_PROCESSKEY => VK_PROCESSKEY,
        OBS_KEY_VK_ICO_CLEAR => VK_ICO_CLEAR,
        OBS_KEY_VK_PACKET => VK_PACKET,
        OBS_KEY_0xE8 => 0xE8,
        OBS_KEY_VK_OEM_RESET => VK_OEM_RESET,
        OBS_KEY_VK_OEM_JUMP => VK_OEM_JUMP,
        OBS_KEY_VK_OEM_PA1 => VK_OEM_PA1,
        OBS_KEY_VK_OEM_PA2 => VK_OEM_PA2,
        OBS_KEY_VK_OEM_PA3 => VK_OEM_PA3,
        OBS_KEY_VK_OEM_WSCTRL => VK_OEM_WSCTRL,
        OBS_KEY_VK_OEM_CUSEL => VK_OEM_CUSEL,
        OBS_KEY_VK_OEM_ATTN => VK_OEM_ATTN,
        OBS_KEY_VK_OEM_FINISH => VK_OEM_FINISH,
        OBS_KEY_VK_OEM_COPY => VK_OEM_COPY,
        OBS_KEY_VK_OEM_AUTO => VK_OEM_AUTO,
        OBS_KEY_VK_OEM_ENLW => VK_OEM_ENLW,
        OBS_KEY_VK_ATTN => VK_ATTN,
        OBS_KEY_VK_CRSEL => VK_CRSEL,
        OBS_KEY_VK_EXSEL => VK_EXSEL,
        OBS_KEY_VK_EREOF => VK_EREOF,
        OBS_KEY_VK_PLAY => VK_PLAY,
        OBS_KEY_VK_ZOOM => VK_ZOOM,
        OBS_KEY_VK_NONAME => VK_NONAME,
        OBS_KEY_VK_PA1 => VK_PA1,
        OBS_KEY_VK_OEM_CLEAR => VK_OEM_CLEAR,

        // TODO: Implement keys for non-US keyboards
        _ => 0,
    }
}

/// `obs_hotkeys_platform_is_pressed()`'s Meta rule: whether `key` is down
/// when either Windows key is ([`OBS_KEY_META`]), rather than when its own
/// virtual key is.
pub fn either_win(key: i32) -> bool {
    key == OBS_KEY_META
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

/// `key - first + base` for a `key` in `first ..= last`.
fn number(key: i32, first: i32, last: i32, base: i32) -> Option<i32> {
    if !(first..=last).contains(&key) {
        return None;
    }
    key.checked_sub(first)?.checked_add(base)
}

/// `obs_key_to_str()`'s route for `key`.
pub fn name(key: i32) -> Name {
    if key == OBS_KEY_NONE {
        Name::None
    } else if let Some(n) = number(key, OBS_KEY_F13, OBS_KEY_F24, 13) {
        Name::F(n)
    } else if let Some(n) = number(key, OBS_KEY_MOUSE1, OBS_KEY_MOUSE29, 1) {
        Name::Mouse(n)
    } else if key == OBS_KEY_PAUSE {
        Name::Pause
    } else if key == OBS_KEY_META {
        Name::Meta
    } else {
        Name::System
    }
}

/// `obs_key_to_str()`'s `GetKeyNameTextW()` key code of virtual key `vk`,
/// whose `MapVirtualKey()` scan code is `mapped`: `mapped` in bits 16 and
/// up (its high bits shifted out), with [`EXTENDED`] for the 11 extended
/// keys (Home, End, the arrows, Page Up and Down, Insert, Delete and Num
/// Lock).
pub fn scan_code(vk: i32, mapped: u32) -> u32 {
    let scan_code = mapped.wrapping_shl(16);
    match vk {
        VK_HOME | VK_END | VK_LEFT | VK_UP | VK_RIGHT | VK_DOWN | VK_PRIOR | VK_NEXT
        | VK_INSERT | VK_DELETE | VK_NUMLOCK => scan_code | EXTENDED,
        _ => scan_code,
    }
}

/// `obs_key_to_str()`'s gate before `GetKeyNameTextW()`: `key` is not an
/// `OBS_KEY_VK_*` key ([`OBS_KEY_VK_CANCEL`] to [`OBS_KEY_VK_OEM_CLEAR`])
/// and `scan_code` is not 0.
pub fn system_name(key: i32, scan_code: u32) -> bool {
    !(OBS_KEY_VK_CANCEL..=OBS_KEY_VK_OEM_CLEAR).contains(&key) && scan_code != 0
}

/// `obs_key_from_virtual_key()`: the first key whose entry in `vk_codes` is
/// `code`, or `OBS_KEY_NONE`.
pub fn from_virtual(vk_codes: &[i32], code: i32) -> i32 {
    vk_codes
        .iter()
        .position(|&vk| vk == code)
        .and_then(|key| i32::try_from(key).ok())
        .unwrap_or(OBS_KEY_NONE)
}

/// `obs_key_to_virtual_key()`: [`VK_LWIN`] for [`OBS_KEY_META`]; any other
/// key's entry in the table `vk_codes` returns, which is asked for only
/// then, or 0 for a key outside it.
pub fn to_virtual<'a>(key: i32, vk_codes: impl FnOnce() -> &'a [i32]) -> i32 {
    if key == OBS_KEY_META {
        return VK_LWIN;
    }

    let vk_codes = vk_codes();
    usize::try_from(key)
        .ok()
        .and_then(|key| vk_codes.get(key))
        .copied()
        .unwrap_or(0)
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

    fn push(&mut self, key: i32) {
        if let Some(slot) = self.keys.get_mut(self.len) {
            *slot = key;
            self.len = self.len.saturating_add(1);
        }
    }
}

/// `obs_key_combination_to_str()`'s order for `modifiers` (`INTERACT_*`
/// bits) and `key`: [`OBS_KEY_CONTROL`], [`OBS_KEY_META`], [`OBS_KEY_ALT`]
/// and [`OBS_KEY_SHIFT`] for their bits, then `key` unless it is
/// `OBS_KEY_NONE`.
pub fn combination(modifiers: u32, key: i32) -> Combination {
    let mut combination = Combination {
        keys: [OBS_KEY_NONE; COMBINATION_MAX],
        len: 0,
    };
    for (bit, modifier) in [
        (INTERACT_CONTROL_KEY, OBS_KEY_CONTROL),
        (INTERACT_COMMAND_KEY, OBS_KEY_META),
        (INTERACT_ALT_KEY, OBS_KEY_ALT),
        (INTERACT_SHIFT_KEY, OBS_KEY_SHIFT),
    ] {
        if modifiers & bit != 0 {
            combination.push(modifier);
        }
    }
    if key != OBS_KEY_NONE {
        combination.push(key);
    }
    combination
}
