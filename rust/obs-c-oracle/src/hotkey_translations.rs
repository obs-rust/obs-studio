//! `oracle_*` build of `libobs/obs-hotkey-translations.c.h`
//! (oracle/hotkey_translations.c), on the C dstr oracle.
//!
//! The struct is passed as an untyped pointer: callers pass the shim's
//! `#[repr(C)]` struct, whose layout `oracle_hotkey_translations_layout`
//! checks. The store, `obs_hotkeys_store_key_translation`, is the caller's:
//! the test binary defines it.

use core::ffi::c_void;

unsafe extern "C" {
    pub fn oracle_obs_hotkeys_set_translations_s(translations: *mut c_void, size: usize);
    /// 0: `sizeof`, 1: `_Alignof`, `2 + i`: the offset of member `i` in
    /// struct order; `usize::MAX` past the end.
    pub fn oracle_hotkey_translations_layout(which: usize) -> usize;
}
