//! Safe core of `libobs/obs-nix-x11-hotkeys.c.h`: the X11 key names and the
//! raw button decode of the X11 hotkey platform (`obs-nix-x11.c`, Linux and
//! FreeBSD).
//!
//! The X11 hotkey platform (still C) asks this code:
//! - [`route`], from `obs_key_to_str()`: how to name a key (a mouse button
//!   or numpad digit by number, a fixed key by its default name, a function
//!   key by number, or through its keycodes);
//! - [`raw_button`], for each event the hotkey poll drains with XInput:
//!   whether it is a raw button press or release, and the button's slot in
//!   the poll's per-button arrays.
//!
//! The C keeps the translations, the keycode lookups, every string and every
//! X call. It keeps no state, takes no lock, allocates nothing and logs
//! nothing.
//!
//! Intentional differences from C:
//! - the generic event test clears the send-event bit, [`SEND_EVENT`] (C
//!   clears decimal 80, 0x50, so it ignored a raw button event sent with the
//!   bit and took response types 51, 99 and 115 for generic events);
//! - button 0 is [`RawButton::Unsupported`] (C stored it at slot `0 - 1`,
//!   wrapped);
//! - a value outside `0 .. OBS_KEY_LAST_VALUE - 1` is [`Route::None`] (C
//!   indexed the keycode table with it).
//!
//! Characterized and kept: F1 to F35 never look up a translation, unlike
//! every other named key.

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

use std::ffi::CStr;

use crate::name_map::{self as k, OBS_KEY_LAST_VALUE};

/// `XINPUT_MOUSE_LEN`: the entries of each per-button array.
pub const MOUSE_LEN: usize = 33;

/// `XCB_GE_GENERIC`: the response type of a generic (extension) event.
pub const GE_GENERIC: u8 = 35;
/// The send-event bit of a response type: the event came from `SendEvent`.
pub const SEND_EVENT: u8 = 0x80;
/// `XCB_INPUT_RAW_BUTTON_PRESS`.
pub const RAW_BUTTON_PRESS: u16 = 15;
/// `XCB_INPUT_RAW_BUTTON_RELEASE`.
pub const RAW_BUTTON_RELEASE: u16 = 16;

/// How `obs_key_to_str()` names a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Nothing: the name is left as it is (a value that isn't a key).
    None,
    /// Mouse button `n` (1 to 29): its translation, else `Mouse n`.
    Mouse(i32),
    /// Numpad digit `n` (0 to 9): its translation, else `Numpad n`.
    Numpad(i32),
    /// Its translation, with this default name.
    Fixed(&'static CStr),
    /// Function key `n` (1 to 35): `Fn`, with no translation.
    Function(i32),
    /// The first keycode of the key that X names, else its key name.
    Keycodes,
}

/// What the hotkey poll does with one drained event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawButton {
    /// Nothing.
    Ignore,
    /// The button at this slot is pressed and updated in this poll.
    Press(usize),
    /// The button at this slot is updated in this poll.
    Release(usize),
    /// A raw button event for a button with no slot: logged.
    Unsupported,
}

