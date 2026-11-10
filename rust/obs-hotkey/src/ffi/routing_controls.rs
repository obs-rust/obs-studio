//! C ABI and private field conversion for the callback routing controls.
use core::ffi::c_void;

/// The nullable router from obs-hotkey.h; setters store it without calling it.
pub type Callback = Option<unsafe extern "C" fn(*mut c_void, usize, bool)>;

unsafe extern "C" {
    fn obs_hotkey_routing_lock() -> bool;
    fn obs_hotkey_routing_callback_field() -> *mut Callback;
    fn obs_hotkey_routing_data_field() -> *mut *mut c_void;
    fn obs_hotkey_routing_enabled_field() -> *mut bool;
    fn obs_hotkey_routing_unlock();
}

struct Registry;
impl crate::routing_controls::RoutingControls for Registry {
    type Callback = Callback;
    type Data = *mut c_void;
    fn lock(&mut self) -> bool {
        // SAFETY: the existing seam checks core presence and acquires its mutex.
        unsafe { obs_hotkey_routing_lock() }
    }
    fn set_callback(&mut self, callback: Callback, data: *mut c_void) {
        // SAFETY: the core calls this only after successful locking. Each pointer
        // addresses its live, correctly typed private C field under that lock.
        unsafe {
            *obs_hotkey_routing_callback_field() = callback;
            *obs_hotkey_routing_data_field() = data;
        }
    }
    fn set_enabled(&mut self, enabled: bool) {
        // SAFETY: the core holds the mutex and the real C boolean field is live.
        unsafe { *obs_hotkey_routing_enabled_field() = enabled };
    }
    fn unlock(&mut self) {
        // SAFETY: the core releases exactly one successful acquisition.
        unsafe { obs_hotkey_routing_unlock() };
    }
}

/// Store a nullable callback and opaque context under the registry mutex.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_set_callback_routing_func(callback: Callback, data: *mut c_void) {
    crate::routing_controls::set_callback(&mut Registry, callback, data);
}

/// Replace only the routing flag under the registry mutex.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkey_enable_callback_rerouting(enabled: bool) {
    crate::routing_controls::set_enabled(&mut Registry, enabled);
}
