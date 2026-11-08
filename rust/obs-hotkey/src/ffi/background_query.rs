//! The original public query, with C owning background status and identifiers.
use super::nix_hotkey_state::obs_key_combination;
use core::ffi::{c_char, c_int};
use obs_util::ffi::darray::bfree;

unsafe extern "C" {
    fn obs_hotkeys_get_background_state(flags: *mut u32) -> c_int;
    fn obs_hotkey_combination_get_background_id(combo: obs_key_combination) -> *mut c_char;
}

/// # Safety
/// `combo` must satisfy the active platform's identifier callback contract,
/// just as for the original C API. No identifier is retained.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkey_combination_is_background(combo: obs_key_combination) -> bool {
    // SAFETY: the status query explicitly permits a null flags output.
    let state = unsafe { obs_hotkeys_get_background_state(core::ptr::null_mut()) };
    crate::background_query::is_background(state, || {
        // SAFETY: the caller supplies a valid combination; the returned identifier
        // is owned by the caller and must be freed with libobs's allocator.
        let id = unsafe { obs_hotkey_combination_get_background_id(combo) };
        let present = !id.is_null();
        // SAFETY: bfree accepts both the returned allocation and null.
        unsafe { bfree(id.cast()) };
        present
    })
}
