//! Safe core of `libobs/obs-nix-x11-input.c.h`: the X11 keysym table and
//! the mouse button answers of the X11 hotkey platform (`obs-nix-x11.c`,
//! Linux and FreeBSD).
//!
//! The X11 hotkey platform (still C) asks this code:
//! - [`keysym`], for every key at init (`fill_base_keysyms()`): the X keysym
//!   of the key, which fills the base keysym table that
//!   [`nix_x11_keys`](crate::nix_x11_keys) searches;
//! - [`mouse_pressed`], on each hotkey poll of a mouse key with XInput, after
//!   the C drained the raw button events into the poll's `pressed` and
//!   `update` arrays: whether the key's button is down, pressed in this poll
//!   or held from an earlier one; then [`commit`], which stores the poll's
//!   result in the held array;
//! - [`mask_pressed`], on each hotkey poll of a mouse key without XInput:
//!   whether the key's button bit is set in the QueryPointer reply's mask.
//!
//! It reads the caller's arrays and values, so the C keeps the raw event
//! drain, the array clears, the QueryPointer request and every X call. It
//! keeps no state, takes no lock, allocates nothing and logs nothing.
//!
//! The three per-button arrays (pressed in this poll, updated in this poll,
//! held from earlier polls) have [`MOUSE_LEN`] entries, `XINPUT_MOUSE_LEN`
//! in `obs-hotkey.h`: a mouse key reads [`slot`] in them.

use crate::name_map as k;

/// `XINPUT_MOUSE_LEN`: the entries of each per-button array.
pub const MOUSE_LEN: usize = 33;

/// `XCB_BUTTON_MASK_1`: the left button's bit in a QueryPointer mask.
pub const BUTTON_MASK_1: u16 = 0x100;
/// `XCB_BUTTON_MASK_2`: the middle button's bit.
pub const BUTTON_MASK_2: u16 = 0x200;
/// `XCB_BUTTON_MASK_3`: the right button's bit.
pub const BUTTON_MASK_3: u16 = 0x400;

/// `get_keysym()`: the X keysym of `key` (`obs_key_t`), 0 for a key it
/// doesn't name and for a value that isn't a key. Mouse buttons 1 to 5 are
/// their number shifted left by 16.
pub fn keysym(key: i32) -> i32 {
    let _ = key;
    todo!()
}

/// The slot an obs mouse key reads in the per-button arrays: Mouse 1 slot 0
/// (button 1), Mouse 2 slot 2 (the right button, 3), Mouse 3 slot 1 (the
/// middle button, 2), Mouse 4 to 29 slots 7 to 32 (buttons 8 to 33; buttons
/// 4 to 7, the wheel, have no key). `None` for any other key.
pub fn slot(key: i32) -> Option<usize> {
    let _ = key;
    todo!()
}

/// `mouse_button_pressed()`'s answer with XInput: the key's button pressed
/// in this poll or held from an earlier one; false for a key that isn't a
/// mouse key. `held` is read only when the button wasn't pressed in this
/// poll.
pub fn mouse_pressed(key: i32, pressed: &[bool; MOUSE_LEN], held: &[bool; MOUSE_LEN]) -> bool {
    let _ = (key, pressed, held);
    todo!()
}

/// `mouse_button_pressed()`'s commit with XInput: each slot updated in this
/// poll is held when it was pressed in this poll and released otherwise;
/// every other slot keeps its held state.
pub fn commit(
    update: &[bool; MOUSE_LEN],
    pressed: &[bool; MOUSE_LEN],
    held: &mut [bool; MOUSE_LEN],
) {
    let _ = (update, pressed, held);
    todo!()
}

/// `mouse_button_pressed()`'s answer without XInput: whether the key's
/// button bit is set in `buttons`, a QueryPointer reply's mask. Mouse 1 is
/// [`BUTTON_MASK_1`], Mouse 2 [`BUTTON_MASK_3`] and Mouse 3
/// [`BUTTON_MASK_2`]; any other key is never pressed.
pub fn mask_pressed(key: i32, buttons: u16) -> bool {
    let _ = (key, buttons, k::OBS_KEY_MOUSE1);
    todo!()
}
