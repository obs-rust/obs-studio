//! C ABI for `libobs/obs-hotkey-press.c.h`, the hotkey press state machine,
//! declared in `libobs/obs-internal.h` without `EXPORT`.
//!
//! `query_hotkey()`, `inject_hotkey()` and `query_hotkeys()` in
//! `obs-hotkey.c.h` call these inside libobs, once per binding, so the
//! symbols stay hidden (`rust-hidden.txt`). The bindings array and its
//! walk, the hotkey mutex, the pressed count and the callbacks stay C.
//!
//! The shims run with the hotkey mutex held, on the hotkey thread (a poll)
//! or on whatever thread calls `obs_hotkey_inject_event()`, and the press
//! and release callbacks run inside them. So they keep no state between
//! calls and take no lock, and they reach C only through libobs's hidden
//! seams (`obs-internal.h`):
//! - `obs_hotkey_binding_fields()`, once per call: pointers to the
//!   binding's key combination, pressed flag and `modifiers_match` latch,
//!   so `struct obs_hotkey_binding` is never mirrored. The fields are read
//!   once, before the step, and a changed latch is written back after it
//!   and before any press or release, as in C. (C forces the non-strict
//!   latch before the key state query; nothing between reads it.)
//! - `obs_hotkey_key_is_pressed()`, the platform key state, called at the
//!   point and as often as the C calls it (the step's closure);
//! - `obs_hotkey_binding_press()` and `obs_hotkey_binding_release()`, last:
//!   the binding is not touched after them, as in C, since a callback may
//!   rebind or unregister hotkeys and so move the bindings array.
//!
//! `obs_key_t` is a C enum, passed as `int`.
//!
//! Intentional differences from C:
//! - the C's `handle_binding()` took a `bool *pressed` that every caller
//!   passed as `NULL` and that was only aliased to the modifier-only flag.
//!   It is gone (it was never part of the entry points, so no signature
//!   changes); the core branches on `OBS_KEY_NONE`, which is the same.
//! - where C has undefined behavior: a null `binding`, or a field pointer
//!   the accessor returned as null, does nothing (C dereferences it).
//!   libobs never passes one: the bindings are elements of its array.

use std::ffi::c_int;

use super::nix_hotkey_state::obs_key_combination;
use crate::press::{self as core, Action, Binding, KeyCombination};

/// `obs_hotkey_binding_t` (`struct obs_hotkey_binding`, `obs-internal.h`):
/// opaque here, reached only through [`obs_hotkey_binding_fields`].
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct obs_hotkey_binding {
    _private: [u8; 0],
}

/// `struct obs_hotkey_binding_fields` (`obs-internal.h`): pointers into one
/// binding. The state machine only reads the key and the pressed flag, and
/// reads and writes the latch.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct obs_hotkey_binding_fields {
    pub key: *const obs_key_combination,
    pub pressed: *const bool,
    pub modifiers_match: *mut bool,
}

unsafe extern "C" {
    fn obs_hotkey_binding_fields(binding: *mut obs_hotkey_binding) -> obs_hotkey_binding_fields;
    fn obs_hotkey_key_is_pressed(key: c_int) -> bool;
    fn obs_hotkey_binding_press(binding: *mut obs_hotkey_binding);
    fn obs_hotkey_binding_release(binding: *mut obs_hotkey_binding);
}

fn key_is_pressed(key: i32) -> bool {
    // SAFETY: libobs's hidden key state takes any key; the caller holds
    // the hotkey mutex, as the C does here.
    unsafe { obs_hotkey_key_is_pressed(key) }
}

/// The binding's fields, read once, with the pointer to write the latch
/// back to; `None` for a null binding or a null field pointer.
///
/// # Safety
///
/// `binding` is null or a live binding of libobs's bindings array.
unsafe fn read_binding(binding: *mut obs_hotkey_binding) -> Option<(Binding, *mut bool)> {
    if binding.is_null() {
        return None;
    }
    // SAFETY: a live binding, per the caller.
    let fields = unsafe { obs_hotkey_binding_fields(binding) };
    if fields.key.is_null() || fields.pressed.is_null() || fields.modifiers_match.is_null() {
        return None;
    }
    // SAFETY: the accessor's pointers point into the live binding, which
    // nothing else touches while the hotkey mutex is held.
    let (key, pressed, modifiers_match) = unsafe {
        (
            fields.key.read(),
            fields.pressed.read(),
            fields.modifiers_match.read(),
        )
    };
    let binding = Binding {
        key: KeyCombination {
            modifiers: key.modifiers,
            key: key.key,
        },
        pressed,
        modifiers_match,
    };
    Some((binding, fields.modifiers_match))
}

/// Writes the latch back if the step changed it, then takes the action.
///
/// # Safety
///
/// As [`read_binding`]; `latch` is the pointer it returned for `binding`.
unsafe fn finish(
    binding: *mut obs_hotkey_binding,
    before: Binding,
    after: Binding,
    latch: *mut bool,
    action: Action,
) {
    if after.modifiers_match != before.modifiers_match {
        // SAFETY: the binding's own latch, per the caller.
        unsafe { latch.write(after.modifiers_match) };
    }
    match action {
        Action::None => {}
        // SAFETY: a live binding; libobs's wrappers set its pressed flag,
        // count its hotkey and call the callback, as the C does here.
        Action::Press => unsafe { obs_hotkey_binding_press(binding) },
        // SAFETY: as above.
        Action::Release => unsafe { obs_hotkey_binding_release(binding) },
    }
}

/// The body of `query_hotkey()`: one poll of one binding.
///
/// # Safety
///
/// `binding` is null or a live binding of libobs's bindings array, and the
/// caller holds the hotkey mutex.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_binding_query(
    binding: *mut obs_hotkey_binding,
    modifiers: u32,
    no_press: bool,
    strict_modifiers: bool,
) {
    // SAFETY: per the caller.
    let Some((before, latch)) = (unsafe { read_binding(binding) }) else {
        return;
    };
    let mut after = before;
    let action = core::query(
        &mut after,
        modifiers,
        no_press,
        strict_modifiers,
        key_is_pressed,
    );
    // SAFETY: per the caller; `latch` came from `read_binding(binding)`.
    unsafe { finish(binding, before, after, latch, action) };
}

/// The decision of `inject_hotkey()`: one injected event for one binding.
///
/// # Safety
///
/// As [`obs_hotkey_binding_query`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_binding_inject(
    binding: *mut obs_hotkey_binding,
    hotkey: obs_key_combination,
    pressed: bool,
    strict_modifiers: bool,
) {
    // SAFETY: per the caller.
    let Some((before, latch)) = (unsafe { read_binding(binding) }) else {
        return;
    };
    let mut after = before;
    let event = KeyCombination {
        modifiers: hotkey.modifiers,
        key: hotkey.key,
    };
    let action = core::inject(&mut after, event, pressed, strict_modifiers);
    // SAFETY: per the caller; `latch` came from `read_binding(binding)`.
    unsafe { finish(binding, before, after, latch, action) };
}

/// The modifier gathering of `query_hotkeys()`: Shift, Control, Alt and
/// Meta, through the platform key state, in that order.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkeys_query_modifiers() -> u32 {
    core::query_modifiers(key_is_pressed)
}
