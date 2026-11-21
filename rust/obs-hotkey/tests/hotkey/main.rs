//! obs-hotkey integration tests: hotkey (one module per former tests/*.rs file, #968).

mod background_app_id;
mod background_app_id_parity;
mod background_control;
mod background_control_parity;
mod background_enum;
mod background_enum_parity;
mod background_error;
mod background_error_parity;
mod background_press;
mod background_press_parity;
mod background_query;
mod background_query_parity;
mod background_state;
mod background_state_parity;
mod background_trigger;
mod background_trigger_parity;
mod binding_getters;
mod binding_getters_parity;
mod binding_press;
mod binding_press_parity;
mod bindings;
mod bindings_parity;
mod cocoa_keys;
mod cocoa_keys_parity;
mod combination_background_id;
mod combination_background_id_parity;
mod dispatch;
mod dispatch_parity;
mod enumeration;
mod enumeration_parity;
mod hotkey_name_map;
mod identity_getters;
mod identity_getters_parity;
mod identity_setters;
mod identity_setters_parity;
mod inject_entry;
mod inject_entry_parity;
mod key_combination;
mod key_combination_parity;
mod key_translation;
mod key_translation_parity;
mod nix_hotkey_state;
mod nix_hotkey_state_parity;
mod nix_wayland_key_names;
mod nix_wayland_key_names_parity;
mod nix_wayland_keys;
mod nix_wayland_keys_parity;
mod nix_wayland_seats;
mod nix_wayland_seats_parity;
mod nix_x11_hotkeys;
mod nix_x11_hotkeys_parity;
mod nix_x11_input;
mod nix_x11_input_parity;
mod nix_x11_keys;
mod nix_x11_keys_parity;
mod object_getters;
mod object_getters_parity;
mod owned_translations;
mod owned_translations_parity;
mod pair;
mod pair_meta;
mod pair_meta_parity;
mod pair_parity;
mod pair_register;
mod pair_register_parity;
mod parity;
mod portal_shortcut_ids;
mod portal_shortcut_ids_parity;
mod portal_shortcut_sets;
mod portal_shortcut_sets_parity;
mod portal_shortcut_status;
mod portal_shortcut_status_parity;
mod press;
mod press_parity;
mod register;
mod register_parity;
mod routed_callback;
mod routed_callback_parity;
mod routing_controls;
mod routing_controls_parity;
mod save_load;
mod save_load_parity;
mod translations;
mod translations_parity;
mod unregister;
mod unregister_entry;
mod unregister_entry_parity;
mod update_atomic;
mod update_atomic_parity;
mod windows_keys;
mod windows_keys_parity;

/// Cargo-test stand-ins for the two hidden seams of the ported binding press
/// count (`obs-hotkey-binding-press`): `obs_hotkey_binding_count_fields()`
/// and the delivery of `obs_hotkey_dispatch()`, which more than one test
/// module drives. Only one definition can exist in the binary, so the module
/// that owns the fake libobs world of the running test installs its hooks
/// here, per thread; the seams read them. `binding_press_parity` and
/// `press_parity` both do. The dispatch itself is the real Rust one
/// (`obs_hotkey::ffi::dispatch`); outside a dispatch probe its adapters in
/// the dispatch oracle (`hotkey_dispatch.c`) see rerouting off and forward
/// the callback call to [`deliver`](binding_press_hooks), this thread's hook.
pub mod binding_press_hooks {
    use std::cell::RefCell;

    use obs_hotkey::ffi::binding_press::{
        obs_hotkey_binding, obs_hotkey_binding_count_fields as Fields, obs_hotkey_t,
    };

    /// One fake world's accessor and dispatch, on raw pointers the world
    /// owns.
    #[derive(Debug, Clone, Copy)]
    pub struct Hooks {
        pub accessor: fn(*mut obs_hotkey_binding) -> Fields,
        pub dispatch: fn(*mut obs_hotkey_t, bool),
    }

    thread_local! {
        static HOOKS: RefCell<Option<Hooks>> = const { RefCell::new(None) };
    }

    unsafe extern "C" {
        fn oracle_hotkey_dispatch_fallback(
            fallback: Option<unsafe extern "C" fn(*mut obs_hotkey_t, bool)>,
        );
    }

    pub fn install(hooks: Hooks) {
        HOOKS.with(|h| *h.borrow_mut() = Some(hooks));
        // SAFETY: sets this thread's fallback; `deliver` reads this thread's
        // hooks.
        unsafe { oracle_hotkey_dispatch_fallback(Some(deliver)) };
    }

    pub fn clear() {
        HOOKS.with(|h| *h.borrow_mut() = None);
        // SAFETY: clears this thread's fallback.
        unsafe { oracle_hotkey_dispatch_fallback(None) };
    }

    fn hooks() -> Hooks {
        HOOKS.with(|h| h.borrow().expect("a binding press world"))
    }

    /// The binding accessor (`obs-hotkey.c.h`).
    ///
    /// # Safety
    ///
    /// `binding` points at a live fake binding of the installed world.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn obs_hotkey_binding_count_fields(
        binding: *mut obs_hotkey_binding,
    ) -> Fields {
        (hooks().accessor)(binding)
    }

    /// The delivered call of the real dispatch: records it in the installed
    /// world.
    ///
    /// # Safety
    ///
    /// `hotkey` points at a live fake hotkey of the installed world.
    unsafe extern "C" fn deliver(hotkey: *mut obs_hotkey_t, pressed: bool) {
        (hooks().dispatch)(hotkey, pressed)
    }
}

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

/// Records each call of libobs's hidden `obs_hotkeys_store_key_translation`,
/// which the translations shim and its C oracle both call. The store is the
/// real Rust one (`obs_hotkey::ffi::key_translation`); under cargo test its
/// C adapters run on the key-translation oracle's per-thread core, which
/// reports each store to this module, so each test sees only its own.
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

    unsafe extern "C" {
        fn oracle_hotkey_key_translation_record(
            hook: Option<unsafe extern "C" fn(c_int, *const c_char)>,
        );
    }

    /// The store frees the old name first (`NULL`), then copies the
    /// caller's string, if any.
    unsafe extern "C" fn record(key: c_int, translation: *const c_char) {
        CALLS.with(|c| {
            let mut calls = c.borrow_mut();
            if translation.is_null() {
                calls.push(Stored {
                    key,
                    ptr: 0,
                    name: None,
                });
            } else if let Some(last) = calls.last_mut() {
                // SAFETY: the store copies a NUL-terminated string.
                last.name = Some(unsafe { CStr::from_ptr(translation) }.to_bytes().to_vec());
                last.ptr = translation as usize;
            }
        });
    }

    /// The calls on this thread since the last `take`. Each `take` empties
    /// the fixture's name store and starts recording again.
    pub fn take() -> Vec<Stored> {
        // SAFETY: frees only names this thread's fixture stored.
        unsafe { oracle_hotkey_key_translation_record(Some(record)) };
        CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
    }
}
