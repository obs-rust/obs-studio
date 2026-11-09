use obs_scene::set_visibility::{Item, reconcile};

#[derive(Debug)]
struct State {
    refs: i64,
    visible: bool,
    user_visible: bool,
    queued: usize,
    children: usize,
    events: Vec<&'static str>,
}
impl Item for State {
    fn clear_actions(&mut self) {
        self.events.push("clear");
        self.queued = 0;
    }
    fn active_refs(&self) -> i64 {
        self.refs
    }
    fn add_child(&mut self) {
        self.events.push("add");
        self.children += 1;
    }
    fn remove_child(&mut self) {
        self.events.push("remove");
        self.children -= 1;
    }
    fn normalize(&mut self, visible: bool) {
        self.events.push("normalize");
        self.refs = i64::from(visible);
        self.visible = visible;
        self.user_visible = visible;
    }
}
fn state(refs: i64, queued: usize) -> State {
    State {
        refs,
        visible: true,
        user_visible: false,
        queued,
        children: usize::from(refs > 0),
        events: vec![],
    }
}
fn check(s: &State, visible: bool) {
    assert_eq!(s.visible, visible);
    assert_eq!(s.user_visible, visible);
    assert_eq!(s.refs, i64::from(visible));
    assert_eq!(s.queued, 0);
    assert_eq!(s.children, usize::from(visible));
}
#[test]
fn mirrors_immediate_video() {
    let mut s = state(1, 0);
    for _ in 0..3 {
        reconcile(&mut s, false);
        check(&s, false);
        reconcile(&mut s, false);
        check(&s, false);
        reconcile(&mut s, true);
        check(&s, true);
        reconcile(&mut s, true);
        check(&s, true);
    }
}
#[test]
fn mirrors_remove_pending_audio() {
    let mut s = state(2, 4);
    reconcile(&mut s, false);
    check(&s, false);
    assert_eq!(s.events, ["clear", "remove", "normalize"]);
}
#[test]
fn mirrors_loaded_and_duplicated_audio() {
    for visible in [false, true] {
        let mut s = state(1, 1);
        reconcile(&mut s, visible);
        check(&s, visible);
        if !visible {
            let mut copy = state(1, 1);
            reconcile(&mut copy, false);
            check(&copy, false);
        }
    }
}
#[test]
fn prior_refs_decide_child_change_before_normalization() {
    for refs in [i64::MIN, -1, 0, 1, 2, i64::MAX] {
        for visible in [false, true] {
            let mut s = state(refs, 7);
            reconcile(&mut s, visible);
            check(&s, visible);
            let expected = match (refs > 0, visible) {
                (true, false) => vec!["clear", "remove", "normalize"],
                (false, true) => vec!["clear", "add", "normalize"],
                _ => vec!["clear", "normalize"],
            };
            assert_eq!(s.events, expected);
        }
    }
}
