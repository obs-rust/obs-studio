//! obs-hotkey: the portal shortcut status rules
//! (`libobs/obs-nix-portal-shortcut-status.c.h`). Each sync of the Wayland
//! GlobalShortcuts portal client counts every binding's actions, decides
//! the outcome of its bind and marks each shortcut the desktop returns.
//! Each side runs 64 syncs: the action and overflow count of 16
//! combinations with up to 8 actions, the outcome of a bind with varying
//! inputs, and the trigger rule of 16 returned shortcuts, folded into a
//! checksum both sides must match. Nothing is allocated with `bmalloc`.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::portal_shortcut_status::{
    oracle_obs_nix_portal_status_action, oracle_obs_nix_portal_status_after_bind,
    oracle_obs_nix_portal_status_bound, oracle_obs_nix_portal_status_more,
};
use obs_hotkey::ffi::portal_shortcut_status::{
    obs_nix_portal_status_action, obs_nix_portal_status_after_bind, obs_nix_portal_status_bound,
    obs_nix_portal_status_more,
};
use std::ffi::c_int;
use std::hint::black_box;

const SYNCS: usize = 64;

/// The outcome's state and flags, folded.
trait Fold {
    fn fold(&self) -> u32;
}

impl Fold for obs_hotkey::ffi::portal_shortcut_status::ObsNixPortalOutcome {
    fn fold(&self) -> u32 {
        u32::from(self.mark_declined) << 3
            | u32::from(self.clear_declined) << 2
            | u32::from(self.set_partly) << 1
            | u32::from(self.partly)
    }
}

impl Fold for obs_c_oracle::portal_shortcut_status::OracleOutcome {
    fn fold(&self) -> u32 {
        u32::from(self.mark_declined) << 3
            | u32::from(self.clear_declined) << 2
            | u32::from(self.set_partly) << 1
            | u32::from(self.partly)
    }
}

type Action = unsafe extern "C" fn(usize) -> c_int;
type More = unsafe extern "C" fn(usize) -> usize;
type Bound = unsafe extern "C" fn(bool, bool, bool) -> c_int;

fn run<O: Fold>(
    action: Action,
    more: More,
    after_bind: unsafe extern "C" fn(bool, bool, bool) -> O,
    bound: Bound,
) -> u32 {
    let mut acc = 0_u32;
    for sync in 0..SYNCS {
        for combo in 0..16 {
            let actions = (sync + combo) % 9;
            for count in 1..=actions {
                // SAFETY: a plain value.
                let a = unsafe { action(black_box(count)) };
                acc = acc.wrapping_mul(31).wrapping_add(a as u32);
            }
            // SAFETY: a plain value.
            let m = unsafe { more(black_box(actions)) };
            acc = acc.wrapping_mul(31).wrapping_add(m as u32);
        }
        let b = |n: usize| (sync >> n) & 1 != 0;
        // SAFETY: plain values; the struct is returned by value.
        let outcome = unsafe { after_bind(b(0), b(1), b(2)) };
        acc = acc.wrapping_mul(31).wrapping_add(outcome.fold());
        for shortcut in 0..16 {
            // SAFETY: plain values.
            let c = unsafe { bound(b(1), b(0), shortcut % 3 != 0) };
            acc = acc.wrapping_mul(31).wrapping_add(c as u32);
        }
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(
        criterion,
        "portal_shortcut_status/sync/64",
        || {
            run(
                obs_nix_portal_status_action,
                obs_nix_portal_status_more,
                obs_nix_portal_status_after_bind,
                obs_nix_portal_status_bound,
            )
        },
        || {
            run(
                oracle_obs_nix_portal_status_action,
                oracle_obs_nix_portal_status_more,
                oracle_obs_nix_portal_status_after_bind,
                oracle_obs_nix_portal_status_bound,
            )
        },
    );
}
