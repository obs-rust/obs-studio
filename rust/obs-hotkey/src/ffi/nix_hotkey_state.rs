//! C ABI for `libobs/obs-nix-hotkey-state.c.h`, declared in
//! `libobs/obs-nix-hotkey-state.h`.
//!
//! The Wayland hotkey platform (`obs-nix-wayland.c`) and the GlobalShortcuts
//! portal client (`obs-nix-portal-shortcuts.c`) call these inside libobs on
//! Linux and FreeBSD with `ENABLE_WAYLAND`. The C file has no `EXPORT`, so
//! the symbols stay hidden (`rust-hidden.txt`). Elsewhere the shims are built
//! but unused, and hidden all the same.
//!
//! The struct is the caller's, and the shortcut list is its `bmalloc`
//! darray, grown and erased as `da_push_back` and `da_erase` do. The lock is
//! `obs->hotkeys.mutex`, taken through libobs's hidden
//! `obs_hotkeys_mutex_lock` and `obs_hotkeys_mutex_unlock` (obs-internal.h)
//! at the points the C takes it; this file never reads `obs` itself. The
//! state is only borrowed after the lock is taken, as other threads change
//! it under that lock.
//!
//! `obs_key_t` is a C enum, passed as `int`.
//!
//! Intentional difference from C, where C has undefined behavior: a null
//! `state` returns (false from `flush` and `is_pressed`) without locking; C
//! dereferences it.

use std::ffi::c_int;
use std::ptr::NonNull;

use obs_util::ffi::darray::{darray, darray_ensure_capacity, darray_erase, darray_free};

use crate::nix_hotkey_state::{
    self as core, HotkeyLock, KeyCombination, OBS_KEY_LAST_VALUE, Shortcut, Shortcuts, Store,
};

unsafe extern "C" {
    fn obs_hotkeys_mutex_lock();
    fn obs_hotkeys_mutex_unlock();
}

/// `struct obs_key_combination` (`obs_key_combination_t`) in `obs-hotkey.h`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_key_combination {
    pub modifiers: u32,
    pub key: c_int,
}

/// `struct obs_nix_hotkey_shortcut` in `obs-nix-hotkey-state.h`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_nix_hotkey_shortcut {
    pub combo: obs_key_combination,
    pub observed: bool,
    pub release_pending: bool,
}

/// `DARRAY(struct obs_nix_hotkey_shortcut)`: a `struct darray` of
/// [`obs_nix_hotkey_shortcut`] items, `bmalloc`ed.
///
/// Safe code only makes an empty one ([`Default`]); any other comes from C,
/// whose darray is valid, so the methods below rely on that.
#[repr(transparent)]
#[derive(Debug)]
pub struct ShortcutArray(darray);

impl Default for ShortcutArray {
    fn default() -> Self {
        Self(darray {
            array: std::ptr::null_mut(),
            num: 0,
            capacity: 0,
        })
    }
}

impl ShortcutArray {
    /// The raw darray: buffer, item count and capacity.
    pub fn raw(&self) -> &darray {
        &self.0
    }

    /// The items, in order.
    pub fn items(&self) -> &[obs_nix_hotkey_shortcut] {
        if self.0.num == 0 {
            return &[];
        }
        // SAFETY: a non-empty darray holds `num` initialized items.
        unsafe { std::slice::from_raw_parts(self.0.array.cast(), self.0.num) }
    }

    fn item(&self, idx: usize) -> *mut obs_nix_hotkey_shortcut {
        assert!(idx < self.0.num, "shortcut {idx} of {}", self.0.num);
        // SAFETY: `idx` is below `num`, inside the buffer.
        unsafe { self.0.array.cast::<obs_nix_hotkey_shortcut>().add(idx) }
    }
}

const ITEM: usize = size_of::<obs_nix_hotkey_shortcut>();

impl Shortcuts for ShortcutArray {
    fn len(&self) -> usize {
        self.0.num
    }

    fn get(&self, idx: usize) -> Shortcut {
        // SAFETY: `item` checked the index.
        let c = unsafe { self.item(idx).read() };
        Shortcut {
            combo: KeyCombination {
                modifiers: c.combo.modifiers,
                key: c.combo.key,
            },
            observed: c.observed,
            release_pending: c.release_pending,
        }
    }

    fn set(&mut self, idx: usize, shortcut: Shortcut) {
        // SAFETY: `item` checked the index; `&mut self` is exclusive.
        unsafe { self.item(idx).write(c_shortcut(shortcut)) };
    }

    /// `darray_push_back`: count the item, ensure that capacity, then copy
    /// it to the end.
    fn push(&mut self, shortcut: Shortcut) {
        self.0.num += 1;
        let num = self.0.num;
        // SAFETY: a valid darray of `ITEM`-byte items.
        unsafe { darray_ensure_capacity(ITEM, &raw mut self.0, num) };
        // SAFETY: the capacity now covers index `num - 1`.
        unsafe { self.item(num - 1).write(c_shortcut(shortcut)) };
    }

    fn erase(&mut self, idx: usize) {
        // SAFETY: a valid darray of `ITEM`-byte items.
        unsafe { darray_erase(ITEM, &raw mut self.0, idx) };
    }

