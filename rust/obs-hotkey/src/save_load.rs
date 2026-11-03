//! The rules of `libobs/obs-hotkey-save-load.c.h`, the hotkey save and load
//! functions.
//!
//! The registry, the lock and the obs_data calls stay behind the C ABI in
//! [`crate::ffi::save_load`]. What is left to decide is here: which key a
//! saved volume binding means, when a load sends `hotkey_bindings_changed`,
//! and which saved bindings carry the volume key mark.

use std::ffi::CStr;

use crate::name_map::{OBS_KEY_VK_VOLUME_DOWN, OBS_KEY_VK_VOLUME_UP};

/// The signal a load sends.
pub const BINDINGS_CHANGED: &CStr = c"hotkey_bindings_changed";

/// The field that marks a volume binding saved since the key tables were
/// fixed (#2635): a binding in a scene collection without it may name the
/// other key.
pub const VOLUME_KEYS_FIXED: &CStr = c"volume_keys_fixed";

/// `is_volume_up_down()`.
pub fn is_volume_up_down(key: i32) -> bool {
    key == OBS_KEY_VK_VOLUME_UP || key == OBS_KEY_VK_VOLUME_DOWN
}

/// The key a loaded binding means, in the order of the C's condition: `fixed`
/// and `swapped` are read only if the rules before them leave the swap open.
///
/// `saved` is true for a scene collection's data. `fixed` reads the binding's
/// [`VOLUME_KEYS_FIXED`], `swapped` whether this host's key tables had the two
/// volume keys the wrong way round.
pub fn loaded_key(
    key: i32,
    saved: bool,
    fixed: impl FnOnce() -> bool,
    swapped: impl FnOnce() -> bool,
) -> i32 {
    if saved && is_volume_up_down(key) && !fixed() && swapped() {
        if key == OBS_KEY_VK_VOLUME_UP {
            OBS_KEY_VK_VOLUME_DOWN
        } else {
            OBS_KEY_VK_VOLUME_UP
        }
    } else {
        key
    }
}

/// Whether a load of `count` items sends [`BINDINGS_CHANGED`]. `removed`: the
/// caller removed bindings of this hotkey first, so an empty or missing array
/// still changed them. A rejected item counts: the rule looks at the array.
pub fn load_signals(count: usize, removed: bool) -> bool {
    count != 0 || removed
}

/// Whether a saved binding gets [`VOLUME_KEYS_FIXED`].
pub fn save_marks_fixed(saved: bool, key: i32) -> bool {
    saved && is_volume_up_down(key)
}
