//! ABI conversion; registry layout and callback ABI remain owned by C.
use core::ffi::c_void;

unsafe extern "C" {
    fn obs_hotkey_routed_callback_lock() -> bool;
    fn obs_hotkey_routed_callback_unlock();
    fn obs_hotkey_routed_callback_enabled() -> bool;
    fn obs_hotkey_routed_callback_find(id: usize) -> *mut c_void;
    fn obs_hotkey_routed_callback_call(hotkey: *mut c_void, id: usize, pressed: bool);
}

struct Registry;
impl crate::routed_callback::Registry for Registry {
    type Hotkey = *mut c_void;
    fn lock(&mut self) -> bool {
        // SAFETY: the existing seam checks core presence and locks its mutex.
        unsafe { obs_hotkey_routed_callback_lock() }
    }
    fn enabled(&self) -> bool {
        // SAFETY: the core reads this only while the registry mutex is held.
        unsafe { obs_hotkey_routed_callback_enabled() }
    }
    fn find(&mut self, id: usize) -> Option<Self::Hotkey> {
        // SAFETY: the successful lock keeps the registry live for this lookup.
        let hotkey = unsafe { obs_hotkey_routed_callback_find(id) };
        (!hotkey.is_null()).then_some(hotkey)
    }
    fn call(&mut self, hotkey: Self::Hotkey, id: usize, pressed: bool) {
        // SAFETY: the token came from the locked registry. C invokes its original
        // callback and data synchronously; nothing touches the token afterwards.
        unsafe { obs_hotkey_routed_callback_call(hotkey, id, pressed) };
    }
    fn unlock(&mut self) {
        // SAFETY: the core releases exactly one successful lock, after dispatch.
        unsafe { obs_hotkey_routed_callback_unlock() };
    }
}

/// Invoke a registered hotkey directly while callback rerouting is enabled.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_trigger_routed_callback(id: usize, pressed: bool) {
    crate::routed_callback::trigger(&mut Registry, id, pressed);
}
