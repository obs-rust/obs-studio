//! obs-hotkey integration tests: hotkey (one module per former tests/*.rs file, #968).

mod hotkey_name_map;
mod nix_hotkey_state;
mod nix_hotkey_state_parity;
mod parity;

/// Cargo-test stand-ins for libobs's hidden `obs_hotkeys_mutex_lock` and
/// `obs_hotkeys_mutex_unlock` (obs-hotkey.c.h), which the Wayland key state
/// shim and its C oracle both call. They count calls per thread, so each
/// test sees only its own.
pub mod hotkey_mutex {
    use std::cell::Cell;

    thread_local! {
        static LOCKS: Cell<u64> = const { Cell::new(0) };
        static UNLOCKS: Cell<u64> = const { Cell::new(0) };
    }

    /// Lock and unlock calls on this thread so far.
    pub fn counts() -> (u64, u64) {
        (LOCKS.with(Cell::get), UNLOCKS.with(Cell::get))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn obs_hotkeys_mutex_lock() {
        LOCKS.with(|c| c.set(c.get() + 1));
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn obs_hotkeys_mutex_unlock() {
        UNLOCKS.with(|c| c.set(c.get() + 1));
    }
}
