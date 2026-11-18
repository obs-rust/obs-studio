//! C ownership and the existing hotkey mutex around background trigger dispatch.
use core::ffi::{c_char, c_void};
use obs_util::ffi::dstr::dstr;

use super::nix_hotkey_state::obs_key_combination;
use crate::background_trigger::{self, BackgroundTrigger};
use crate::nix_hotkey_state::KeyCombination;

unsafe extern "C" {
    fn obs_hotkey_background_trigger_lock() -> *mut c_void;
    fn obs_hotkey_background_trigger_dispatch(
        core: *mut c_void,
        combo: obs_key_combination,
        trigger: *mut dstr,
    ) -> bool;
    fn obs_hotkey_background_trigger_unlock();
    fn obs_hotkey_background_trigger_release(trigger: *mut dstr);
}

struct Trigger(dstr);
impl Trigger {
    fn into_raw(mut self) -> *mut c_char {
        core::mem::replace(&mut self.0.array, core::ptr::null_mut())
    }
}
impl Drop for Trigger {
    fn drop(&mut self) {
        // SAFETY: this live dstr owns the callback's allocation or NULL. The
        // real C adapter uses dstr_free and the original libobs allocator.
        unsafe { obs_hotkey_background_trigger_release(&raw mut self.0) };
    }
}

struct Background(*mut c_void);
impl BackgroundTrigger for Background {
    type Trigger = Trigger;
    fn lock(&mut self) -> bool {
        // SAFETY: this adapter checks core presence and uses the existing seam.
        self.0 = unsafe { obs_hotkey_background_trigger_lock() };
        !self.0.is_null()
    }
    fn trigger(&mut self, combo: KeyCombination) -> (bool, Trigger) {
        let mut trigger = Trigger(dstr {
            array: core::ptr::null_mut(),
            len: 0,
            capacity: 0,
        });
        // Check before the C adapter can read any private core field.
        if self.0.is_null() {
            return (false, trigger);
        }
        // SAFETY: query acquired this core's mutex; the adapter invokes the
        // callback with the exact combo and stored wake_data while it stays held.
        let found = unsafe {
            obs_hotkey_background_trigger_dispatch(
                self.0,
                obs_key_combination {
                    modifiers: combo.modifiers,
                    key: combo.key,
                },
                &raw mut trigger.0,
            )
        };
        (found, trigger)
    }
    fn unlock(&mut self) {
        // SAFETY: query releases exactly one successfully acquired mutex.
        unsafe { obs_hotkey_background_trigger_unlock() };
        self.0 = core::ptr::null_mut();
    }
}

/// Return the callback's exact allocation, owned by the caller and freed by bfree.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_combination_get_background_trigger(
    combo: obs_key_combination,
) -> *mut c_char {
    obs_hotkey_background_trigger_request(combo)
}

/// The same getter, selected explicitly by standalone Rust-static consumers.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_background_trigger_request(combo: obs_key_combination) -> *mut c_char {
    background_trigger::query(
        &mut Background(core::ptr::null_mut()),
        KeyCombination {
            modifiers: combo.modifiers,
            key: combo.key,
        },
    )
    .map_or(core::ptr::null_mut(), Trigger::into_raw)
}
