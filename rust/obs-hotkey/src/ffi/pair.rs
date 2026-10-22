//! C ABI for `libobs/obs-hotkey-pair.c.h`, the hotkey pair press latch,
//! declared in `libobs/obs-internal.h` without `EXPORT`.
//!
//! `register_hotkey_pair_internal()` in `obs-hotkey.c.h` registers
//! `obs_hotkey_pair_first_func` and `obs_hotkey_pair_second_func` as the
//! callbacks of a pair's two hotkeys, with the pair as their data, so they
//! keep the C signature (`obs_hotkey_func`) and stay hidden
//! (`rust-hidden.txt`). The pair itself, its hash, the funcs and their
//! callback ABI stay C.
//!
//! The callbacks run with the hotkey mutex held: on the hotkey thread, in
//! `obs_hotkey_inject_event()`, or in `obs_hotkey_trigger_routed_callback()`
//! (the frontend's router and obs-websocket's `TriggerHotkeyByName`). So
//! they keep no state and take no lock, and they reach C only through
//! libobs's hidden seams (`obs-internal.h`):
//! - `obs_hotkey_pair_fields()`, once per call: pointers to the pair's two
//!   latches (`pressed0`, `pressed1`), so `struct obs_hotkey_pair` is never
//!   mirrored. The latches are read and written through those pointers at
//!   the C's points, never through a copy: the func may trigger either half
//!   of this pair from inside its call, which writes them;
//! - `obs_hotkey_pair_call_func()`, which calls `func[half]` with
//!   `data[half]`, the pair id, the hotkey and the pressed value, and
//!   returns its result. A half writes its own latch only after it returns.
//!
//! The `id` the callbacks get is the hotkey's, which the C ignores too (the
//! func gets the pair id from the wrapper).
//!
//! Intentional differences from C, where C has undefined behavior: a null
//! `data`, or a latch pointer the accessor returned as null, does nothing
//! (C dereferences it). libobs never passes one: the data is the pair it
//! registered.

use std::ffi::c_void;

use crate::pair::{self as core, Half, Latches};

/// `obs_hotkey_pair_t` (`struct obs_hotkey_pair`, `obs-internal.h`): opaque
/// here, reached only through [`obs_hotkey_pair_fields`].
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct obs_hotkey_pair {
    _private: [u8; 0],
}

/// `obs_hotkey_t` (`struct obs_hotkey`): opaque, passed on to the func.
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct obs_hotkey_t {
    _private: [u8; 0],
}

/// `struct obs_hotkey_pair_fields` (`obs-internal.h`): pointers to the
/// latch of the first and of the second half.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct obs_hotkey_pair_fields {
    pub pressed0: *mut bool,
    pub pressed1: *mut bool,
}

unsafe extern "C" {
    fn obs_hotkey_pair_fields(pair: *mut obs_hotkey_pair) -> obs_hotkey_pair_fields;
    fn obs_hotkey_pair_call_func(
        pair: *mut obs_hotkey_pair,
        half: usize,
        hotkey: *mut obs_hotkey_t,
        pressed: bool,
    ) -> bool;
}

/// The pair's latches, through the accessor's pointers. Every read and
/// write goes to the pair itself, so a write the func made while it ran is
/// what the next read sees, and nothing is written back from a copy.
struct PairLatches {
    fields: obs_hotkey_pair_fields,
}

impl PairLatches {
    fn ptr(&self, half: Half) -> *mut bool {
        match half {
            Half::First => self.fields.pressed0,
            Half::Second => self.fields.pressed1,
        }
    }
}

impl Latches for PairLatches {
    fn latched(&self, half: Half) -> bool {
        // SAFETY: non-null pointers into the live pair (checked when the
        // fields were taken); the hotkey mutex is held, and no Rust
        // reference to the latch exists, so a write by the func through
        // the same pointer does not alias one.
        unsafe { self.ptr(half).read() }
    }

    fn set_latched(&mut self, half: Half, latched: bool) {
        // SAFETY: as in `latched`.
        unsafe { self.ptr(half).write(latched) }
    }
}

/// One event of one half of the pair `data`.
///
/// # Safety
///
/// `data` is null or a live pair of libobs's pair hash; `hotkey` is passed
/// on to the func as it is; the caller holds the hotkey mutex.
unsafe fn pair_event(data: *mut c_void, hotkey: *mut obs_hotkey_t, pressed: bool, half: Half) {
    let pair = data.cast::<obs_hotkey_pair>();
    if pair.is_null() {
        return;
    }
    // SAFETY: a live pair, per the caller.
    let fields = unsafe { obs_hotkey_pair_fields(pair) };
    if fields.pressed0.is_null() || fields.pressed1.is_null() {
        return;
    }
    let mut latches = PairLatches { fields };
    core::event(&mut latches, half, pressed, |_| {
        // SAFETY: a live pair, per the caller; the wrapper calls the
        // half's func, which may trigger either half of this pair again.
        unsafe { obs_hotkey_pair_call_func(pair, half.index(), hotkey, pressed) }
    });
}

/// The callback of a pair's first hotkey (`func[0]`, `pressed0`).
///
/// # Safety
///
/// As a libobs hotkey callback: `data` is null or the pair it was
/// registered with, and the caller holds the hotkey mutex.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_pair_first_func(
    data: *mut c_void,
    _id: usize,
    hotkey: *mut obs_hotkey_t,
    pressed: bool,
) {
    // SAFETY: per the caller.
    unsafe { pair_event(data, hotkey, pressed, Half::First) }
}

/// The callback of a pair's second hotkey (`func[1]`, `pressed1`).
///
/// # Safety
///
/// As [`obs_hotkey_pair_first_func`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_pair_second_func(
    data: *mut c_void,
    _id: usize,
    hotkey: *mut obs_hotkey_t,
    pressed: bool,
) {
    // SAFETY: per the caller.
    unsafe { pair_event(data, hotkey, pressed, Half::Second) }
}
