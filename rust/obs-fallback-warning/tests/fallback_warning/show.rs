//! Whether a warning shows a dialog.

use obs_fallback_warning::{Session, Show};

#[test]
fn first_warning_with_a_visible_window_shows_the_dialog() {
    let mut session = Session::new();
    assert_eq!(session.begin("browser-sandbox", true), Show::Dialog);
}

#[test]
fn the_same_warning_shows_the_dialog_once_per_session() {
    let mut session = Session::new();
    assert_eq!(session.begin("browser-sandbox", true), Show::Dialog);
    assert_eq!(session.begin("browser-sandbox", true), Show::AcceptNow);
    assert_eq!(session.begin("browser-sandbox", true), Show::AcceptNow);
}

#[test]
fn another_warning_still_shows_its_dialog() {
    let mut session = Session::new();
    assert_eq!(session.begin("browser-sandbox", true), Show::Dialog);
    assert_eq!(session.begin("rtmps-fallback", true), Show::Dialog);
}

#[test]
fn no_visible_window_accepts_now_and_is_not_remembered() {
    let mut session = Session::new();
    assert_eq!(session.begin("browser-sandbox", false), Show::AcceptNow);
    // Nobody saw it, so the dialog shows when there is a window.
    assert_eq!(session.begin("browser-sandbox", true), Show::Dialog);
}

#[test]
fn an_empty_id_is_never_remembered() {
    let mut session = Session::new();
    assert_eq!(session.begin("", true), Show::Dialog);
    assert_eq!(session.begin("", true), Show::Dialog);
}

#[test]
fn sessions_do_not_share_what_they_showed() {
    let mut first = Session::new();
    let mut second = Session::new();
    assert_eq!(first.begin("browser-sandbox", true), Show::Dialog);
    assert_eq!(second.begin("browser-sandbox", true), Show::Dialog);
}
