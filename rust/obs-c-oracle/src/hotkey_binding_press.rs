//! `oracle_*` build of `libobs/obs-hotkey-binding-press.c.h`
//! (oracle/hotkey_binding_press.c).
//!
//! A binding is an untyped pointer: the oracle reaches it only through
//! `obs_hotkey_binding_count_fields()` and calls
//! `obs_hotkey_dispatch()`, which the caller (the test binary) defines.

use core::ffi::c_void;

/// Index of each entry in `oracle_hotkey_binding_press_layout`, in the C's
/// order.
pub const LAYOUT_NAMES: [&str; 7] = [
    "sizeof(struct obs_hotkey_binding_count_fields)",
    "alignof(struct obs_hotkey_binding_count_fields)",
    "offsetof(struct obs_hotkey_binding_count_fields, pressed)",
    "offsetof(struct obs_hotkey_binding_count_fields, count)",
    "offsetof(struct obs_hotkey_binding_count_fields, hotkey)",
    "sizeof(bool)",
    "sizeof(int)",
];

unsafe extern "C" {
    pub fn oracle_obs_hotkey_binding_press(binding: *mut c_void);
    pub fn oracle_obs_hotkey_binding_release(binding: *mut c_void);
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_hotkey_binding_press_layout(which: usize) -> usize;
}
