//! Tier 3 parity: the shims against the original
//! `libobs/obs-nix-portal-shortcut-status.c.h`
//! (oracle/portal_shortcut_status.c).
//!
//! `test_portal_shortcut_status_steps` compiles the C into itself, so it
//! never sees the Rust swap (a variant linked against `libobs_rust-static`
//! does not link on its own: see
//! `test/cmocka/tests.d/test_portal_shortcut_status_steps.cmake`). This file
//! is what compares the two.
//!
//! - The values and layout: every enum value, the size of an enum, and the
//!   size, alignment and field offsets of
//!   `struct obs_nix_portal_outcome`.
//! - The action count and the overflow: every count 0 to 64, the counts
//!   around `usize::MAX`, and random counts.
//! - The description fallback: every position and count 0 to 8, the edges
//!   around `usize::MAX`, and random values.
//! - `bind_approved()`'s rule: every part and declined count 0 to 8, the
//!   edges, and random counts.
//! - The outcome and the state after a failed bind: exhaustive over all 8
//!   inputs of `obs_nix_portal_status_after_bind` (the struct field by
//!   field), and the 4 of `_keep_approved` and of `_declined`.
//! - The trigger rule: all 8 inputs of `_bound` and 4 of
//!   `_desktop_triggers`.
//! - The configure gate: `_can_configure` of versions 0 to 4, the `u32`
//!   edges and random versions, and the client's `take_configure()` gate
//!   built on each side's answer for every pending and session flag.
//!
//! No intentional difference: nothing is excluded.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use obs_c_oracle::portal_shortcut_status::{
    OracleOutcome, VALUE_NAMES, oracle_obs_nix_portal_status_action,
    oracle_obs_nix_portal_status_after_bind, oracle_obs_nix_portal_status_bound,
    oracle_obs_nix_portal_status_can_configure, oracle_obs_nix_portal_status_declined,
    oracle_obs_nix_portal_status_desktop_triggers, oracle_obs_nix_portal_status_has_description,
    oracle_obs_nix_portal_status_keep_approved, oracle_obs_nix_portal_status_more,
    oracle_obs_nix_portal_status_rebind, oracle_portal_status_value,
};
use obs_hotkey::ffi::portal_shortcut_status::{
    ObsNixPortalOutcome, obs_nix_portal_status_action, obs_nix_portal_status_after_bind,
    obs_nix_portal_status_bound, obs_nix_portal_status_can_configure,
    obs_nix_portal_status_declined, obs_nix_portal_status_desktop_triggers,
    obs_nix_portal_status_has_description, obs_nix_portal_status_keep_approved,
    obs_nix_portal_status_more, obs_nix_portal_status_rebind,
};
use obs_hotkey::portal_shortcut_status::{Action, Change, Rebind};
use proptest::prelude::*;

/// The `usize` values the counts are checked at: 0 to 64 and the edges.
fn counts() -> impl Iterator<Item = usize> {
    (0..=64).chain([usize::MAX - 2, usize::MAX - 1, usize::MAX])
}

fn c_value(which: usize) -> i64 {
    // SAFETY: the oracle getter reads only its own table.
    unsafe { oracle_portal_status_value(which) }
}

fn same_outcome(rust: ObsNixPortalOutcome, c: OracleOutcome) -> bool {
    rust.mark_declined == c.mark_declined
        && rust.clear_declined == c.clear_declined
        && rust.set_partly == c.set_partly
        && rust.partly == c.partly
}

#[test]
fn values_and_layout_match_the_c() {
    let rust: [i64; 16] = [
        Action::First as i64,
        Action::Next as i64,
        Action::Unnamed as i64,
        Rebind::None as i64,
        Rebind::KeysDeclined as i64,
        Rebind::DescriptionDeclined as i64,
        Change::Keep as i64,
        Change::Set as i64,
        Change::Clear as i64,
        size_of::<std::ffi::c_int>() as i64,
        size_of::<ObsNixPortalOutcome>() as i64,
        align_of::<ObsNixPortalOutcome>() as i64,
        std::mem::offset_of!(ObsNixPortalOutcome, mark_declined) as i64,
        std::mem::offset_of!(ObsNixPortalOutcome, clear_declined) as i64,
        std::mem::offset_of!(ObsNixPortalOutcome, set_partly) as i64,
        std::mem::offset_of!(ObsNixPortalOutcome, partly) as i64,
    ];
    for (which, name) in VALUE_NAMES.iter().enumerate() {
        assert_eq!(rust[which], c_value(which), "{name}");
    }
    assert_eq!(c_value(VALUE_NAMES.len()), -1);
    assert_eq!(size_of::<OracleOutcome>(), size_of::<ObsNixPortalOutcome>());
}