/// How to name `key` (`obs_key_t`): the tests of `obs_nix_x11_key_to_str()`,
/// in its order (mouse buttons, numpad digits, fixed keys, function keys,
/// then the keycodes). [`Route::None`] for a value outside the key table.
pub fn route(key: i32) -> Route {
    if !usize::try_from(key).is_ok_and(|key| key < OBS_KEY_LAST_VALUE) {
        return Route::None;
    }
    match key {
        k::OBS_KEY_MOUSE1..=k::OBS_KEY_MOUSE29 => {
            Route::Mouse(key.wrapping_sub(k::OBS_KEY_MOUSE1).wrapping_add(1))
        }
        k::OBS_KEY_NUM0..=k::OBS_KEY_NUM9 => Route::Numpad(key.wrapping_sub(k::OBS_KEY_NUM0)),
        k::OBS_KEY_INSERT => Route::Fixed(c"Insert"),
        k::OBS_KEY_DELETE => Route::Fixed(c"Delete"),
        k::OBS_KEY_HOME => Route::Fixed(c"Home"),
        k::OBS_KEY_END => Route::Fixed(c"End"),
        k::OBS_KEY_PAGEUP => Route::Fixed(c"Page Up"),
        k::OBS_KEY_PAGEDOWN => Route::Fixed(c"Page Down"),
        k::OBS_KEY_NUMLOCK => Route::Fixed(c"Num Lock"),
        k::OBS_KEY_SCROLLLOCK => Route::Fixed(c"Scroll Lock"),
        k::OBS_KEY_CAPSLOCK => Route::Fixed(c"Caps Lock"),
        k::OBS_KEY_BACKSPACE => Route::Fixed(c"Backspace"),
        k::OBS_KEY_TAB => Route::Fixed(c"Tab"),
        k::OBS_KEY_PRINT => Route::Fixed(c"Print"),
        k::OBS_KEY_PAUSE => Route::Fixed(c"Pause"),
        k::OBS_KEY_LEFT => Route::Fixed(c"Left"),
        k::OBS_KEY_RIGHT => Route::Fixed(c"Right"),
        k::OBS_KEY_UP => Route::Fixed(c"Up"),
        k::OBS_KEY_DOWN => Route::Fixed(c"Down"),
        k::OBS_KEY_SHIFT => Route::Fixed(c"Shift"),
        k::OBS_KEY_ALT => Route::Fixed(c"Alt"),
        k::OBS_KEY_CONTROL => Route::Fixed(c"Control"),
        k::OBS_KEY_META => Route::Fixed(c"Super"),
        k::OBS_KEY_MENU => Route::Fixed(c"Menu"),
        k::OBS_KEY_NUMASTERISK => Route::Fixed(c"Numpad *"),
        k::OBS_KEY_NUMPLUS => Route::Fixed(c"Numpad +"),
        k::OBS_KEY_NUMMINUS => Route::Fixed(c"Numpad -"),
        k::OBS_KEY_NUMCOMMA => Route::Fixed(c"Numpad ,"),
        k::OBS_KEY_NUMPERIOD => Route::Fixed(c"Numpad ."),
        k::OBS_KEY_NUMSLASH => Route::Fixed(c"Numpad /"),
        k::OBS_KEY_SPACE => Route::Fixed(c"Space"),
        k::OBS_KEY_ESCAPE => Route::Fixed(c"Escape"),
        k::OBS_KEY_F1..=k::OBS_KEY_F35 => {
            Route::Function(key.wrapping_sub(k::OBS_KEY_F1).wrapping_add(1))
        }
        _ => Route::Keycodes,
    }
}

/// The decode of a drained event whose response type is `response_type`:
/// the tests of `mouse_button_pressed()`'s event loop with XInput.
/// `event_type` (the generic event's extension event type) is asked for only
/// for a generic event, and `detail` (the raw event's button) only for a raw
/// button press or release. Buttons 1 to 32 have slots 0 to 31; any other
/// button is [`RawButton::Unsupported`].
pub fn raw_button(
    response_type: u8,
    event_type: impl FnOnce() -> u16,
    detail: impl FnOnce() -> u32,
) -> RawButton {
    if response_type & !SEND_EVENT != GE_GENERIC {
        return RawButton::Ignore;
    }
    let pressed = match event_type() {
        RAW_BUTTON_PRESS => true,
        RAW_BUTTON_RELEASE => false,
        _ => return RawButton::Ignore,
    };
    match (slot(detail()), pressed) {
        (Some(slot), true) => RawButton::Press(slot),
        (Some(slot), false) => RawButton::Release(slot),
        (None, _) => RawButton::Unsupported,
    }
}

/// The slot of raw button `detail`: buttons 1 to `MOUSE_LEN - 1` have slots
/// 0 to `MOUSE_LEN - 2`.
fn slot(detail: u32) -> Option<usize> {
    let detail = usize::try_from(detail).ok()?;
    if detail < MOUSE_LEN {
        detail.checked_sub(1)
    } else {
        None
    }
}
