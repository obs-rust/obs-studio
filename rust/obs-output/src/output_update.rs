//! Safe core of `libobs/obs-output-update.c.h`: an output's settings update.
//!
//! `obs-output.c` keeps `obs_output_valid()`'s pointer check, the settings
//! object and the plugin's callback; the shim passes the check's result and
//! reaches the rest through the live adapter `obs-output-update-fields.c.h`
//! and `obs_data_apply()`. No lock is taken, as in C, and the core holds
//! nothing of the output while the callback runs.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use core::ffi::CStr;

/// `obs_output_update()`: for a valid output, `apply` (the incoming settings
/// merged into the stored ones), then `call_update` when `has_update` (the
/// type's `info.update` is set). A NULL output (`valid == false`) logs and
/// does nothing else.
pub fn update(
    valid: bool,
    apply: impl FnOnce(),
    has_update: impl FnOnce() -> bool,
    call_update: impl FnOnce(),
    log: impl FnOnce(&'static CStr),
) {
    if !valid {
        log(c"obs_output_update");
        return;
    }
    apply();
    if has_update() {
        call_update();
    }
}
