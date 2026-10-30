//! Tier 1: the safe core of `libobs/obs-nix-portal-shortcut-status.c.h`.
//!
//! The first twelve tests port
//! `test/cmocka/test_portal_shortcut_status_steps.c` one to one, with the
//! same names, inputs and expectations, in the same order. `test_values`
//! pins the core's enum values, the C's; Tier 3 compares them with the C.
//!
//! The cases after those pin the edges of each function, and the proptests
//! check each one against a model written from the C.

use obs_hotkey::portal_shortcut_status::{
    self as status, Action, BindResult, Change, MAX_DESCRIBED_ACTIONS, Outcome, Rebind,
};
use proptest::prelude::*;

fn failed(kept: bool) -> Outcome {
    status::after_bind(BindResult::Failed { kept })
}

fn bound(partly: bool) -> Outcome {
    status::after_bind(BindResult::Bound { partly })
}

fn outcome(mark_declined: bool, clear_declined: bool, partly: Option<bool>) -> Outcome {
    Outcome {
        mark_declined,
        clear_declined,
        partly,
    }
}

#[test]
fn test_values() {
    assert_eq!(Action::First as i32, 0);
    assert_eq!(Action::Next as i32, 1);
    assert_eq!(Action::Unnamed as i32, 2);
    assert_eq!(Rebind::None as i32, 0);
    assert_eq!(Rebind::KeysDeclined as i32, 1);
    assert_eq!(Rebind::DescriptionDeclined as i32, 2);
    assert_eq!(Change::Keep as i32, 0);
    assert_eq!(Change::Set as i32, 1);
    assert_eq!(Change::Clear as i32, 2);
}

#[test]
fn test_action_counts() {
    // Characterized, not endorsed: 0 is named after ", ", but the client
    // counts before it asks, so it never asks for 0.
    assert_eq!(status::action(0), Action::Next);
    assert_eq!(status::action(1), Action::First);
    assert_eq!(status::action(2), Action::Next);
    assert_eq!(status::action(3), Action::Next);
    assert_eq!(status::action(4), Action::Unnamed);
    assert_eq!(status::action(5), Action::Unnamed);
    assert_eq!(status::action(6), Action::Unnamed);
    assert_eq!(status::action(usize::MAX), Action::Unnamed);

    assert_eq!(status::more(0), 0);
    assert_eq!(status::more(1), 0);
    assert_eq!(status::more(2), 0);
    assert_eq!(status::more(3), 0);
    assert_eq!(status::more(4), 1);
    assert_eq!(status::more(5), 2);
    assert_eq!(status::more(6), 3);
    assert_eq!(status::more(usize::MAX), usize::MAX - 3);
}

/// The description `collect_binding()` and `collect_bindings()` build for a
/// combination with `count` actions, from the decisions alone.
fn describe(count: usize) -> String {
    let mut out = String::from("OBS:");
    for n in 1..=count {
        match status::action(n) {
            Action::Unnamed => continue,
            Action::First => out.push(' '),
            Action::Next => out.push_str(", "),
        }
        out.push_str(&format!("a{n}"));
    }
    let more = status::more(count);
    if more != 0 {
        out.push_str(&format!(" and {more} more"));
    }
    out
}

#[test]
fn test_descriptions() {
    let expected = [
        "OBS:",
        "OBS: a1",
        "OBS: a1, a2",
        "OBS: a1, a2, a3",
        "OBS: a1, a2, a3 and 1 more",
        "OBS: a1, a2, a3 and 2 more",
        "OBS: a1, a2, a3 and 3 more",
    ];
    for (count, expected) in expected.iter().enumerate() {
        assert_eq!(describe(count), *expected, "{count} actions");
    }
}

#[test]
fn test_has_description() {
    assert!(!status::has_description(0, 0));
    assert!(status::has_description(0, 1));
    assert!(!status::has_description(1, 1));
    assert!(status::has_description(1, 3));
    assert!(!status::has_description(3, 3));
    assert!(!status::has_description(usize::MAX, usize::MAX));
    assert!(status::has_description(usize::MAX - 1, usize::MAX));

    // Three approved combinations with two descriptions:
    // approved_description() stops after the second, so the third is taken
    // as "OBS".
    let compared = (0..)
        .take_while(|&i| status::has_description(i, 3) && status::has_description(i, 2))
        .count();
    assert_eq!(compared, 2);
}

