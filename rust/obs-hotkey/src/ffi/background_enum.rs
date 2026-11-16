//! The public enumeration over the live C list, under the existing hotkey mutex.
use super::nix_hotkey_state::obs_key_combination;
use core::ffi::{c_char, c_void};
use obs_util::ffi::darray::darray;

/// `struct obs_hotkey_background_shortcut` in `obs-internal.h`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct obs_hotkey_background_shortcut {
    pub combo: obs_key_combination,
    pub trigger: *mut c_char,
    pub bound: bool,
}

/// `obs_hotkey_background_enum_func` in `obs-hotkey.h`.
#[allow(non_camel_case_types)]
pub type obs_hotkey_background_enum_func = Option<
    unsafe extern "C" fn(
        data: *mut c_void,
        combo: obs_key_combination,
        trigger: *const c_char,
        bound: bool,
    ) -> bool,
>;

unsafe extern "C" {
    fn obs_hotkey_background_enum_lock() -> bool;
    fn obs_hotkey_background_enum_fields() -> *const darray;
    fn obs_hotkey_background_enum_unlock();
}

/// The core's shortcut list, read through the private adapter.
struct Live;
impl crate::background_enum::BackgroundShortcuts for Live {
    type Shortcut = obs_hotkey_background_shortcut;
    fn lock(&mut self) -> bool {
        // SAFETY: the adapter checks core presence before acquiring its mutex.
        unsafe { obs_hotkey_background_enum_lock() }
    }
    fn count(&self) -> usize {
        // SAFETY: called only while the mutex is held, so the core and its
        // list header are live; num is read again on every call.
        unsafe { (*obs_hotkey_background_enum_fields()).num }
    }
    fn get(&self, index: usize) -> Self::Shortcut {
        // SAFETY: the mutex is held and index is below the live count, so the
        // element lies inside the list's array of shortcuts.
        unsafe {
            *(*obs_hotkey_background_enum_fields())
                .array
                .cast::<obs_hotkey_background_shortcut>()
                .add(index)
        }
    }
    fn unlock(&mut self) {
        // SAFETY: the safe core releases exactly the successfully acquired lock.
        unsafe { obs_hotkey_background_enum_unlock() };
    }
}

/// # Safety
/// `func`, when not NULL, must be safe to call with `data` and each borrowed
/// shortcut, under the hotkey mutex, as for the original C API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkeys_enum_background_shortcuts(
    func: obs_hotkey_background_enum_func,
    data: *mut c_void,
) {
    crate::background_enum::enumerate(
        &mut Live,
        func.map(|func| {
            move |shortcut: obs_hotkey_background_shortcut| {
                // SAFETY: the caller's contract above; the trigger stays
                // borrowed from the list for the duration of the call.
                unsafe { func(data, shortcut.combo, shortcut.trigger, shortcut.bound) }
            }
        }),
    );
}
