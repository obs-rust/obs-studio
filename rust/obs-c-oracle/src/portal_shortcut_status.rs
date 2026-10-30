//! `oracle_*` build of `libobs/obs-nix-portal-shortcut-status.c.h`
//! (oracle/portal_shortcut_status.c).
//!
//! The C enums are returned as `int` and `struct obs_nix_portal_outcome`
//! (four `bool`s) by value (the value getter below gives its layout).

use core::ffi::c_int;

/// `struct obs_nix_portal_outcome`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleOutcome {
    pub mark_declined: bool,
    pub clear_declined: bool,
    pub set_partly: bool,
    pub partly: bool,
}

/// Index of each value in `oracle_portal_status_value`, in the C's order.
pub const VALUE_NAMES: [&str; 16] = [
    "OBS_NIX_PORTAL_ACTION_FIRST",
    "OBS_NIX_PORTAL_ACTION_NEXT",
    "OBS_NIX_PORTAL_ACTION_UNNAMED",
    "OBS_NIX_PORTAL_REBIND_NONE",
    "OBS_NIX_PORTAL_REBIND_KEYS_DECLINED",
    "OBS_NIX_PORTAL_REBIND_DESCRIPTION_DECLINED",
    "OBS_NIX_PORTAL_CHANGE_KEEP",
    "OBS_NIX_PORTAL_CHANGE_SET",
    "OBS_NIX_PORTAL_CHANGE_CLEAR",
    "sizeof(enum obs_nix_portal_action)",
    "sizeof(struct obs_nix_portal_outcome)",
    "_Alignof(struct obs_nix_portal_outcome)",
    "offsetof(mark_declined)",
    "offsetof(clear_declined)",
    "offsetof(set_partly)",
    "offsetof(partly)",
];

unsafe extern "C" {
    pub fn oracle_obs_nix_portal_status_action(count: usize) -> c_int;
    pub fn oracle_obs_nix_portal_status_more(count: usize) -> usize;
    pub fn oracle_obs_nix_portal_status_has_description(i: usize, num: usize) -> bool;
    pub fn oracle_obs_nix_portal_status_rebind(num_part: usize, num_declined: usize) -> c_int;
    pub fn oracle_obs_nix_portal_status_keep_approved(
        request_declined: bool,
        nothing: bool,
    ) -> bool;
    pub fn oracle_obs_nix_portal_status_after_bind(
        bound: bool,
        partly: bool,
        kept: bool,
    ) -> OracleOutcome;
    pub fn oracle_obs_nix_portal_status_declined(request_declined: bool, nothing: bool) -> bool;
    pub fn oracle_obs_nix_portal_status_bound(
        desktop_keys: bool,
        bind_reply: bool,
        triggered: bool,
    ) -> c_int;
    pub fn oracle_obs_nix_portal_status_desktop_triggers(
        desktop_keys: bool,
        bind_reply: bool,
    ) -> c_int;
    pub fn oracle_obs_nix_portal_status_can_configure(version: u32) -> bool;
    /// The value named `VALUE_NAMES[which]`; -1 out of range.
    pub fn oracle_portal_status_value(which: usize) -> i64;
}
