//! obs-hotkey integration tests: hotkey (one module per former tests/*.rs file, #968).

mod bindings;
mod bindings_parity;
mod hotkey_name_map;
mod nix_hotkey_state;
mod nix_hotkey_state_parity;
mod parity;
mod portal_shortcut_ids;
mod portal_shortcut_ids_parity;
mod press;
mod press_parity;
mod translations;
mod translations_parity;

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

/// Cargo-test stand-in for libobs's hidden
/// `obs_hotkeys_store_key_translation` (obs-hotkey.c.h), which the
/// translations shim and its C oracle both call. It records each call per
/// thread, so each test sees only its own.
pub mod hotkey_store {
    use std::cell::RefCell;
    use std::ffi::{CStr, c_char, c_int};

    /// One store call: the key, the pointer passed and the string it
    /// points to (`None` for a null pointer).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Stored {
        pub key: i32,
        pub ptr: usize,
        pub name: Option<Vec<u8>>,
    }

    thread_local! {
        static CALLS: RefCell<Vec<Stored>> = const { RefCell::new(Vec::new()) };
    }

    /// The calls on this thread since the last `take`.
    pub fn take() -> Vec<Stored> {
        CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
    }

    /// # Safety
    ///
    /// `translation` is null or a NUL-terminated string.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn obs_hotkeys_store_key_translation(
        key: c_int,
        translation: *const c_char,
    ) {
        let name = (!translation.is_null()).then(|| {
            // SAFETY: the caller passes a NUL-terminated string.
            unsafe { CStr::from_ptr(translation) }.to_bytes().to_vec()
        });
        CALLS.with(|c| {
            c.borrow_mut().push(Stored {
                key,
                ptr: translation as usize,
                name,
            })
        });
    }
}
