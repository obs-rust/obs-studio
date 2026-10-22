//! `oracle_*` build of `libobs/obs-nix-portal-shortcut-sets.c.h`
//! (oracle/portal_shortcut_sets.c).
//!
//! The views are `struct obs_nix_portal_combos` and
//! `struct obs_nix_portal_descriptions`, whose layouts
//! `oracle_portal_sets_layout` reports.

use core::ffi::c_char;

pub use crate::nix_hotkey_state::OracleKeyCombination;

/// `struct obs_nix_portal_combos`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OracleCombos {
    pub array: *const OracleKeyCombination,
    pub num: usize,
}

/// `struct obs_nix_portal_descriptions`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OracleDescriptions {
    pub array: *const *const c_char,
    pub num: usize,
}

/// Index of each entry in `oracle_portal_sets_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 10] = [
    "sizeof(struct obs_nix_portal_combos)",
    "alignof(struct obs_nix_portal_combos)",
    "offsetof(struct obs_nix_portal_combos, array)",
    "offsetof(struct obs_nix_portal_combos, num)",
    "sizeof(struct obs_nix_portal_descriptions)",
    "alignof(struct obs_nix_portal_descriptions)",
    "offsetof(struct obs_nix_portal_descriptions, array)",
    "offsetof(struct obs_nix_portal_descriptions, num)",
    "sizeof(size_t)",
    "sizeof(bool)",
];

unsafe extern "C" {
    pub fn oracle_obs_nix_portal_sets_has(
        combos: *const OracleCombos,
        combo: OracleKeyCombination,
    ) -> bool;
    #[allow(clippy::too_many_arguments)]
    pub fn oracle_obs_nix_portal_sets_unchanged(
        session: bool,
        retry: bool,
        partly_declined: bool,
        current: *const OracleCombos,
        current_descriptions: *const OracleDescriptions,
        bound: *const OracleCombos,
        bound_descriptions: *const OracleDescriptions,
        declined: *const OracleCombos,
    ) -> bool;
    pub fn oracle_obs_nix_portal_sets_request(
        current: *const OracleCombos,
        partly_declined: bool,
        declined: *const OracleCombos,
        request: *mut usize,
        num_request: *mut usize,
    ) -> bool;
    pub fn oracle_obs_nix_portal_sets_partition(
        current: *const OracleCombos,
        approved: *const OracleCombos,
        request: *const OracleCombos,
        part: *mut usize,
        num_part: *mut usize,
        declined: *mut usize,
        num_declined: *mut usize,
    ) -> bool;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_portal_sets_layout(which: usize) -> usize;
}
