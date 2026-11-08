use obs_hotkey::background_app_id::{BackgroundAppId, query};
use std::cell::RefCell;

struct Background {
    initialized: bool,
    id: Option<&'static str>,
    calls: RefCell<Vec<&'static str>>,
}
impl BackgroundAppId for Background {
    type Id = Option<&'static str>;
    fn lock(&mut self) -> bool {
        self.calls.borrow_mut().push("lock");
        self.initialized
    }
    fn app_id(&self) -> Self::Id {
        self.calls.borrow_mut().push("read");
        self.id
    }
    fn unlock(&mut self) {
        self.calls.borrow_mut().push("unlock");
    }
}

#[test]
fn mirrors_c_background_app_id() {
    for initialized in [false, true] {
        for id in [None, Some(""), Some("app-id sentinel"), Some("App-id sentinel")] {
            let mut background = Background { initialized, id, calls: RefCell::default() };
            for _ in 0..2 {
                background.calls.borrow_mut().clear();
                assert_eq!(query(&mut background), initialized.then_some(id));
                assert_eq!(*background.calls.borrow(), if initialized { vec!["lock", "read", "unlock"] } else { vec!["lock"] });
                assert_eq!(background.id, id);
            }
        }
    }
}
