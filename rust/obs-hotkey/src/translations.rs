//! Safe core of `libobs/obs-hotkey-translations.c.h`: the hotkey key-name
//! translations.
//!
//! At startup the frontend passes `obs_hotkeys_set_translations_s()` its
//! translated names of the named keys (Insert, Page Up, Shift, Numpad /,
//! ...) and two templates, `"Numpad %1"` and `"Mouse %1"`. libobs stores
//! each name, and expands the templates into the names of `NUM0`-`NUM9`
//! and `MOUSE1`-`MOUSE29`. Settings > Hotkeys shows them through
//! `obs_key_to_str()` and `obs_key_combination_to_str()`.
//!
//! - [`Translations`] holds the members of `struct obs_hotkeys_translations`
//!   ([`Member`], in struct order), each a string or `None` (`NULL`).
//! - [`for_each_store`] makes the C's store calls, in the C's order: the
//!   named keys of [`named_keys`], then `NUM0`-`NUM9` ([`NUMPAD_KEYS`]),
//!   then `MOUSE1`-`MOUSE29` ([`MOUSE_KEYS`]). A `None` member stores
//!   nothing, so a set never clears a name. A named key's string is stored
//!   as it is, empty or not.
//! - [`prepare_template`] is the C's `dstr_copy` and `dstr_depad` of a
//!   template, then `" %1"` appended when it has no `%1`. An empty or
//!   padding-only template gives `None` and is skipped like a `NULL` one.
//! - [`replace_placeholder`] is `dstr_replace(&button, "%1", digits)`:
//!   every `%1`, left to right and without rescanning, becomes the digits.
//!   `%1` is a placeholder, not a printf format: no template is ever passed
//!   through a formatter, so `%s`, `%d` and `%n` stay literal.
//!
//! The `size` cut of the public struct (`min(size, sizeof)` bytes) and the
//! `#ifdef __APPLE__` keypad block stay at the C boundary
//! (`ffi::translations`), which picks the [`Keypad`] of its host. Both
//! layouts are part of this module, so either can be tested on any host.
//!
//! The strings come from the frontend's locale files and from plugins, and
//! are bytes, not UTF-8: depadding and replacing work on bytes, and nothing
//! here can panic, index out of bounds or overflow, whatever the input.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use std::ffi::CStr;

use crate::name_map::{
    OBS_KEY_ALT, OBS_KEY_BACKSPACE, OBS_KEY_CAPSLOCK, OBS_KEY_CONTROL, OBS_KEY_DELETE,
    OBS_KEY_DOWN, OBS_KEY_END, OBS_KEY_ESCAPE, OBS_KEY_HOME, OBS_KEY_INSERT, OBS_KEY_LEFT,
    OBS_KEY_MENU, OBS_KEY_META, OBS_KEY_MOUSE1, OBS_KEY_MOUSE2, OBS_KEY_MOUSE3, OBS_KEY_MOUSE4,
    OBS_KEY_MOUSE5, OBS_KEY_MOUSE6, OBS_KEY_MOUSE7, OBS_KEY_MOUSE8, OBS_KEY_MOUSE9,
    OBS_KEY_MOUSE10, OBS_KEY_MOUSE11, OBS_KEY_MOUSE12, OBS_KEY_MOUSE13, OBS_KEY_MOUSE14,
    OBS_KEY_MOUSE15, OBS_KEY_MOUSE16, OBS_KEY_MOUSE17, OBS_KEY_MOUSE18, OBS_KEY_MOUSE19,
    OBS_KEY_MOUSE20, OBS_KEY_MOUSE21, OBS_KEY_MOUSE22, OBS_KEY_MOUSE23, OBS_KEY_MOUSE24,
    OBS_KEY_MOUSE25, OBS_KEY_MOUSE26, OBS_KEY_MOUSE27, OBS_KEY_MOUSE28, OBS_KEY_MOUSE29,
    OBS_KEY_NUM0, OBS_KEY_NUM1, OBS_KEY_NUM2, OBS_KEY_NUM3, OBS_KEY_NUM4, OBS_KEY_NUM5,
    OBS_KEY_NUM6, OBS_KEY_NUM7, OBS_KEY_NUM8, OBS_KEY_NUM9, OBS_KEY_NUMASTERISK, OBS_KEY_NUMEQUAL,
    OBS_KEY_NUMLOCK, OBS_KEY_NUMMINUS, OBS_KEY_NUMPERIOD, OBS_KEY_NUMPLUS, OBS_KEY_NUMSLASH,
    OBS_KEY_PAGEDOWN, OBS_KEY_PAGEUP, OBS_KEY_PAUSE, OBS_KEY_PRINT, OBS_KEY_RIGHT,
    OBS_KEY_SCROLLLOCK, OBS_KEY_SHIFT, OBS_KEY_SPACE, OBS_KEY_TAB, OBS_KEY_UP,
};

