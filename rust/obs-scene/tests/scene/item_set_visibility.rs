//! Tier 1 mirrors the reconciliation assertions in test_scene_item_set_visibility.c.
//! Source creation, public setter return values and detachment remain Tier 2 checks.
use obs_scene::item_set_visibility::{VisibilityItem, set_visibility};

#[derive(Debug)]
struct Item {
    refs: i64,
    visible: bool,
    user_visible: bool,
    queued: usize,
    child: bool,
    locked: bool,
    trace: Vec<&'static str>,
}

impl Item {
    fn new(refs: i64, visible: bool, user_visible: bool, queued: usize) -> Self {
        Self {
            refs,
            visible,
            user_visible,
            queued,
            child: refs > 0,
            locked: false,
            trace: Vec::new(),
        }
    }

    fn record(&mut self, event: &'static str) {
        assert!(self.locked, "{event} must run under actions_mutex");
        self.trace.push(event);
    }

    fn check(&self, visible: bool) {
        assert_eq!(self.user_visible, visible); // obs_sceneitem_visible
        assert_eq!(self.visible, visible);
        assert_eq!(self.user_visible, visible);
        assert_eq!(self.refs, i64::from(visible));
        assert_eq!(self.queued, 0);
        assert_eq!(self.child, visible); // active source and active-child enumeration
        assert!(!self.locked);
    }
}

impl VisibilityItem for Item {
    fn lock(&mut self) {
        assert!(!self.locked);
        self.locked = true;
        self.trace.push("lock");
    }
    fn unlock(&mut self) {
        self.record("unlock");
        self.locked = false;
    }
    fn clear_actions(&mut self) {
        self.record("clear");
        self.queued = 0;
    }
    fn active_refs(&mut self) -> i128 {
        self.record("refs");
        i128::from(self.refs)
    }
    fn add_active_child(&mut self) {
        self.record("add");
        self.child = true;
    }
    fn remove_active_child(&mut self) {
        self.record("remove");
        self.child = false;
    }
    fn set_active_refs(&mut self, visible: bool) {
        self.record("set_refs");
        self.refs = i64::from(visible);
    }
    fn set_visible(&mut self, visible: bool) {
        self.record("visible");
        self.visible = visible;
    }
    fn set_user_visible(&mut self, visible: bool) {
        self.record("user_visible");
        self.user_visible = visible;
    }
}

#[test]
fn test_scene_item_set_visibility_immediate_video() {
    let mut item = Item::new(1, true, true, 0);
    item.check(true);
    for _ in 0..3 {
        set_visibility(&mut item, false);
        item.check(false);
        set_visibility(&mut item, true);
        item.check(true);
    }
}

#[test]
fn test_scene_item_set_visibility_remove_pending_audio() {
    // Public audio toggles have queued four changes, retaining two refs and one child.
    let mut item = Item::new(2, false, false, 4);
    assert_eq!(item.refs, 2);
    assert_eq!(item.queued, 4);
    assert!(item.child);
    set_visibility(&mut item, false);
    item.check(false);
    assert_eq!(
        item.trace,
        [
            "lock",
            "clear",
            "refs",
            "remove",
            "set_refs",
            "visible",
            "user_visible",
            "unlock"
        ]
    );
}

#[test]
fn test_scene_item_set_visibility_loaded_and_duplicated_audio() {
    for visible in [false, true] {
        let mut loaded = Item::new(1, false, true, 1);
        set_visibility(&mut loaded, visible);
        loaded.check(visible);
        if !visible {
            let mut copied = Item::new(1, false, true, 1);
            set_visibility(&mut copied, false);
            copied.check(false);
        }
    }
}

#[test]
fn normalization_and_call_order_for_all_prior_reference_states() {
    for refs in [i64::MIN, -1, 0, 1, 2, i64::MAX] {
        for visible in [false, true] {
            let mut item = Item::new(refs, !visible, !visible, 19);
            set_visibility(&mut item, visible);
            item.check(visible);
            let mut expected = vec!["lock", "clear", "refs"];
            if refs > 0 && !visible {
                expected.push("remove");
            }
            if refs <= 0 && visible {
                expected.push("add");
            }
            expected.extend(["set_refs", "visible", "user_visible", "unlock"]);
            assert_eq!(item.trace, expected);
        }
    }
}
