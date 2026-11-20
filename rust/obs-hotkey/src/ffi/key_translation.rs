//! The key name store and its query over C-owned state:
//! `obs->hotkeys.translations[]` and libobs's allocator, behind the private
//! adapters of `libobs/obs-hotkey-key-translation-fields.c.h`.
//!
//! Both functions are declared in `obs-internal.h` without `EXPORT`, so they
//! stay hidden (`rust-hidden.d/obs-hotkey-key_translation.txt`). As in C,
//! no lock is taken and the key is not range-checked: the slot adapter
//! indexes the array exactly as the original did. `obs_key_t` is a C enum,
//! passed as `int`.
use core::ffi::{c_char, c_int};
use core::ptr::{NonNull, null, null_mut};

unsafe extern "C" {
    fn obs_hotkey_key_translation_slot(key: c_int) -> *mut *mut c_char;
    fn obs_hotkey_key_translation_free(translation: *mut c_char);
    fn obs_hotkey_key_translation_dup(translation: *const c_char) -> *mut c_char;
}

struct Core;
impl crate::key_translation::KeyTranslations for Core {
    type Text = NonNull<c_char>;
    fn get(&mut self, key: i32) -> Option<NonNull<c_char>> {
        // SAFETY: as for the C function, the core is live and `key` indexes
        // its translations array; the adapter reads `obs` once.
        NonNull::new(unsafe { *obs_hotkey_key_translation_slot(key) })
    }
    fn set(&mut self, key: i32, value: Option<NonNull<c_char>>) {
        // SAFETY: as in `get`; the slot is C-owned and nothing else holds it.
        unsafe {
            *obs_hotkey_key_translation_slot(key) = value.map_or(null_mut(), NonNull::as_ptr)
        };
    }
    fn release(&mut self, value: Option<NonNull<c_char>>) {
        // SAFETY: the slot held NULL or a bstrdup copy, freed exactly once here.
        unsafe { obs_hotkey_key_translation_free(value.map_or(null_mut(), NonNull::as_ptr)) };
    }
    fn duplicate(&mut self, value: NonNull<c_char>) -> Option<NonNull<c_char>> {
        // SAFETY: `value` is the caller's NUL-terminated string; the copy is
        // bmalloc-owned and stored as bstrdup returned it.
        NonNull::new(unsafe { obs_hotkey_key_translation_dup(value.as_ptr()) })
    }
}

/// The stored name of `key` (borrowed from the store), else `def`;
/// `OBS_KEY_NONE` returns NULL without touching the core.
///
/// # Safety
/// As for the C function: unless `key` is `OBS_KEY_NONE`, the core is live
/// and `key` is a valid `obs_key_t` below `OBS_KEY_LAST_VALUE`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_get_hotkey_translation(
    key: c_int,
    def: *const c_char,
) -> *const c_char {
    crate::key_translation::translation(&mut Core, key, NonNull::new(def.cast_mut()))
        .map_or(null(), |text| text.as_ptr().cast_const())
}

/// Replace the stored name of `key` with a bstrdup copy of `translation`,
/// or clear it for NULL, freeing the old name with bfree.
///
/// # Safety
/// As for the C function: the core is live, `key` is a valid `obs_key_t`
/// below `OBS_KEY_LAST_VALUE`, and `translation` is NULL or a NUL-terminated
/// string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkeys_store_key_translation(key: c_int, translation: *const c_char) {
    crate::key_translation::store(&mut Core, key, NonNull::new(translation.cast_mut()));
}
