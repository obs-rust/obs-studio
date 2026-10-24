//! How a dialog ends.

use obs_fallback_warning::{End, accepted};

#[test]
fn the_countdown_accepts() {
    assert!(accepted(End::CountdownDone, false));
    assert!(accepted(End::CountdownDone, true));
}

#[test]
fn accept_anyway_accepts() {
    assert!(accepted(End::AcceptClicked, false));
    assert!(accepted(End::AcceptClicked, true));
}

#[test]
fn cancel_stops_a_caller_that_can_stop() {
    assert!(!accepted(End::CancelClicked, true));
}

#[test]
fn closing_the_dialog_stops_a_caller_that_can_stop() {
    assert!(!accepted(End::Closed, true));
}

#[test]
fn cancel_and_close_accept_when_there_is_nothing_to_stop() {
    assert!(accepted(End::CancelClicked, false));
    assert!(accepted(End::Closed, false));
}