#[test]
fn test_rebind() {
    // Nothing approved before is in the request: nothing is bound again.
    assert_eq!(status::rebind(0, 0), Rebind::None);
    assert_eq!(status::rebind(0, 2), Rebind::None);
    // New keys were declined: the approved part is bound again.
    assert_eq!(status::rebind(1, 1), Rebind::KeysDeclined);
    assert_eq!(status::rebind(3, 5), Rebind::KeysDeclined);
    // No key is new, only a description changed: the approved keys are
    // bound again with the descriptions they were approved with.
    assert_eq!(status::rebind(1, 0), Rebind::DescriptionDeclined);
    assert_eq!(status::rebind(usize::MAX, 0), Rebind::DescriptionDeclined);
    assert_eq!(status::rebind(usize::MAX, usize::MAX), Rebind::KeysDeclined);
}

#[test]
fn test_keep_approved() {
    // Only a request the desktop declined, and that asked for something.
    assert!(status::keep_approved(true, false));
    assert!(!status::keep_approved(true, true));
    assert!(!status::keep_approved(false, false));
    assert!(!status::keep_approved(false, true));
}

#[test]
fn test_after_failed_bind() {
    // `partly` is not read on this path: the C test runs each case with
    // both values, and BindResult::Failed has no `partly`.
    for _partly in [false, true] {
        // bind_approved() bound the approved part again and set the status
        // itself: nothing more changes.
        assert_eq!(failed(true), outcome(false, false, None));
        // Otherwise every current combination becomes declined, and the
        // partly-declined flag is cleared.
        assert_eq!(failed(false), outcome(true, false, Some(false)));
    }
}

#[test]
fn test_declined() {
    // Declined by the desktop: declined.
    assert!(status::declined(true, false));
    // Nothing to ask for (every combination declined before): declined.
    assert!(status::declined(false, true));
    assert!(status::declined(true, true));
    // An error: failed, unless the portal left the bus or the client is
    // stopping (the client reads those two only then).
    assert!(!status::declined(false, false));
}

#[test]
fn test_after_bind() {
    // `kept` is not read on this path: the C test runs both values, and
    // BindResult::Bound has no `kept`.
    // The whole request: nothing is left declined.
    assert_eq!(bound(false), outcome(false, true, Some(false)));
    // Only part of it (keys declined before are left out until Ask again):
    // they stay declined, and the status says so.
    assert_eq!(bound(true), outcome(false, false, Some(true)));
}

#[test]
fn test_trigger_desktop_keys() {
    for reply in [false, true] {
        assert_eq!(status::bound(true, reply, true), Change::Set);
        assert_eq!(status::bound(true, reply, false), Change::Clear);
        assert_eq!(status::desktop_triggers(true, reply), Change::Set);
    }
}

#[test]
fn test_trigger_no_desktop_keys() {
    assert_eq!(status::bound(false, true, false), Change::Set);
    assert_eq!(status::bound(false, false, false), Change::Keep);
    assert_eq!(status::desktop_triggers(false, true), Change::Clear);
    assert_eq!(status::desktop_triggers(false, false), Change::Keep);

    // Characterized, not endorsed: a trigger on a desktop that does not
    // pick the keys is ignored. The client never asks: any trigger makes it
    // a desktop that picks the keys.
    assert_eq!(status::bound(false, true, true), Change::Set);
    assert_eq!(status::bound(false, false, true), Change::Keep);
}

#[test]
fn test_configure() {
    for version in [0, 1, 2, 3, u32::MAX] {
        let editor = version >= 2;
        // take_configure() takes a pending request with a session, or
        // without one when the portal has no editor (then
        // configure_shortcuts() refuses it); otherwise it waits for the
        // bind a probe is about to make.
        assert_eq!(status::can_configure(version), editor);
    }
}

// ---------------------------------------------------------------------
// Edges

