//! `oracle_*` build of `libobs/obs-hotkey-pair.c.h`
//! (oracle/hotkey_pair.c).
//!
//! A pair and a hotkey are untyped pointers: the oracle reaches a pair only
//! through `obs_hotkey_pair_fields()` and `obs_hotkey_pair_call_func()`,
//! which the caller (the test binary) defines.

use core::ffi::c_void;

/// Index of each entry in `oracle_hotkey_pair_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 6] = [
    "sizeof(struct obs_hotkey_pair_fields)",
    "alignof(struct obs_hotkey_pair_fields)",
    "offsetof(struct obs_hotkey_pair_fields, pressed0)",
    "offsetof(struct obs_hotkey_pair_fields, pressed1)",
    "sizeof(bool)",
    "sizeof(obs_hotkey_id)",
];

unsafe extern "C" {
    pub fn oracle_obs_hotkey_pair_first_func(
        data: *mut c_void,
        id: usize,
        hotkey: *mut c_void,
        pressed: bool,
    );
    pub fn oracle_obs_hotkey_pair_second_func(
        data: *mut c_void,
        id: usize,
        hotkey: *mut c_void,
        pressed: bool,
    );
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_hotkey_pair_layout(which: usize) -> usize;
}
