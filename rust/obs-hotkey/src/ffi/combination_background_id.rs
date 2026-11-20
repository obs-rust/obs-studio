//! The public combination background-id query over C-owned state: the
//! existing hotkey mutex, the backend's callback slot and libobs's dstr.
use super::nix_hotkey_state::obs_key_combination;
use core::ffi::c_char;
use obs_util::ffi::dstr::dstr;

/// `bool (*shortcut_id)(obs_key_combination_t, struct dstr *)` in `obs-internal.h`.
pub type ShortcutId = unsafe extern "C" fn(obs_key_combination, *mut dstr) -> bool;

unsafe extern "C" {
    fn obs_hotkey_combination_background_id_lock() -> bool;
    fn obs_hotkey_combination_background_id_fields() -> *const Option<ShortcutId>;
    fn obs_hotkey_combination_background_id_unlock();
    fn obs_hotkey_combination_background_id_free(id: *mut dstr);
}

struct Background(obs_key_combination);
impl crate::combination_background_id::CombinationBackgroundId for Background {
    type Callback = ShortcutId;
    type Id = dstr;
    type Owned = *mut c_char;
    fn lock(&mut self) -> bool {
        // SAFETY: the adapter checks core presence before acquiring its mutex.
        unsafe { obs_hotkey_combination_background_id_lock() }
    }
    fn callback(&mut self) -> Option<ShortcutId> {
        // SAFETY: the safe query reads the slot only after lock succeeded; the
        // core and its callback table are live under the existing mutex.
        unsafe { *obs_hotkey_combination_background_id_fields() }
    }
    fn unlock(&mut self) {
        // SAFETY: the safe query releases exactly the successfully acquired lock.
        unsafe { obs_hotkey_combination_background_id_unlock() };
    }
    fn empty(&mut self) -> dstr {
        dstr {
            array: core::ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }
    fn invoke(&mut self, callback: ShortcutId, id: &mut dstr) -> bool {
        // SAFETY: the backend registered this callback for exactly this
        // signature; it receives the combination by value and an empty dstr
        // it may fill with libobs's allocator. No lock is held.
        unsafe { callback(self.0, id) }
    }
    fn free(&mut self, mut id: dstr) {
        // SAFETY: id is empty or was filled by the callback with libobs's allocator.
        unsafe { obs_hotkey_combination_background_id_free(&mut id) };
    }
    fn transfer(&mut self, id: dstr) -> *mut c_char {
        id.array
    }
}

/// Return the backend's bfree-owned identifier for `combo`, or NULL.
///
/// # Safety
/// As for the C API: a registered callback must accept `combo` and fill the
/// dstr only with libobs's allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_combination_get_background_id(
    combo: obs_key_combination,
) -> *mut c_char {
    crate::combination_background_id::query(&mut Background(combo)).unwrap_or(core::ptr::null_mut())
}
