//! C pointer and bmem ownership boundary for the owned descriptions.
use core::ffi::c_char;
use std::ffi::CStr;

use crate::owned_translations::{Storage, set_audio, set_sceneitem};
use obs_util::ffi::darray::{bfree, bmalloc};

unsafe extern "C" {
    fn obs_hotkey_owned_translation_field(slot: usize) -> *mut *mut c_char;
}

// Constructed only by the unsafe public setters under their C preconditions.
struct Fields;
impl Storage for Fields {
    type Input = *const c_char;
    type Owned = *mut c_char;
    fn release(&mut self, slot: usize) {
        // SAFETY: the core selects one of eight fields in the initialized C core;
        // each field is null or owns a bmem allocation, as in the original C.
        unsafe { bfree((*obs_hotkey_owned_translation_field(slot)).cast()) };
    }
    fn duplicate(&mut self, value: Self::Input) -> Self::Owned {
        if value.is_null() {
            return core::ptr::null_mut();
        }
        // SAFETY: the public setter requires a live NUL-terminated input. As in
        // bstrdup, include the terminator and allocate with the libobs allocator.
        unsafe {
            let bytes = CStr::from_ptr(value).to_bytes_with_nul();
            let copy = bmalloc(bytes.len()).cast::<c_char>();
            core::ptr::copy_nonoverlapping(value, copy, bytes.len());
            copy
        }
    }
    fn write(&mut self, slot: usize, value: Self::Owned) {
        // SAFETY: the initialized core owns this field and takes the duplicate.
        unsafe { *obs_hotkey_owned_translation_field(slot) = value };
    }
}

/// Replace all six audio descriptions.
///
/// # Safety
/// OBS is initialized; inputs are null or live NUL-terminated strings and do
/// not alias storage released by this call, as required by the original C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkeys_set_audio_hotkeys_translations(
    mute: *const c_char,
    unmute: *const c_char,
    push_to_mute: *const c_char,
    push_to_talk: *const c_char,
    monitor_on: *const c_char,
    monitor_off: *const c_char,
) {
    set_audio(
        &mut Fields,
        [
            mute,
            unmute,
            push_to_mute,
            push_to_talk,
            monitor_on,
            monitor_off,
        ],
    );
}

/// Replace both scene-item description templates.
///
/// # Safety
/// Same initialized-core and string lifetime requirements as the audio setter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkeys_set_sceneitem_hotkeys_translations(
    show: *const c_char,
    hide: *const c_char,
) {
    set_sceneitem(&mut Fields, [show, hide]);
}
