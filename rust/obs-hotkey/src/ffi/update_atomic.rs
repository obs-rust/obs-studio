//! Atomic update ABI, using the existing recursive C registry lock.
use core::ffi::c_void;

/// The public C callback, nullable when locking fails.
pub type Callback = Option<unsafe extern "C" fn(*mut c_void)>;

unsafe extern "C" {
    fn obs_hotkey_seam_lock() -> bool;
    fn obs_hotkey_seam_unlock();
}

struct Registry;
impl crate::update_atomic::AtomicUpdate for Registry {
    fn lock(&mut self) -> bool {
        // SAFETY: the existing seam checks core presence and takes its mutex.
        unsafe { obs_hotkey_seam_lock() }
    }

    fn unlock(&mut self) {
        // SAFETY: the safe core releases exactly one successfully acquired lock.
        unsafe { obs_hotkey_seam_unlock() };
    }
}

/// Invoke the callback synchronously under the recursive hotkey mutex.
///
/// # Safety
/// If locking succeeds, callback must be nonnull and callable with data. Its
/// backing state must remain live until it returns, including recursive calls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_update_atomic(callback: Callback, data: *mut c_void) {
    crate::update_atomic::update(&mut Registry, |_| {
        // SAFETY: the caller supplies a valid callback when locking succeeds.
        unsafe { callback.unwrap_unchecked()(data) };
    });
}