/// The number of members of `struct obs_hotkeys_translations`.
pub const MEMBER_COUNT: usize = 38;

/// A member of `struct obs_hotkeys_translations` (`obs-hotkey.h`), in
/// struct order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Member {
    Insert,
    Del,
    Home,
    End,
    PageUp,
    PageDown,
    NumLock,
    ScrollLock,
    CapsLock,
    Backspace,
    Tab,
    Print,
    Pause,
    Left,
    Right,
    Up,
    Down,
    Shift,
    Alt,
    Control,
    Meta,
    Menu,
    Space,
    NumpadNum,
    NumpadDivide,
    NumpadMultiply,
    NumpadMinus,
    NumpadPlus,
    NumpadDecimal,
    AppleKeypadNum,
    AppleKeypadDivide,
    AppleKeypadMultiply,
    AppleKeypadMinus,
    AppleKeypadPlus,
    AppleKeypadDecimal,
    AppleKeypadEqual,
    MouseNum,
    Escape,
}

impl Member {
    /// Every member, in struct order.
    pub const ALL: [Self; MEMBER_COUNT] = [
        Self::Insert,
        Self::Del,
        Self::Home,
        Self::End,
        Self::PageUp,
        Self::PageDown,
        Self::NumLock,
        Self::ScrollLock,
        Self::CapsLock,
        Self::Backspace,
        Self::Tab,
        Self::Print,
        Self::Pause,
        Self::Left,
        Self::Right,
        Self::Up,
        Self::Down,
        Self::Shift,
        Self::Alt,
        Self::Control,
        Self::Meta,
        Self::Menu,
        Self::Space,
        Self::NumpadNum,
        Self::NumpadDivide,
        Self::NumpadMultiply,
        Self::NumpadMinus,
        Self::NumpadPlus,
        Self::NumpadDecimal,
        Self::AppleKeypadNum,
        Self::AppleKeypadDivide,
        Self::AppleKeypadMultiply,
        Self::AppleKeypadMinus,
        Self::AppleKeypadPlus,
        Self::AppleKeypadDecimal,
        Self::AppleKeypadEqual,
        Self::MouseNum,
        Self::Escape,
    ];

    /// The member's position in the struct (0 for `insert`).
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The member's name in the C struct (`"insert"`, `"del"`, ...).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Insert => "insert",
            Self::Del => "del",
            Self::Home => "home",
            Self::End => "end",
            Self::PageUp => "page_up",
            Self::PageDown => "page_down",
            Self::NumLock => "num_lock",
            Self::ScrollLock => "scroll_lock",
            Self::CapsLock => "caps_lock",
            Self::Backspace => "backspace",
            Self::Tab => "tab",
            Self::Print => "print",
            Self::Pause => "pause",
            Self::Left => "left",
            Self::Right => "right",
            Self::Up => "up",
            Self::Down => "down",
            Self::Shift => "shift",
            Self::Alt => "alt",
            Self::Control => "control",
            Self::Meta => "meta",
            Self::Menu => "menu",
            Self::Space => "space",
            Self::NumpadNum => "numpad_num",
            Self::NumpadDivide => "numpad_divide",
            Self::NumpadMultiply => "numpad_multiply",
            Self::NumpadMinus => "numpad_minus",
            Self::NumpadPlus => "numpad_plus",
            Self::NumpadDecimal => "numpad_decimal",
            Self::AppleKeypadNum => "apple_keypad_num",
            Self::AppleKeypadDivide => "apple_keypad_divide",
            Self::AppleKeypadMultiply => "apple_keypad_multiply",
            Self::AppleKeypadMinus => "apple_keypad_minus",
            Self::AppleKeypadPlus => "apple_keypad_plus",
            Self::AppleKeypadDecimal => "apple_keypad_decimal",
            Self::AppleKeypadEqual => "apple_keypad_equal",
            Self::MouseNum => "mouse_num",
            Self::Escape => "escape",
        }
    }
}

