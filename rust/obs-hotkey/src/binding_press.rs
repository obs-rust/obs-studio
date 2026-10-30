//! Safe core of `libobs/obs-hotkey-binding-press.c.h`: the hotkey binding
//! press count.
//!
//! A hotkey can have several bindings (key combinations), and each binding is
//! pressed and released on its own: by the hotkey thread on every 25 ms poll,
//! by `obs_hotkey_inject_event()` (the frontend while OBS has focus, and
//! obs-websocket's `TriggerHotkeyByKeySequence`), and by `remove_bindings()`
//! when a pressed binding is rebound or its hotkey unregistered. The hotkey's
//! `pressed` count turns those binding events into hotkey events: its
//! callback runs on the first press and the last release only, so a hotkey
//! held through two bindings fires once.
//!
//! A step writes the binding's `pressed` flag and the hotkey's count, and
//! says whether the caller dispatches the hotkey event now:
//! - [`press`] sets the flag and tests the count *before* incrementing it, so
//!   only a press that leaves 0 fires;
//! - [`release`] clears the flag and tests the count *after* decrementing it,
//!   so only a release that reaches 0 fires;
//! - the event is dispatched after both fields are written, and nothing is
//!   written after it: a callback may rebind or unregister hotkeys, and it
//!   sees the new flag and the new count.
//!
//! The core keeps no state (it all lives in the binding and the hotkey),
//! takes no lock (every caller holds the hotkey mutex) and never sees a
//! pointer: it works on the flag and the count by reference, so the shim
//! reads and writes them through the accessor's pointers at these points.
//!
//! Kept, characterized, not endorsed (`test_hotkey_press_state`,
//! `test_hotkey_binding_press_steps`): neither function checks the flag, so a
//! press of a pressed binding counts again and a release at count 0 makes the
//! count negative. libobs presses only a released binding and releases only a
//! pressed one, so this is unreachable today; a fix is a separate change.
//!
//! The count is C's `int`, and computing it in `i32` would be undefined at
//! the edges that C leaves undefined; the core wraps instead, which libobs
//! never reaches.

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

/// One press of one binding: `press_released_binding()`, without the call.
///
/// Sets the binding's `pressed` flag and increments the hotkey's count
/// (wrapping, where C's `int` is undefined). Returns whether the caller
/// dispatches the hotkey's press: `true` only when the count was 0, which the
/// caller must do after both writes.
pub fn press(pressed: &mut bool, count: &mut i32) -> bool {
    *pressed = true;
    let before = *count;
    *count = before.wrapping_add(1);
    before == 0
}

/// One release of one binding: `release_pressed_binding()`, without the call.
///
/// Clears the binding's `pressed` flag and decrements the hotkey's count
/// (wrapping, where C's `int` is undefined). Returns whether the caller
/// dispatches the hotkey's release: `true` only when the count reaches 0,
/// after the decrement, which the caller must do after both writes.
pub fn release(pressed: &mut bool, count: &mut i32) -> bool {
    *pressed = false;
    let after = (*count).wrapping_sub(1);
    *count = after;
    after == 0
}
