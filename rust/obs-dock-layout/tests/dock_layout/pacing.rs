//! The session around the model (#5475): the blob the window started with,
//! and when the adapter takes a snapshot. These were DockLayout's own state
//! (`restored`, `started`, `queued`, `observeQueued`, `dragging`).

use obs_dock_layout::{Cause, Session};

use crate::dock_layout::fixture::{MIN_W, bottom, full_record};

/// The full-size blob restored into a 470 px tile: every dock clamped.
fn clamped() -> obs_dock_layout::Snapshot {
    bottom(
        470,
        900,
        200,
        &[("scenes", MIN_W), ("sources", MIN_W), ("mixer", MIN_W)],
    )
}

fn started() -> Session {
    let mut session = Session::new();
    session.start(None, b"blob", "h1");
    session
}

#[test]
fn nothing_is_taken_before_the_first_start() {
    let mut session = Session::new();
    assert!(session.queue(Cause::Change));
    assert_eq!(session.turn(true), None);
    // The cause queued before the start is dropped, not kept for later.
    session.start(None, b"blob", "h1");
    assert!(session.queue(Cause::Change));
    assert_eq!(session.turn(true), Some(Cause::Change));
}

#[test]
fn one_turn_is_posted_until_it_runs() {
    let mut session = started();
    assert!(session.queue(Cause::Change));
    assert!(!session.queue(Cause::Change));
    assert!(!session.queue(Cause::Restored));
    assert_eq!(session.turn(true), Some(Cause::Restored));
    // Nothing queued: the turn takes nothing, and the next change posts.
    assert_eq!(session.turn(true), None);
    assert!(session.queue(Cause::Change));
}

#[test]
fn the_strongest_cause_wins() {
    let mut session = started();
    session.queue(Cause::Restored);
    session.queue(Cause::Change);
    session.queue(Cause::SeparatorDrag);
    assert_eq!(session.turn(true), Some(Cause::Restored));
    session.queue(Cause::Change);
    session.queue(Cause::SeparatorDrag);
    session.queue(Cause::Change);
    assert_eq!(session.turn(true), Some(Cause::SeparatorDrag));
}

#[test]
fn no_snapshot_during_a_drag_and_the_release_gives_the_drag() {
    let mut session = started();
    session.press();
    assert!(session.queue(Cause::Change));
    assert_eq!(session.turn(true), None);
    // The cause stays; the turn ran, so the release posts again.
    assert!(session.release());
    assert_eq!(session.turn(true), Some(Cause::SeparatorDrag));
}

#[test]
fn a_restore_during_a_drag_stays_a_restore() {
    let mut session = started();
    session.press();
    session.queue(Cause::Restored);
    assert_eq!(session.turn(true), None);
    session.release();
    assert_eq!(session.turn(true), Some(Cause::Restored));
}

#[test]
fn a_release_without_a_press_queues_nothing() {
    let mut session = started();
    assert!(!session.release());
    assert_eq!(session.turn(true), None);
    session.press();
    assert!(session.release());
    assert!(!session.release());
    assert_eq!(session.turn(true), Some(Cause::SeparatorDrag));
}

#[test]
fn a_hidden_window_keeps_only_a_restore() {
    let mut session = started();
    session.queue(Cause::Change);
    assert_eq!(session.turn(false), None);
    assert!(session.queue(Cause::Change));
    assert_eq!(session.turn(true), Some(Cause::Change));

    session.queue(Cause::Restored);
    assert_eq!(session.turn(false), None);
    // Kept: the next turn after the window shows takes it.
    assert!(session.queue(Cause::Change));
    assert_eq!(session.turn(true), Some(Cause::Restored));
}

#[test]
fn a_start_keeps_the_queue_and_the_drag() {
    let mut session = started();
    session.press();
    assert!(session.queue(Cause::Change));
    session.start(None, b"other", "h2");
    // Still posted and still dragging, as before the start.
    assert!(!session.queue(Cause::Restored));
    assert_eq!(session.turn(true), None);
    session.release();
    assert_eq!(session.turn(true), Some(Cause::Restored));
}

#[test]
fn a_save_writes_the_started_blob_while_its_sizes_are_unknown() {
    let mut session = started();
    session.observe(&clamped(), Cause::Restored);
    assert!(session.model().saves_restored_blob());
    assert_eq!(session.blob_to_save(), Some(&b"blob"[..]));
    // The user changes something: the live layout is saved from now on.
    let mut hidden = clamped();
    hidden.docks[2].shown = false;
    session.observe(&hidden, Cause::Change);
    assert_eq!(session.blob_to_save(), None);
}

#[test]
fn a_blob_with_its_sizes_is_never_saved_instead() {
    let mut session = Session::new();
    assert!(session.start(Some(&full_record()), b"blob", "h1"));
    assert_eq!(session.blob_to_save(), None);
    assert_eq!(session.restored_blob(), b"blob");
}

#[test]
fn an_empty_blob_is_never_saved() {
    let mut session = Session::new();
    session.start(None, b"", "h1");
    session.observe(&clamped(), Cause::Restored);
    assert!(session.model().saves_restored_blob());
    assert_eq!(session.blob_to_save(), None);
}

#[test]
fn restore_again_restores_the_started_blob() {
    let mut session = started();
    let out = session.observe(&clamped(), Cause::Restored);
    assert!(!out.restore_again);
    let grown = bottom(
        800,
        900,
        200,
        &[("scenes", 262), ("sources", 262), ("mixer", 264)],
    );
    assert!(session.observe(&grown, Cause::Change).restore_again);
    assert_eq!(session.restored_blob(), b"blob");
    // A new start replaces it.
    session.start(None, b"reset", "h2");
    assert_eq!(session.restored_blob(), b"reset");
}
