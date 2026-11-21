//! C ABI for `libobs/obs-hotkey-dispatch.c.h`, declared in
//! `libobs/obs-internal.h` without `EXPORT` and so hidden (`rust-hidden.d`).
//! The binding press count (`obs_hotkey_binding_press()` and
//! `obs_hotkey_binding_release()`) calls it with the hotkey mutex held.
//!
//! The core layout, the rerouting flag, the router slot and the callback ABI
//! stay in C behind the private adapters of
//! `libobs/obs-hotkey-dispatch-fields.c.h`; the hotkey stays opaque here and
//! is only passed on. Neither the rerouting nor the router adapter touches
//! the hotkey, so the no-router branch keeps C's no dereference of it.
use crate::ffi::binding_press::obs_hotkey_t;

unsafe extern "C" {
    fn obs_hotkey_dispatch_rerouting() -> bool;
    fn obs_hotkey_dispatch_has_router() -> bool;
    fn obs_hotkey_dispatch_call(hotkey: *mut obs_hotkey_t, pressed: bool);
    fn obs_hotkey_dispatch_route(hotkey: *mut obs_hotkey_t, pressed: bool);
}

struct Core;
impl crate::dispatch::Dispatch for Core {
    type Hotkey = *mut obs_hotkey_t;
    fn rerouting(&mut self) -> bool {
        // SAFETY: a live core is the caller's precondition, as in C.
        unsafe { obs_hotkey_dispatch_rerouting() }
    }
    fn has_router(&mut self) -> bool {
        // SAFETY: as for the rerouting flag.
        unsafe { obs_hotkey_dispatch_has_router() }
    }
    fn call(&mut self, hotkey: Self::Hotkey, pressed: bool) {
        // SAFETY: on this delivered path a live hotkey with a callback is the
        // caller's precondition, as in C; the callback runs inside the call.
        unsafe { obs_hotkey_dispatch_call(hotkey, pressed) }
    }
    fn route(&mut self, hotkey: Self::Hotkey, pressed: bool) {
        // SAFETY: a live hotkey is the caller's precondition on this path,
        // and the router was just seen set.
        unsafe { obs_hotkey_dispatch_route(hotkey, pressed) }
    }
}

/// `obs_hotkey_dispatch()`: deliver one event of `hotkey`.
///
/// # Safety
///
/// As for the C: a live libobs core; `hotkey` is live with a callback when
/// rerouting is off, and live when rerouting is on with a router set.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_dispatch(hotkey: *mut obs_hotkey_t, pressed: bool) {
    crate::dispatch::dispatch(&mut Core, hotkey, pressed);
}
