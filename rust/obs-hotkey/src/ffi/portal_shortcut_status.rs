//! C ABI for `libobs/obs-nix-portal-shortcut-status.c.h`, declared in
//! `libobs/obs-nix-portal-shortcuts.h`.
//!
//! The GlobalShortcuts portal client (`obs-nix-portal-shortcuts.c`) calls
//! these inside libobs on Linux and FreeBSD with Wayland and GIO. The C
//! file has no `EXPORT`, so the symbols stay hidden (`rust-hidden.txt`).
//! Elsewhere the shims are built but unused, and hidden all the same.
//!
//! Only `bool`, `size_t`, `uint32_t`, C enums (passed as `int`) and
//! `struct obs_nix_portal_outcome` (four `bool`s) cross; nothing is read
//! through a pointer.
//! No intentional difference from C.

use std::ffi::c_int;

use crate::portal_shortcut_status::{self as core, BindResult};

/// `struct obs_nix_portal_outcome`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObsNixPortalOutcome {
    pub mark_declined: bool,
    pub clear_declined: bool,
    pub set_partly: bool,
    pub partly: bool,
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_portal_status_action(count: usize) -> c_int {
    core::action(count) as c_int
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_portal_status_more(count: usize) -> usize {
    core::more(count)
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_portal_status_has_description(i: usize, num: usize) -> bool {
    core::has_description(i, num)
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_portal_status_rebind(num_part: usize, num_declined: usize) -> c_int {
    core::rebind(num_part, num_declined) as c_int
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_portal_status_keep_approved(
    request_declined: bool,
    nothing: bool,
) -> bool {
    core::keep_approved(request_declined, nothing)
}

/// `partly` is read only when `bound`, `kept` only when not.
#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_portal_status_after_bind(
    bound: bool,
    partly: bool,
    kept: bool,
) -> ObsNixPortalOutcome {
    let result = if bound {
        BindResult::Bound { partly }
    } else {
        BindResult::Failed { kept }
    };
    let outcome = core::after_bind(result);
    ObsNixPortalOutcome {
        mark_declined: outcome.mark_declined,
        clear_declined: outcome.clear_declined,
        set_partly: outcome.partly.is_some(),
        partly: outcome.partly.unwrap_or(false),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_portal_status_declined(request_declined: bool, nothing: bool) -> bool {
    core::declined(request_declined, nothing)
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_portal_status_bound(
    desktop_keys: bool,
    bind_reply: bool,
    triggered: bool,
) -> c_int {
    core::bound(desktop_keys, bind_reply, triggered) as c_int
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_portal_status_desktop_triggers(
    desktop_keys: bool,
    bind_reply: bool,
) -> c_int {
    core::desktop_triggers(desktop_keys, bind_reply) as c_int
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_nix_portal_status_can_configure(version: u32) -> bool {
    core::can_configure(version)
}