/// The members of one `struct obs_hotkeys_translations`: a string, or
/// `None` for a `NULL` member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Translations<'a> {
    members: [Option<&'a CStr>; MEMBER_COUNT],
}

impl<'a> Translations<'a> {
    /// Every member `None`.
    pub const fn new() -> Self {
        Self {
            members: [None; MEMBER_COUNT],
        }
    }

    /// The members in struct order ([`Member::ALL`]).
    pub const fn from_members(members: [Option<&'a CStr>; MEMBER_COUNT]) -> Self {
        Self { members }
    }

    /// The members in struct order.
    pub const fn members(&self) -> &[Option<&'a CStr>; MEMBER_COUNT] {
        &self.members
    }

    /// One member.
    pub fn get(&self, member: Member) -> Option<&'a CStr> {
        self.members.get(member.index()).copied().flatten()
    }

    /// Sets one member.
    pub fn set(&mut self, member: Member, value: Option<&'a CStr>) {
        if let Some(slot) = self.members.get_mut(member.index()) {
            *slot = value;
        }
    }

    /// With one member set.
    #[must_use]
    pub fn with(mut self, member: Member, value: &'a CStr) -> Self {
        self.set(member, Some(value));
        self
    }
}

impl Default for Translations<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// The keypad members a host applies: the C's `#ifdef __APPLE__`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keypad {
    /// Every host but macOS: `numpad_num` and the `numpad_*` keys; the
    /// `apple_*` members are ignored.
    Standard,
    /// macOS: `apple_keypad_num` and the `apple_keypad_*` keys, `=`
    /// included; the `numpad_*` members are ignored.
    Apple,
}

/// The named keys every host stores, in the C's order (struct order, then
/// escape): `ADD_TRANSLATION` before the `#ifdef __APPLE__`.
const COMMON_KEYS: [(i32, Member); 24] = [
    (OBS_KEY_INSERT, Member::Insert),
    (OBS_KEY_DELETE, Member::Del),
    (OBS_KEY_HOME, Member::Home),
    (OBS_KEY_END, Member::End),
    (OBS_KEY_PAGEUP, Member::PageUp),
    (OBS_KEY_PAGEDOWN, Member::PageDown),
    (OBS_KEY_NUMLOCK, Member::NumLock),
    (OBS_KEY_SCROLLLOCK, Member::ScrollLock),
    (OBS_KEY_CAPSLOCK, Member::CapsLock),
    (OBS_KEY_BACKSPACE, Member::Backspace),
    (OBS_KEY_TAB, Member::Tab),
    (OBS_KEY_PRINT, Member::Print),
    (OBS_KEY_PAUSE, Member::Pause),
    (OBS_KEY_LEFT, Member::Left),
    (OBS_KEY_RIGHT, Member::Right),
    (OBS_KEY_UP, Member::Up),
    (OBS_KEY_DOWN, Member::Down),
    (OBS_KEY_SHIFT, Member::Shift),
    (OBS_KEY_ALT, Member::Alt),
    (OBS_KEY_CONTROL, Member::Control),
    (OBS_KEY_META, Member::Meta),
    (OBS_KEY_MENU, Member::Menu),
    (OBS_KEY_SPACE, Member::Space),
    (OBS_KEY_ESCAPE, Member::Escape),
];

/// The keypad keys of [`Keypad::Standard`], after [`COMMON_KEYS`].
const STANDARD_KEYPAD_KEYS: [(i32, Member); 5] = [
    (OBS_KEY_NUMSLASH, Member::NumpadDivide),
    (OBS_KEY_NUMASTERISK, Member::NumpadMultiply),
    (OBS_KEY_NUMMINUS, Member::NumpadMinus),
    (OBS_KEY_NUMPLUS, Member::NumpadPlus),
    (OBS_KEY_NUMPERIOD, Member::NumpadDecimal),
];

