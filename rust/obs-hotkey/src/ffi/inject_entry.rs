//! Opaque registry access; the existing pointer-returning lock adapter owns the
//! seam lock. The field adapter has no global or runtime-symbol dependencies.
use core::ffi::c_void;

use super::nix_hotkey_state::obs_key_combination;
use super::binding_press::obs_hotkey_binding;
use crate::inject_entry::{self, Event, Registry};
use crate::press::KeyCombination;

pub type Lock = unsafe extern "C" fn() -> *mut c_void;
pub type Strict = unsafe extern "C" fn(*const c_void) -> bool;
pub type Visitor = unsafe extern "C" fn(*mut c_void, usize, *mut obs_hotkey_binding) -> bool;
pub type Walk = unsafe extern "C" fn(Visitor, *mut c_void);
pub type Inject = unsafe extern "C" fn(*mut obs_hotkey_binding, obs_key_combination, bool, bool);
pub type Unlock = unsafe extern "C" fn();

unsafe extern "C" {
    fn obs_hotkey_background_press_lock() -> *mut c_void;
    fn obs_hotkey_inject_entry_strict(core: *const c_void) -> bool;
    fn obs_hotkey_seam_enum_bindings(visitor: Visitor, data: *mut c_void);
    fn obs_hotkey_seam_unlock();
    fn obs_hotkey_binding_inject(
        binding: *mut obs_hotkey_binding,
        hotkey: obs_key_combination,
        pressed: bool,
        strict: bool,
    );
}

struct Live {
    core: *mut c_void,
    lock: Lock,
    strict: Strict,
    walk: Walk,
    inject: Inject,
    unlock: Unlock,
}

impl Registry for Live {
    fn lock(&mut self) -> bool {
        // SAFETY: the adapter returns a live core only after the seam locks it.
        self.core = unsafe { (self.lock)() };
        !self.core.is_null()
    }
    fn strict_modifiers(&self) -> bool {
        if self.core.is_null() {
            return false;
        }
        // SAFETY: successful lock keeps this core live until unlock.
        unsafe { (self.strict)(self.core) }
    }
    fn bindings(&mut self, event: Event) {
        let mut dispatch = (event, self.inject);
        // SAFETY: the C walk uses this stack context synchronously and retains
        // the existing C array snapshot. No borrow crosses the binding callback.
        unsafe { (self.walk)(visit, (&raw mut dispatch).cast()) };
    }
    fn unlock(&mut self) {
        // SAFETY: the core releases exactly one successful acquisition.
        unsafe { (self.unlock)() };
    }
}

unsafe extern "C" fn visit(
    data: *mut c_void,
    _index: usize,
    binding: *mut obs_hotkey_binding,
) -> bool {
    // SAFETY: Live::bindings keeps this copyable context live during the walk.
    let (event, inject) = unsafe { *data.cast::<(Event, Inject)>() };
    inject_entry::forward(event, binding, |binding, event| {
        // SAFETY: the C walk supplies a live binding under the hotkey lock.
        unsafe {
            inject(
                binding,
                obs_key_combination {
                    modifiers: event.hotkey.modifiers,
                    key: event.hotkey.key,
                },
                event.pressed,
                event.strict_modifiers,
            );
        }
    })
}

/// Original public ABI, including false injected events.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_inject_event(hotkey: obs_key_combination, pressed: bool) {
    obs_hotkey_inject_entry_request(hotkey, pressed);
}

/// Static entry for existing hotkey fixtures with their own core namespace.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_inject_entry_request(hotkey: obs_key_combination, pressed: bool) {
    // SAFETY: these existing seams and the field adapter share libobs's core;
    // all callbacks return synchronously while the recursive lock is held.
    unsafe {
        with_seams(
            hotkey,
            pressed,
            obs_hotkey_background_press_lock,
            obs_hotkey_inject_entry_strict,
            obs_hotkey_seam_enum_bindings,
            obs_hotkey_binding_inject,
            obs_hotkey_seam_unlock,
        );
    }
}

/// Same ABI conversion and dispatch for the real-header recorder/oracle worlds.
///
/// # Safety
/// The adapters must obey the same synchronous registry and lifetime contracts
/// as the production seams. A successful lock must return a live core pointer.
pub unsafe fn with_seams(
    hotkey: obs_key_combination,
    pressed: bool,
    lock: Lock,
    strict: Strict,
    walk: Walk,
    inject: Inject,
    unlock: Unlock,
) {
    inject_entry::inject(
        &mut Live {
            core: core::ptr::null_mut(),
            lock,
            strict,
            walk,
            inject,
            unlock,
        },
        KeyCombination {
            modifiers: hotkey.modifiers,
            key: hotkey.key,
        },
        pressed,
    );
}