    fn free(&mut self) {
        // SAFETY: the buffer is `bmalloc`ed (or null).
        unsafe { darray_free(&raw mut self.0) };
    }
}

fn c_shortcut(s: Shortcut) -> obs_nix_hotkey_shortcut {
    obs_nix_hotkey_shortcut {
        combo: obs_key_combination {
            modifiers: s.combo.modifiers,
            key: s.combo.key,
        },
        observed: s.observed,
        release_pending: s.release_pending,
    }
}

/// `struct obs_nix_hotkey_state` in `obs-nix-hotkey-state.h`.
#[repr(C)]
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub struct obs_nix_hotkey_state {
    pub focused_keys: [bool; OBS_KEY_LAST_VALUE],
    pub focused_modifiers: u32,
    pub shortcuts: ShortcutArray,
}

impl Default for obs_nix_hotkey_state {
    /// A zeroed struct, as the C callers start it.
    fn default() -> Self {
        Self {
            focused_keys: [false; OBS_KEY_LAST_VALUE],
            focused_modifiers: 0,
            shortcuts: ShortcutArray::default(),
        }
    }
}

/// The caller's struct, borrowed one field at a time, after the lock.
struct CState(NonNull<obs_nix_hotkey_state>);

impl Store for CState {
    type Shortcuts = ShortcutArray;
    fn focused_keys(&mut self) -> &mut [bool] {
        // SAFETY: a live struct; the core asks only under the lock.
        unsafe { &mut (*self.0.as_ptr()).focused_keys }
    }
    fn focused_modifiers(&mut self) -> &mut u32 {
        // SAFETY: as above.
        unsafe { &mut (*self.0.as_ptr()).focused_modifiers }
    }
    fn shortcuts(&mut self) -> &mut ShortcutArray {
        // SAFETY: as above.
        unsafe { &mut (*self.0.as_ptr()).shortcuts }
    }
}

/// `obs->hotkeys.mutex`.
struct CoreLock;

impl HotkeyLock for CoreLock {
    fn lock(&self) {
        // SAFETY: libobs's lock accessor; the mutex is recursive.
        unsafe { obs_hotkeys_mutex_lock() };
    }
    fn unlock(&self) {
        // SAFETY: paired with `lock` by the core.
        unsafe { obs_hotkeys_mutex_unlock() };
    }
}

fn state(ptr: *mut obs_nix_hotkey_state) -> Option<CState> {
    NonNull::new(ptr).map(CState)
}

/// Same contract as C `obs_nix_hotkey_state_free`.
///
/// # Safety
///
/// `state` is null or a live `struct obs_nix_hotkey_state`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_hotkey_state_free(state: *mut obs_nix_hotkey_state) {
    if let Some(mut s) = self::state(state) {
        core::free(&mut s);
    }
}

/// Same contract as C `obs_nix_hotkey_state_set_key`.
///
/// # Safety
///
/// `state` is null or a live `struct obs_nix_hotkey_state`, changed by other
/// threads only under the hotkey lock.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_hotkey_state_set_key(
    state: *mut obs_nix_hotkey_state,
    key: c_int,
    pressed: bool,
) {
    if let Some(mut s) = self::state(state) {
        core::set_key(&mut s, &CoreLock, key, pressed);
    }
}

/// Same contract as C `obs_nix_hotkey_state_set_modifiers`.
///
/// # Safety
///
/// As [`obs_nix_hotkey_state_set_key`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_hotkey_state_set_modifiers(
    state: *mut obs_nix_hotkey_state,
    modifiers: u32,
) {
    if let Some(mut s) = self::state(state) {
        core::set_modifiers(&mut s, &CoreLock, modifiers);
    }
}

/// Same contract as C `obs_nix_hotkey_state_clear_keys`.
///
/// # Safety
///
/// As [`obs_nix_hotkey_state_set_key`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_hotkey_state_clear_keys(state: *mut obs_nix_hotkey_state) {
    if let Some(mut s) = self::state(state) {
        core::clear_keys(&mut s, &CoreLock);
    }
}

/// Same contract as C `obs_nix_hotkey_state_set_shortcut`.
///
/// # Safety
///
/// As [`obs_nix_hotkey_state_set_key`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_hotkey_state_set_shortcut(
    state: *mut obs_nix_hotkey_state,
    combo: obs_key_combination,
    active: bool,
) {
    if let Some(mut s) = self::state(state) {
        let combo = KeyCombination {
            modifiers: combo.modifiers,
            key: combo.key,
        };
        core::set_shortcut(&mut s, &CoreLock, combo, active);
    }
}

/// Same contract as C `obs_nix_hotkey_state_flush`.
///
/// # Safety
///
/// As [`obs_nix_hotkey_state_set_key`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_hotkey_state_flush(state: *mut obs_nix_hotkey_state) -> bool {
    self::state(state).is_some_and(|mut s| core::flush(&mut s, &CoreLock))
}

/// Same contract as C `obs_nix_hotkey_state_is_pressed`.
///
/// # Safety
///
/// As [`obs_nix_hotkey_state_set_key`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_nix_hotkey_state_is_pressed(
    state: *mut obs_nix_hotkey_state,
    key: c_int,
) -> bool {
    self::state(state).is_some_and(|mut s| core::is_pressed(&mut s, &CoreLock, key))
}