fn check_count(count: usize) {
    // SAFETY: the oracle functions take and return plain values.
    let (action, more) = unsafe {
        (
            oracle_obs_nix_portal_status_action(count),
            oracle_obs_nix_portal_status_more(count),
        )
    };
    assert_eq!(
        obs_nix_portal_status_action(count),
        action,
        "action({count})"
    );
    assert_eq!(obs_nix_portal_status_more(count), more, "more({count})");
}

#[test]
fn action_counts_match_the_c() {
    counts().for_each(check_count);
}

fn check_description(i: usize, num: usize) {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_nix_portal_status_has_description(i, num) };
    assert_eq!(
        obs_nix_portal_status_has_description(i, num),
        c,
        "has_description({i}, {num})"
    );
}

#[test]
fn descriptions_match_the_c() {
    let values = || (0..=8).chain([usize::MAX - 1, usize::MAX]);
    for i in values() {
        for num in values() {
            check_description(i, num);
        }
    }
}

fn check_rebind(part: usize, declined: usize) {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_nix_portal_status_rebind(part, declined) };
    assert_eq!(
        obs_nix_portal_status_rebind(part, declined),
        c,
        "rebind({part}, {declined})"
    );
}

#[test]
fn rebind_matches_the_c() {
    let values = || (0..=8).chain([usize::MAX - 1, usize::MAX]);
    for part in values() {
        for declined in values() {
            check_rebind(part, declined);
        }
    }
}

#[test]
fn every_outcome_matches_the_c() {
    for bits in 0..8_u8 {
        let (bound, partly, kept) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
        let rust = obs_nix_portal_status_after_bind(bound, partly, kept);
        // SAFETY: plain values; the struct is returned by value.
        let c = unsafe { oracle_obs_nix_portal_status_after_bind(bound, partly, kept) };
        assert!(same_outcome(rust, c), "{bits:03b}: {rust:?} vs {c:?}");
    }
    for bits in 0..4_u8 {
        let (declined, nothing) = (bits & 1 != 0, bits & 2 != 0);
        // SAFETY: plain values.
        let (keep, shown) = unsafe {
            (
                oracle_obs_nix_portal_status_keep_approved(declined, nothing),
                oracle_obs_nix_portal_status_declined(declined, nothing),
            )
        };
        assert_eq!(obs_nix_portal_status_keep_approved(declined, nothing), keep);
        assert_eq!(obs_nix_portal_status_declined(declined, nothing), shown);
    }
}

#[test]
fn every_trigger_rule_matches_the_c() {
    for bits in 0..8_u8 {
        let (desktop, reply, triggered) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
        // SAFETY: plain values.
        let c = unsafe { oracle_obs_nix_portal_status_bound(desktop, reply, triggered) };
        assert_eq!(
            obs_nix_portal_status_bound(desktop, reply, triggered),
            c,
            "bound({desktop}, {reply}, {triggered})"
        );
        // SAFETY: plain values.
        let c = unsafe { oracle_obs_nix_portal_status_desktop_triggers(desktop, reply) };
        assert_eq!(
            obs_nix_portal_status_desktop_triggers(desktop, reply),
            c,
            "desktop_triggers({desktop}, {reply})"
        );
    }
}

fn check_configure(version: u32) {
    // SAFETY: a plain value.
    let can = unsafe { oracle_obs_nix_portal_status_can_configure(version) };
    assert_eq!(
        obs_nix_portal_status_can_configure(version),
        can,
        "can_configure({version})"
    );
    // take_configure()'s gate, as the client builds it on the answer.
    for bits in 0..4_u8 {
        let (pending, session) = (bits & 1 != 0, bits & 2 != 0);
        assert_eq!(
            pending && (session || !obs_nix_portal_status_can_configure(version)),
            pending && (session || !can),
            "take_configure({pending}, {session}, {version})"
        );
    }
}

#[test]
fn every_configure_gate_matches_the_c() {
    for version in (0..=4).chain([u32::MAX - 1, u32::MAX]) {
        check_configure(version);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn random_counts_match_the_c(count in any::<usize>()) {
        check_count(count);
    }

    #[test]
    fn random_positions_match_the_c(i in any::<usize>(), num in any::<usize>()) {
        check_description(i, num);
        check_rebind(i, num);
    }

    #[test]
    fn random_versions_match_the_c(version in any::<u32>()) {
        check_configure(version);
    }
}
