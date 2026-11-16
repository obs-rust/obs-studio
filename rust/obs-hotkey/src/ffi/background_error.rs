//! Copy the real background error using libobs allocation under its mutex.
use core::ffi::{CStr, c_char};
use core::ptr::NonNull;
use obs_util::ffi::darray::bmalloc;

unsafe extern "C" {
    fn obs_hotkey_background_error_lock() -> bool;
    fn obs_hotkey_background_error_unlock();
    fn obs_hotkey_background_error_fields() -> *const *mut c_char;
}

struct Background;
impl crate::background_error::BackgroundError for Background {
    type Error = NonNull<c_char>;
    type Owned = *mut c_char;
    fn lock(&mut self) -> bool {
        // SAFETY: the existing seam checks core presence and acquires its mutex.
        unsafe { obs_hotkey_background_error_lock() }
    }
    fn error(&self) -> Option<Self::Error> {
        // SAFETY: the safe core reads this field only after acquiring the mutex.
        NonNull::new(unsafe { *obs_hotkey_background_error_fields() })
    }
    fn duplicate(&mut self, error: Self::Error) -> Self::Owned {
        // SAFETY: the locked backend owns this NUL-terminated C string. bmalloc
        // provides enough bytes or aborts; the fresh allocation cannot overlap.
        unsafe {
            let bytes = CStr::from_ptr(error.as_ptr()).to_bytes_with_nul();
            let copy = bmalloc(bytes.len()).cast::<c_char>();
            core::ptr::copy_nonoverlapping(bytes.as_ptr().cast(), copy, bytes.len());
            copy
        }
    }
    fn unlock(&mut self) {
        // SAFETY: the safe core releases exactly the successfully acquired lock.
        unsafe { obs_hotkey_background_error_unlock() };
    }
}

/// Return a bfree-owned error copy, or NULL if unavailable.
#[unsafe(no_mangle)]
pub extern "C" fn obs_hotkeys_get_background_error() -> *mut c_char {
    crate::background_error::query(&mut Background).unwrap_or(core::ptr::null_mut())
}