/// The keypad keys of [`Keypad::Apple`], after [`COMMON_KEYS`].
const APPLE_KEYPAD_KEYS: [(i32, Member); 6] = [
    (OBS_KEY_NUMSLASH, Member::AppleKeypadDivide),
    (OBS_KEY_NUMASTERISK, Member::AppleKeypadMultiply),
    (OBS_KEY_NUMMINUS, Member::AppleKeypadMinus),
    (OBS_KEY_NUMPLUS, Member::AppleKeypadPlus),
    (OBS_KEY_NUMPERIOD, Member::AppleKeypadDecimal),
    (OBS_KEY_NUMEQUAL, Member::AppleKeypadEqual),
];

/// The named keys `keypad` stores, as (key code, member), in the C's
/// order: the common keys, then the keypad's own.
pub fn named_keys(keypad: Keypad) -> impl Iterator<Item = (i32, Member)> {
    let own: &'static [(i32, Member)] = match keypad {
        Keypad::Standard => &STANDARD_KEYPAD_KEYS,
        Keypad::Apple => &APPLE_KEYPAD_KEYS,
    };
    COMMON_KEYS.iter().chain(own).copied()
}

/// The member `keypad` reads the numpad template from.
pub const fn numpad_template(keypad: Keypad) -> Member {
    match keypad {
        Keypad::Standard => Member::NumpadNum,
        Keypad::Apple => Member::AppleKeypadNum,
    }
}

/// `NUM0`-`NUM9` and the digits that replace `%1` for each, in the C's
/// order (`ADD_NUMPAD_NUM(0)` .. `(9)`).
pub const NUMPAD_KEYS: [(i32, &str); 10] = [
    (OBS_KEY_NUM0, "0"),
    (OBS_KEY_NUM1, "1"),
    (OBS_KEY_NUM2, "2"),
    (OBS_KEY_NUM3, "3"),
    (OBS_KEY_NUM4, "4"),
    (OBS_KEY_NUM5, "5"),
    (OBS_KEY_NUM6, "6"),
    (OBS_KEY_NUM7, "7"),
    (OBS_KEY_NUM8, "8"),
    (OBS_KEY_NUM9, "9"),
];

/// `MOUSE1`-`MOUSE29` and the digits that replace `%1` for each, in the
/// C's order (`ADD_MOUSE_NUM(1)` .. `(29)`).
pub const MOUSE_KEYS: [(i32, &str); 29] = [
    (OBS_KEY_MOUSE1, "1"),
    (OBS_KEY_MOUSE2, "2"),
    (OBS_KEY_MOUSE3, "3"),
    (OBS_KEY_MOUSE4, "4"),
    (OBS_KEY_MOUSE5, "5"),
    (OBS_KEY_MOUSE6, "6"),
    (OBS_KEY_MOUSE7, "7"),
    (OBS_KEY_MOUSE8, "8"),
    (OBS_KEY_MOUSE9, "9"),
    (OBS_KEY_MOUSE10, "10"),
    (OBS_KEY_MOUSE11, "11"),
    (OBS_KEY_MOUSE12, "12"),
    (OBS_KEY_MOUSE13, "13"),
    (OBS_KEY_MOUSE14, "14"),
    (OBS_KEY_MOUSE15, "15"),
    (OBS_KEY_MOUSE16, "16"),
    (OBS_KEY_MOUSE17, "17"),
    (OBS_KEY_MOUSE18, "18"),
    (OBS_KEY_MOUSE19, "19"),
    (OBS_KEY_MOUSE20, "20"),
    (OBS_KEY_MOUSE21, "21"),
    (OBS_KEY_MOUSE22, "22"),
    (OBS_KEY_MOUSE23, "23"),
    (OBS_KEY_MOUSE24, "24"),
    (OBS_KEY_MOUSE25, "25"),
    (OBS_KEY_MOUSE26, "26"),
    (OBS_KEY_MOUSE27, "27"),
    (OBS_KEY_MOUSE28, "28"),
    (OBS_KEY_MOUSE29, "29"),
];

/// The placeholder the digits replace.
pub const PLACEHOLDER: &[u8] = b"%1";

