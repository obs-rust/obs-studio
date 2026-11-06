//! Whether a warning shows a dialog, and what an answer does (#3724).

use obs_camera_busy::{
    Action, Allowlist, Answer, Begin, Cause, Holder, RATE_LIMIT_MS, Show, Warnings,
};

const DEV: &str = "/dev/video0";

fn h(pid: u32, name: &str) -> Holder {
    Holder {
        pid,
        name: name.into(),
        can_close: true,
    }
}

fn dialog(holders: Vec<Holder>) -> Begin {
    Begin {
        show: Show::Dialog,
        holders,
    }
}

fn not_shown(show: Show) -> Begin {
    Begin {
        show,
        holders: vec![],
    }
}

#[test]
fn first_warning_shows_a_dialog() {
    let mut w = Warnings::default();
    assert_eq!(
        w.begin(Cause::Busy, DEV, &[h(12, "zoom")], 0),
        dialog(vec![h(12, "zoom")])
    );
}

#[test]
fn no_duplicate_while_the_dialog_is_open() {
    let mut w = Warnings::default();
    w.begin(Cause::Busy, DEV, &[h(12, "zoom")], 0);
    assert_eq!(
        w.begin(Cause::Opened, DEV, &[h(12, "zoom")], 5),
        not_shown(Show::Duplicate)
    );
    // A new PID of the same app is the same holder.
    assert_eq!(
        w.begin(Cause::Busy, DEV, &[h(13, "zoom")], RATE_LIMIT_MS * 10),
        not_shown(Show::Duplicate)
    );
    // Another device, or another app, is not a duplicate.
    assert_eq!(
        w.begin(Cause::Busy, "/dev/video2", &[h(12, "zoom")], 5)
            .show,
        Show::Dialog
    );
    assert_eq!(
        w.begin(Cause::Busy, DEV, &[h(12, "zoom"), h(40, "cheese")], 5),
        dialog(vec![h(40, "cheese")])
    );
}

#[test]
fn rate_limited_after_an_answer() {
    let mut w = Warnings::default();
    let b = w.begin(Cause::Busy, DEV, &[h(12, "zoom")], 1_000);
    assert_eq!(
        w.end(DEV, &b.holders, Answer::Ignore, 2_000),
        Action::default()
    );
    assert_eq!(
        w.begin(
            Cause::Busy,
            DEV,
            &[h(12, "zoom")],
            2_000 + RATE_LIMIT_MS - 1
        ),
        not_shown(Show::RateLimited)
    );
    assert_eq!(
        w.begin(Cause::Busy, DEV, &[h(12, "zoom")], 2_000 + RATE_LIMIT_MS)
            .show,
        Show::Dialog
    );
}

#[test]
fn a_clock_going_back_does_not_unblock() {
    let mut w = Warnings::default();
    let b = w.begin(Cause::Busy, DEV, &[h(12, "zoom")], 5_000);
    w.end(DEV, &b.holders, Answer::Ignore, 5_000);
    assert_eq!(
        w.begin(Cause::Busy, DEV, &[h(12, "zoom")], 0).show,
        Show::RateLimited
    );
}

#[test]
fn allowlisted_apps_never_warn() {
    let mut w = Warnings::new(Allowlist::parse("zoom"));
    assert_eq!(
        w.begin(Cause::Busy, DEV, &[h(12, "zoom")], 0),
        not_shown(Show::Allowed)
    );
    assert_eq!(
        w.begin(Cause::Opened, DEV, &[h(12, "zoom"), h(40, "cheese")], 0),
        dialog(vec![h(40, "cheese")])
    );
}

#[test]
fn always_allow_adds_the_named_apps() {
    let mut w = Warnings::default();
    let b = w.begin(Cause::Busy, DEV, &[h(12, "zoom"), h(40, "cheese")], 0);
    let action = w.end(DEV, &b.holders, Answer::AlwaysAllow, 1);
    assert_eq!(
        action,
        Action {
            close: vec![],
            allowlist_changed: true
        }
    );
    assert_eq!(w.allowlist().to_config(), "cheese;zoom");
    assert_eq!(
        w.begin(Cause::Busy, DEV, &[h(12, "zoom")], RATE_LIMIT_MS * 2)
            .show,
        Show::Allowed
    );
    // Allowing again changes nothing.
    let b = w.begin(Cause::Busy, "/dev/video2", &[], 0);
    assert!(
        !w.end("/dev/video2", &b.holders, Answer::AlwaysAllow, 1)
            .allowlist_changed
    );
}

#[test]
fn close_it_closes_only_what_this_user_owns() {
    let mut w = Warnings::default();
    let other = Holder {
        pid: 50,
        name: "root-app".into(),
        can_close: false,
    };
    let b = w.begin(Cause::Busy, DEV, &[h(12, "zoom"), other], 0);
    assert_eq!(
        w.end(DEV, &b.holders, Answer::CloseIt, 1),
        Action {
            close: vec![12],
            allowlist_changed: false
        }
    );
}

#[test]
fn an_answer_reopens_the_slot() {
    let mut w = Warnings::default();
    let b = w.begin(Cause::Busy, DEV, &[h(12, "zoom")], 0);
    w.end(DEV, &b.holders, Answer::CloseIt, 1);
    // Rate limited, not a duplicate: the dialog is closed.
    assert_eq!(
        w.begin(Cause::Busy, DEV, &[h(12, "zoom")], 2).show,
        Show::RateLimited
    );
}

#[test]
fn busy_with_an_unknown_holder_still_warns_once() {
    let mut w = Warnings::default();
    assert_eq!(w.begin(Cause::Busy, DEV, &[], 0), dialog(vec![]));
    assert_eq!(w.begin(Cause::Busy, DEV, &[], 1).show, Show::Duplicate);
    w.end(DEV, &[], Answer::Ignore, 2);
    assert_eq!(w.begin(Cause::Busy, DEV, &[], 3).show, Show::RateLimited);
}

#[test]
fn an_open_nobody_holds_any_more_is_nothing() {
    let mut w = Warnings::default();
    assert_eq!(
        w.begin(Cause::Opened, DEV, &[], 0),
        not_shown(Show::NoHolder)
    );
    // And it blocks nothing.
    assert_eq!(w.begin(Cause::Busy, DEV, &[], 0).show, Show::Dialog);
}

#[test]
fn set_allowlist_replaces_it() {
    let mut w = Warnings::new(Allowlist::parse("zoom"));
    w.set_allowlist(Allowlist::parse("cheese"));
    assert_eq!(w.allowlist().to_config(), "cheese");
    assert_eq!(
        w.begin(Cause::Busy, DEV, &[h(12, "zoom")], 0).show,
        Show::Dialog
    );
}

/// A warning that could not name the holder stays open while the source keeps
/// retrying; a later report that names it replaces it (#3726).
#[test]
fn unknown_warning_is_open_until_answered() {
    let mut w = Warnings::default();
    assert!(!w.unknown_open(DEV));
    assert_eq!(w.begin(Cause::Busy, DEV, &[], 0), dialog(vec![]));
    assert!(w.unknown_open(DEV));
    assert!(!w.unknown_open("/dev/video1"));

    // The holder is found on a retry: a named warning shows beside it.
    assert_eq!(
        w.begin(Cause::Busy, DEV, &[h(12, "zoom")], 1),
        dialog(vec![h(12, "zoom")])
    );
    assert!(w.unknown_open(DEV));

    w.end(DEV, &[], Answer::Ignore, 2);
    assert!(!w.unknown_open(DEV));
}
