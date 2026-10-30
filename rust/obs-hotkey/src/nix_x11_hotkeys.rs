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

use std::ffi::CStr;

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

/// How to name `key` (`obs_key_t`).
pub fn route(key: i32) -> Route {
    let _ = key;
    todo!()
}

/// The decode of a drained event whose response type is `response_type`.
/// `event_type` (the generic event's extension event type) is asked for only
/// for a generic event, and `detail` (the raw event's button) only for a raw
/// button press or release.
pub fn raw_button(
    response_type: u8,
    event_type: impl FnOnce() -> u16,
    detail: impl FnOnce() -> u32,
) -> RawButton {
    let _ = (response_type, event_type, detail);
    todo!()
}
