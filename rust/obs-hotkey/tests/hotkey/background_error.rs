//! Mirror the unchanged portal getter cases and pin mutex/copy ordering.
use obs_hotkey::background_error::{BackgroundError, query};

struct Backend {
    available: bool,
    error: Option<&'static str>,
    held: bool,
    events: Vec<&'static str>,
}

impl BackgroundError for Backend {
    type Error = &'static str;
    type Owned = String;
    fn lock(&mut self) -> bool {
        assert!(!self.held);
        self.events.push("lock");
        self.held = self.available;
        self.available
    }
    fn error(&self) -> Option<Self::Error> {
        assert!(self.held);
        self.error
    }
    fn duplicate(&mut self, error: Self::Error) -> Self::Owned {
        assert!(self.held);
        self.events.push("duplicate");
        error.to_owned()
    }
    fn unlock(&mut self) {
        assert!(self.held);
        self.events.push("unlock");
        self.held = false;
    }
}

#[test]
fn native_without_portal_client() {
    for available in [false, true] {
        let mut backend = Backend { available, error: None, held: false, events: vec![] };
        assert_eq!(query(&mut backend), None);
        assert!(!backend.held);
        assert_eq!(backend.events, if available { vec!["lock", "unlock"] } else { vec!["lock"] });
    }
}

#[test]
fn status_without_portal_and_status_reports_portal_error() {
    for error in ["", "No portal available", "An app id is required"] {
        let mut backend = Backend { available: true, error: Some(error), held: false, events: vec![] };
        let owned = query(&mut backend).expect("portal error copy");
        assert_eq!(owned, error);
        assert!(!backend.held);
        assert_eq!(backend.events, ["lock", "duplicate", "unlock"]);
        backend.error = None;
        assert_eq!(owned, error);
    }
}

#[test]
fn failed_lock_never_reads_or_copies_error() {
    let mut backend = Backend { available: false, error: Some("unreachable"), held: false, events: vec![] };
    assert_eq!(query(&mut backend), None);
    assert_eq!(backend.events, ["lock"]);
}
