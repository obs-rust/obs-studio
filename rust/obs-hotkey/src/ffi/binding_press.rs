//! C ABI for `libobs/obs-hotkey-binding-press.c.h`, the hotkey binding press
//! count, declared in `libobs/obs-internal.h` without `EXPORT`.
//!
//! The hotkey thread (a 25 ms poll), `obs_hotkey_inject_event()` (the frontend
//! while OBS has focus, and obs-websocket's `TriggerHotkeyByKeySequence`) and
//! `remove_bindings()` in `obs-hotkey.c.h` call these inside libobs, so the
//! symbols stay hidden (`rust-hidden.txt`). The bindings array and its walk,
//! the hotkey mutex, the callback and the router stay C.
//!
//! The shims run with the hotkey mutex held, on the hotkey thread or on
//! whatever thread injects or rebinds, and the callback runs inside the
//! dispatch. So they keep no state between calls and take no lock, and they
//! reach C only through libobs's hidden seams (`obs-internal.h`):
//! - `obs_hotkey_binding_count_fields()`, once per call: pointers to the
//!   binding's `pressed` flag, its hotkey's `pressed` count and the hotkey,
//!   so `struct obs_hotkey_binding` and `struct obs_hotkey` are never
//!   mirrored. The flag and the count are written through those pointers at
//!   the core's points, never through a copy;
//! - `obs_hotkey_dispatch()`, last and only when the core says to: it makes
//!   the rerouting choice (the hotkey's own func, or the router when
//!   callback rerouting is on, nothing when it is on and unset) and calls
//!   it. The binding and the hotkey are not touched after it, as in C: a
//!   callback may rebind or unregister hotkeys and so move the bindings
//!   array.
//!
//! Intentional differences from C, where C has undefined behavior: a null
//! `binding`, or a field pointer the accessor returned as null, does nothing
//! (C dereferences it). libobs never passes one: the bindings are elements of
//! its array. The count wraps at the edges of `i32` where C's `int`
//! arithmetic is undefined; libobs never reaches them.

use std::ffi::c_int;

use crate::binding_press as core;

/// `obs_hotkey_binding_t` (`struct obs_hotkey_binding`, `obs-internal.h`):
/// opaque here, reached only through [`obs_hotkey_binding_count_fields`].
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct obs_hotkey_binding {
    _private: [u8; 0],
}

/// `obs_hotkey_t` (`struct obs_hotkey`): opaque, passed on to the dispatch as
/// it is.
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct obs_hotkey_t {
    _private: [u8; 0],
}

/// `struct obs_hotkey_binding_count_fields` (`obs-internal.h`): the binding's
/// pressed flag, its hotkey's pressed count and the hotkey.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct obs_hotkey_binding_count_fields {
    pub pressed: *mut bool,
    pub count: *mut c_int,
    pub hotkey: *mut obs_hotkey_t,
}

unsafe extern "C" {
    fn obs_hotkey_binding_count_fields(
        binding: *mut obs_hotkey_binding,
    ) -> obs_hotkey_binding_count_fields;
    fn obs_hotkey_dispatch(hotkey: *mut obs_hotkey_t, pressed: bool);
}

/// One event of one binding: the flag and the count written at the C's
/// points, then the dispatch when the core says so.
///
/// # Safety
///
/// `binding` is null or a live binding of libobs's bindings array; the caller
/// holds the hotkey mutex.
unsafe fn binding_event(binding: *mut obs_hotkey_binding, pressed: bool) {
    if binding.is_null() {
        return;
    }
    // SAFETY: a live binding, per the caller.
    let fields = unsafe { obs_hotkey_binding_count_fields(binding) };
    if fields.pressed.is_null() || fields.count.is_null() || fields.hotkey.is_null() {
        return;
    }
    // SAFETY: the accessor's pointers point into the live binding and its
    // hotkey, which nothing else touches while the hotkey mutex is held; the
    // flag and the count are written through them, so no copy is written
    // back. A `&mut`'s exclusive range ends before the dispatch below.
    let dispatch = unsafe {
        let flag = &mut *fields.pressed;
        let count = &mut *fields.count;
        if pressed {
            core::press(flag, count)
        } else {
            core::release(flag, count)
        }
    };
    if dispatch {
        // SAFETY: a live hotkey, per the caller; libobs's dispatch makes the
        // rerouting choice and calls the callback or the router inside it.
        unsafe { obs_hotkey_dispatch(fields.hotkey, pressed) }
    }
}

/// `obs_hotkey_binding_press()`: the binding is pressed.
///
/// # Safety
///
/// As [`binding_event`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_binding_press(binding: *mut obs_hotkey_binding) {
    // SAFETY: per the caller.
    unsafe { binding_event(binding, true) }
}

/// `obs_hotkey_binding_release()`: the binding is released.
///
/// # Safety
///
/// As [`binding_event`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_binding_release(binding: *mut obs_hotkey_binding) {
    // SAFETY: per the caller.
    unsafe { binding_event(binding, false) }
}