/// `is_padding()` of `util/dstr.c`: only `' '`, `'\t'`, `'\n'` and `'\r'`
/// (not `'\v'` or `'\f'`, which `u8::is_ascii_whitespace` would also take).
pub const fn is_padding(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

/// `strdepad()`: `s` without its leading and trailing padding.
pub fn depad(mut s: &[u8]) -> &[u8] {
    while let [first, rest @ ..] = s
        && is_padding(*first)
    {
        s = rest;
    }
    while let [rest @ .., last] = s
        && is_padding(*last)
    {
        s = rest;
    }
    s
}

/// Whether `template` has a `%1`: `dstr_find(&t, "%1")`.
pub fn has_placeholder(template: &[u8]) -> bool {
    template
        .windows(PLACEHOLDER.len())
        .any(|w| w == PLACEHOLDER)
}

/// The template the C expands, from a numpad or mouse member: depadded
/// (`dstr_copy`, `dstr_depad`), with `" %1"` appended when it has no `%1`.
/// `None` for an empty or padding-only template, which the C skips like a
/// `NULL` one (`dstr_is_empty`), so those keys keep their names.
pub fn prepare_template(template: &[u8]) -> Option<Vec<u8>> {
    let depadded = depad(template);
    if depadded.is_empty() {
        return None;
    }
    let mut out = depadded.to_vec();
    if !has_placeholder(&out) {
        out.extend_from_slice(b" ");
        out.extend_from_slice(PLACEHOLDER);
    }
    Some(out)
}

/// `dstr_replace(&button, "%1", digits)`: every `%1` in `template`, left
/// to right and without rescanning what was put in, replaced by `digits`.
/// Plain byte replacement, never a format.
pub fn replace_placeholder(template: &[u8], digits: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(template.len());
    let mut rest = template;
    while let Some((first, tail)) = rest.split_first() {
        if let Some(after) = rest.strip_prefix(PLACEHOLDER) {
            out.extend_from_slice(digits);
            rest = after;
        } else {
            out.push(*first);
            rest = tail;
        }
    }
    out
}

/// One key's name from a template member: [`prepare_template`], then
/// [`replace_placeholder`]. `None` where the C stores nothing.
pub fn expand(template: &[u8], digits: &str) -> Option<Vec<u8>> {
    prepare_template(template).map(|t| replace_placeholder(&t, digits.as_bytes()))
}

/// Expands `template` into each of `keys` and stores the result. `None`
/// (a `NULL`, empty or padding-only template) stores nothing.
fn store_expanded(
    template: Option<&CStr>,
    keys: &[(i32, &str)],
    store: &mut impl FnMut(i32, &CStr),
) {
    let Some(template) = template.and_then(|t| prepare_template(t.to_bytes())) else {
        return;
    };
    for (key, digits) in keys {
        let mut name = replace_placeholder(&template, digits.as_bytes());
        name.push(0);
        // The bytes come from a C string and ASCII digits, so the NUL just
        // pushed is the first one; as in C, the name ends at the first NUL.
        if let Ok(name) = CStr::from_bytes_until_nul(&name) {
            store(*key, name);
        }
    }
}

/// `obs_hotkeys_set_translations_s()` after its copy of the struct: calls
/// `store(key, name)` once for each name the C stores, in the C's order.
///
/// 1. Each named key of [`named_keys`] whose member is set, with the
///    member's string as it is (`ADD_TRANSLATION`).
/// 2. `NUM0`-`NUM9`, from the [`numpad_template`] member, when that
///    template is set and not empty or padding only ([`expand`]).
/// 3. `MOUSE1`-`MOUSE29`, from `mouse_num`, likewise.
pub fn for_each_store(
    translations: &Translations<'_>,
    keypad: Keypad,
    mut store: impl FnMut(i32, &CStr),
) {
    for (key, member) in named_keys(keypad) {
        if let Some(name) = translations.get(member) {
            store(key, name);
        }
    }
    store_expanded(
        translations.get(numpad_template(keypad)),
        &NUMPAD_KEYS,
        &mut store,
    );
    store_expanded(translations.get(Member::MouseNum), &MOUSE_KEYS, &mut store);
}
