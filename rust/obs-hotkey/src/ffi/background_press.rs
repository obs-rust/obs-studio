//! Private field borrowing and synchronous C calldata dispatch.
use core::ffi::c_void;

use crate::background_press::{self, BackgroundPress};

unsafe extern "C" {
    fn obs_hotkey_background_press_lock() -> *mut c_void;
    fn obs_hotkey_background_press_disabled(core: *mut c_void) -> *mut bool;
    fn obs_hotkey_background_press_unlock();
    fn obs_hotkey_background_press_signal(core: *mut c_void, enabled: bool);
}

struct Background(*mut c_void);
impl BackgroundPress for Background {
    fn lock(&mut self) -> bool {
        // SAFETY: the adapter checks presence through the existing seam lock.
        self.0 = unsafe { obs_hotkey_background_press_lock() };
        !self.0.is_null()
    }
    fn disabled(&self) -> bool {
        if self.0.is_null() {
            return false;
        }
        // SAFETY: enable acquired this core's mutex; the private bool is live
        // and no Rust reference crosses a callback or unlock.
        unsafe { *obs_hotkey_background_press_disabled(self.0) }
    }
    fn set_disabled(&mut self, disabled: bool) {
        if self.0.is_null() {
            return;
        }
        // SAFETY: enable writes the same live field while holding its mutex.
        unsafe { *obs_hotkey_background_press_disabled(self.0) = disabled };
    }
    fn unlock(&mut self) {
        // SAFETY: enable releases exactly one successful seam lock. The core
        // remains live through the synchronous signal, as in the original C.
        unsafe { obs_hotkey_background_press_unlock() };
    }
    fn signal(&mut self, enabled: bool) {
        if self.0.is_null() {
            return;
        }
        // SAFETY: enable has unlocked; C owns the fixed stack calldata for the
        // entire synchronous dispatch. No pointer to the payload escapes.
        unsafe { obs_hotkey_background_press_signal(self.0, enabled) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_enable_background_press(enable: bool) {
    obs_hotkey_background_press_request(enable);
}

/// Explicit static entry for fixtures with their own private core namespace.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_background_press_request(enable: bool) {
    background_press::enable(&mut Background(core::ptr::null_mut()), enable);
}
