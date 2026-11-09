//! Private ABI bridge; all access occurs under the caller's hotkey lock.
use std::ffi::{c_char, c_int, c_void};

use super::pair::obs_hotkey_t;
use crate::unregister::{self as core, Registry};

unsafe extern "C" {
    fn obs_hotkey_unregister_next_id() -> usize;
    fn obs_hotkey_unregister_find(id: usize) -> *mut c_void;
    fn obs_hotkey_unregister_remove(key: *mut c_void);
    fn obs_hotkey_unregister_category(key: *mut c_void) -> c_int;
    fn obs_hotkey_unregister_owner(key: *mut c_void) -> *mut c_void;
    fn obs_hotkey_unregister_clear_owner(key: *mut c_void);
    fn obs_hotkey_unregister_name(key: *mut c_void) -> *mut c_void;
    fn obs_hotkey_unregister_description(key: *mut c_void) -> *mut c_void;
    fn obs_hotkey_seam_signal(signal: *const c_char, key: *mut obs_hotkey_t);
    fn obs_hotkey_seam_remove_bindings(id: usize) -> bool;
    fn obs_weak_source_release(owner: *mut c_void);
    fn obs_weak_output_release(owner: *mut c_void);
    fn obs_weak_encoder_release(owner: *mut c_void);
    fn obs_weak_service_release(owner: *mut c_void);
}

struct LockedRegistry;

impl Registry for LockedRegistry {
    type Key = *mut c_void;
    fn next_id(&self) -> usize {
        // SAFETY: the caller holds the registry lock, or the core is absent.
        unsafe { obs_hotkey_unregister_next_id() }
    }
    fn find(&mut self, id: usize) -> Option<Self::Key> {
        // SAFETY: the safe core has checked id < next_id, so the core exists.
        let key = unsafe { obs_hotkey_unregister_find(id) };
        (!key.is_null()).then_some(key)
    }
    fn remove(&mut self, key: Self::Key) {
        // SAFETY: find returned a live entry under the caller's lock.
        unsafe { obs_hotkey_unregister_remove(key) };
    }
    fn notify(&mut self, key: Self::Key) {
        // SAFETY: hash removal has not freed the key or any of its fields.
        unsafe { obs_hotkey_seam_signal(c"hotkey_unregister".as_ptr(), key.cast()) };
    }
    fn category(&self, key: Self::Key) -> i32 {
        // SAFETY: key remains live until free, as in the original C.
        unsafe { obs_hotkey_unregister_category(key) }
    }
    fn release(&mut self, key: Self::Key, category: i32) {
        // SAFETY: a live key; each weak release accepts its stored pointer or null.
        unsafe {
            let owner = obs_hotkey_unregister_owner(key);
            match category {
                1 => obs_weak_source_release(owner),
                2 => obs_weak_output_release(owner),
                3 => obs_weak_encoder_release(owner),
                4 => obs_weak_service_release(owner),
                _ => unreachable!("safe core selects only owner categories"),
            }
        }
    }
    fn clear_owner(&mut self, key: Self::Key) {
        // SAFETY: key remains live and exclusively locked.
        unsafe { obs_hotkey_unregister_clear_owner(key) };
    }
    fn remove_bindings(&mut self, id: usize) {
        // SAFETY: the caller holds the recursive lock; the key is still live.
        unsafe { obs_hotkey_seam_remove_bindings(id) };
    }
    fn free_name(&mut self, key: Self::Key) {
        // SAFETY: the live key owns its bmalloc-allocated name.
        unsafe { obs_util::ffi::darray::bfree(obs_hotkey_unregister_name(key)) };
    }
    fn free_description(&mut self, key: Self::Key) {
        // SAFETY: the live key owns its bmalloc-allocated description.
        unsafe { obs_util::ffi::darray::bfree(obs_hotkey_unregister_description(key)) };
    }
    fn free(&mut self, key: Self::Key) {
        // SAFETY: the key is detached, owner cleared and bindings removed.
        unsafe { obs_util::ffi::darray::bfree(key) };
    }
}

/// Release a key's weak owner without removing or freeing the key.
///
/// # Safety
/// `key` is a live key protected by the hotkey mutex.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_unregister_release_registerer(key: *mut c_void) {
    core::release_registerer(&mut LockedRegistry, key);
}

/// Remove one key from the already locked registry.
///
/// # Safety
/// The caller holds the hotkey mutex, or no core exists.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_unregister_one(id: usize) {
    core::unregister_hotkey(&mut LockedRegistry, id);
}