#[test]
fn described_actions_is_three() {
    assert_eq!(MAX_DESCRIBED_ACTIONS, 3);
    assert_eq!(status::action(MAX_DESCRIBED_ACTIONS), Action::Next);
    assert_eq!(status::action(MAX_DESCRIBED_ACTIONS + 1), Action::Unnamed);
    assert_eq!(status::more(MAX_DESCRIBED_ACTIONS), 0);
    assert_eq!(status::more(MAX_DESCRIBED_ACTIONS + 1), 1);
}

#[test]
fn counted_descriptions_up_to_64() {
    // Every count names min(count, 3) actions and counts the rest.
    for count in 0..=64 {
        let description = describe(count);
        let named = description.matches(", ").count() + usize::from(count > 0);
        assert_eq!(named, count.min(MAX_DESCRIBED_ACTIONS), "{description}");
        let more = count.saturating_sub(MAX_DESCRIBED_ACTIONS);
        assert_eq!(
            description.ends_with(&format!(" and {more} more")),
            more > 0,
            "{description}"
        );
    }
}

#[test]
fn every_failed_bind() {
    // Only `kept` keeps the declined set and the partly-declined flag.
    for kept in [false, true] {
        let o = failed(kept);
        assert!(!o.clear_declined);
        assert_eq!(o.mark_declined, !kept);
        assert_eq!(o.partly, (!kept).then_some(false));
    }
}

#[test]
fn bound_never_marks_declined() {
    for partly in [false, true] {
        let o = bound(partly);
        assert!(!o.mark_declined);
        assert_eq!(o.clear_declined, !partly);
    }
}

#[test]
fn configure_version_edges() {
    assert!(!status::can_configure(0));
    assert!(!status::can_configure(status::CONFIGURE_VERSION - 1));
    assert!(status::can_configure(status::CONFIGURE_VERSION));
    assert!(status::can_configure(u32::MAX));
}

// ---------------------------------------------------------------------
// Models written from the C

fn model_after_bind(bound: bool, partly: bool, kept: bool) -> Outcome {
    if bound {
        return outcome(false, !partly, Some(partly));
    }
    if kept {
        return outcome(false, false, None);
    }
    outcome(true, false, Some(false))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn action_and_more_match_model(count in any::<usize>()) {
        let expected = if count > 3 {
            Action::Unnamed
        } else if count == 1 {
            Action::First
        } else {
            Action::Next
        };
        prop_assert_eq!(status::action(count), expected);
        prop_assert_eq!(status::more(count), if count > 3 { count.wrapping_sub(3) } else { 0 });
    }

    #[test]
    fn has_description_matches_model(i in any::<usize>(), num in any::<usize>()) {
        prop_assert_eq!(status::has_description(i, num), i < num);
    }

    #[test]
    fn rebind_matches_model(part in any::<usize>(), declined in any::<usize>()) {
        let expected = match (part > 0, declined > 0) {
            (false, _) => Rebind::None,
            (true, true) => Rebind::KeysDeclined,
            (true, false) => Rebind::DescriptionDeclined,
        };
        prop_assert_eq!(status::rebind(part, declined), expected);
    }

    #[test]
    fn after_bind_matches_model(bits in 0..8_u8) {
        let b = |n: u8| bits & (1 << n) != 0;
        let result = if b(0) {
            BindResult::Bound { partly: b(1) }
        } else {
            BindResult::Failed { kept: b(2) }
        };
        prop_assert_eq!(status::after_bind(result), model_after_bind(b(0), b(1), b(2)));
    }

    #[test]
    fn declined_matches_model(declined in any::<bool>(), nothing in any::<bool>()) {
        prop_assert_eq!(status::declined(declined, nothing), declined || nothing);
    }

    #[test]
    fn triggers_match_model(desktop in any::<bool>(), reply in any::<bool>(), triggered in any::<bool>()) {
        let expected = if reply || desktop {
            if !desktop || triggered { Change::Set } else { Change::Clear }
        } else {
            Change::Keep
        };
        prop_assert_eq!(status::bound(desktop, reply, triggered), expected);
        let flag = if desktop { Change::Set } else if reply { Change::Clear } else { Change::Keep };
        prop_assert_eq!(status::desktop_triggers(desktop, reply), flag);
    }

    #[test]
    fn configure_matches_model(version in any::<u32>()) {
        prop_assert_eq!(status::can_configure(version), version >= 2);
    }
}
