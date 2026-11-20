//! The public background-state query over C-owned state: the existing
//! hotkey mutex and `obs->hotkeys.background`, behind the private adapters
//! of `libobs/obs-hotkey-background-state-fields.c.h`.
use core::cell::Cell;
use core::ffi::c_int;

unsafe extern "C" {
    fn obs_hotkeys_background_state_lock() -> bool;
    fn obs_hotkeys_background_state_read() -> c_int;
    fn obs_hotkeys_background_state_flags() -> u32;
    fn obs_hotkeys_background_state_unlock();
}

struct Core;
impl crate::background_state::BackgroundState for Core {
    fn lock(&mut self) -> bool {
        // SAFETY: the adapter checks core presence before taking its mutex.
        unsafe { obs_hotkeys_background_state_lock() }
    }
    fn state(&mut self) -> i32 {
        // SAFETY: the safe query reads the field only after lock succeeded;
        // the core is live under the existing mutex.
        unsafe { obs_hotkeys_background_state_read() }
    }
    fn flags(&mut self) -> u32 {
        // SAFETY: as for the state, between a successful lock and its unlock.
        unsafe { obs_hotkeys_background_state_flags() }
    }
    fn unlock(&mut self) {
        // SAFETY: the safe query releases exactly the successfully acquired lock.
        unsafe { obs_hotkeys_background_state_unlock() };
    }
}

/// Return the background hotkey state, and its flags through `flags` when
/// it is not NULL.
///
/// # Safety
/// As for the C API: `flags` is NULL or points to a writable `uint32_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkeys_get_background_state(flags: *mut u32) -> c_int {
    // SAFETY: the same contract, forwarded unchanged.
    unsafe { obs_hotkeys_background_state_request(flags) }
}

/// Private entry for isolated C fixtures. The public name is also exported by
/// libobs, and a Darwin two-level namespace binds a fixture's call to it
/// there instead of to the fixture's own recording adapters.
///
/// # Safety
/// As for [`obs_hotkeys_get_background_state`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_hotkeys_background_state_request(flags: *mut u32) -> c_int {
    // SAFETY: the caller passes NULL or a valid, writable uint32_t that it
    // does not access during the call.
    let flags = unsafe { flags.as_mut() }.map(Cell::from_mut);
    crate::background_state::query(&mut Core, flags)
}
