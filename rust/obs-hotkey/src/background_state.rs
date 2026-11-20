//! The background hotkey state and its flags, read under the caller's
//! hotkey mutex. The mutex and both fields stay with the caller.

use core::cell::Cell;

/// `OBS_HOTKEYS_BACKGROUND_NATIVE`, also the answer when the lock fails.
pub const NATIVE: i32 = crate::background_query::NATIVE;

/// Access to the C-owned mutex and the two background fields.
pub trait BackgroundState {
    /// Take the hotkey mutex; false when there is no core.
    fn lock(&mut self) -> bool;
    /// The stored state, unchecked; read only while the lock is held.
    fn state(&mut self) -> i32;
    /// The stored flags, every bit; read only while the lock is held.
    fn flags(&mut self) -> u32;
    /// Release exactly the lock that succeeded.
    fn unlock(&mut self);
}

/// The background state, and its flags in `flags` when given.
///
/// `flags` is cleared before the lock is tried. A failed lock returns
/// [`NATIVE`] and reads nothing. Otherwise the state and then the flags are
/// read under the lock, the lock is released once, and the captured state is
/// returned, unknown values included.
pub fn query<B: BackgroundState>(backend: &mut B, flags: Option<&Cell<u32>>) -> i32 {
    if let Some(flags) = flags {
        flags.set(0);
    }
    if !backend.lock() {
        return NATIVE;
    }
    let state = backend.state();
    if let Some(flags) = flags {
        flags.set(backend.flags());
    }
    backend.unlock();
    state
}
