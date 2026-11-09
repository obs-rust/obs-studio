//! Preserve the existing hotkey mutex around a borrowed C field read.
use core::ffi::c_char;

unsafe extern "C" {
    fn obs_hotkey_background_app_id_lock() -> bool;
    fn obs_hotkey_background_app_id_fields() -> *const *const c_char;
    fn obs_hotkey_background_app_id_unlock();
}

struct Background;
impl crate::background_app_id::BackgroundAppId for Background {
    type Id = *const c_char;
    fn lock(&mut self) -> bool {
        // SAFETY: the adapter checks core presence before acquiring its mutex.
        unsafe { obs_hotkey_background_app_id_lock() }
    }
    fn app_id(&self) -> Self::Id {
        // SAFETY: the safe query reads only after lock succeeded. The core
        // and the borrowed field are live under the existing hotkey mutex.
        unsafe { *obs_hotkey_background_app_id_fields() }
    }
    fn unlock(&mut self) {
        // SAFETY: the safe query releases exactly the successfully acquired lock.
        unsafe { obs_hotkey_background_app_id_unlock() };
    }
}

/// Borrow the background client application ID under the existing C API lifetime.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkeys_get_background_app_id() -> *const c_char {
    crate::background_app_id::query(&mut Background).unwrap_or(core::ptr::null())
}
