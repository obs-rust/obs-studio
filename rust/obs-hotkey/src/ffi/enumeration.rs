//! Opaque C registry adapters; traversal and locking policy live in the safe core.
use super::binding_press::obs_hotkey_binding;
use super::pair::obs_hotkey_t;
use core::ffi::c_void;

pub type HotkeyCallback = unsafe extern "C" fn(*mut c_void, usize, *mut obs_hotkey_t) -> bool;
pub type BindingCallback =
    unsafe extern "C" fn(*mut c_void, usize, *mut obs_hotkey_binding) -> bool;
pub type ContextCallback = unsafe extern "C" fn(*mut c_void, *mut obs_hotkey_t) -> bool;

unsafe extern "C" {
    fn obs_hotkey_seam_lock() -> bool;
    fn obs_hotkey_seam_unlock();
    fn obs_hotkey_seam_find(id: usize) -> *mut obs_hotkey_t;
    fn obs_hotkey_seam_context_count(context: *const c_void) -> usize;
    fn obs_hotkey_seam_enum_bindings(func: BindingCallback, data: *mut c_void);
    fn obs_hotkey_enum_first() -> *mut obs_hotkey_t;
    fn obs_hotkey_enum_next(hotkey: *mut obs_hotkey_t) -> *mut obs_hotkey_t;
    fn obs_hotkey_enum_context_id(context: *const c_void, index: usize) -> usize;
    fn obs_hotkey_get_id(hotkey: *const obs_hotkey_t) -> usize;
}

fn present<T>(ptr: *mut T) -> Option<*mut T> {
    (!ptr.is_null()).then_some(ptr)
}

/// # Safety
/// `func` and `data` obey the public callback contract. The hotkey mutex keeps
/// registry links live; the successor is captured before calling the callback,
/// matching HASH_ITER even when the current hotkey is unregistered there.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_hotkeys(func: HotkeyCallback, data: *mut c_void) {
    crate::enumeration::locked(
        // SAFETY: the seam handles an absent core and acquires its mutex.
        || unsafe { obs_hotkey_seam_lock() },
        || {
            // SAFETY: the registry mutex is held throughout this traversal.
            let first = present(unsafe { obs_hotkey_enum_first() });
            crate::enumeration::hotkeys(
                first,
                // SAFETY: this node is live until the callback; cache its successor.
                |key| present(unsafe { obs_hotkey_enum_next(key) }),
                // SAFETY: the caller supplies the callback and data; key is live.
                |key| unsafe { func(data, obs_hotkey_get_id(key), key) },
            );
        },
        // SAFETY: called only after successful acquisition, after all callbacks.
        || unsafe { obs_hotkey_seam_unlock() },
    );
}

/// # Safety
/// `func` and `data` obey the public binding callback contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_enum_hotkey_bindings(func: BindingCallback, data: *mut c_void) {
    crate::enumeration::locked(
        // SAFETY: the seam handles an absent core and acquires its mutex.
        || unsafe { obs_hotkey_seam_lock() },
        // SAFETY: the original binding enumerator runs under the acquired mutex.
        || unsafe { obs_hotkey_seam_enum_bindings(func, data) },
        // SAFETY: called only after successful acquisition, after all callbacks.
        || unsafe { obs_hotkey_seam_unlock() },
    );
}

/// # Safety
/// The caller holds the hotkey mutex and supplies a live context, callback and
/// callback data, as required by the original internal helper. The callback
/// must preserve the context's ID storage, as in the C helper.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_seam_enum_context(
    context: *mut c_void,
    func: ContextCallback,
    data: *mut c_void,
) {
    // SAFETY: the caller holds the mutex and keeps the context live.
    let count = unsafe { obs_hotkey_seam_context_count(context) };
    crate::enumeration::context(
        count,
        // SAFETY: index is bounded by the captured count, and storage stays live.
        |index| unsafe { obs_hotkey_enum_context_id(context, index) },
        // SAFETY: lookup runs under the caller's hotkey mutex.
        |id| present(unsafe { obs_hotkey_seam_find(id) }),
        // SAFETY: callback/data obey the caller's contract; lookup returned a live key.
        |key| unsafe { func(data, key) },
    );
}
