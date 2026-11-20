//! Tier 1: one-to-one counterparts of test_core_private_data.c, using safe Data.
use obs_core::private_data::{self, Host};
use obs_data::data::{Data, Item};

pub(super) struct Core {
    pub(super) objects: Vec<Option<Data>>,
    pub(super) held: Vec<(usize, Data)>,
    pub(super) calls: Vec<&'static str>,
}

impl Core {
    pub(super) fn new() -> Self {
        Self {
            objects: vec![Some(Data::new()), Some(Data::new()), Some(Data::new())],
            held: Vec::new(),
            calls: Vec::new(),
        }
    }
    pub(super) fn data(&self, id: usize) -> &Data {
        self.objects[id]
            .as_ref()
            .unwrap_or_else(|| &self.held.iter().find(|(index, _)| *index == id).unwrap().1)
    }
    pub(super) fn release(&mut self, id: usize) {
        let index = self
            .held
            .iter()
            .rposition(|(index, _)| *index == id)
            .unwrap();
        self.held.remove(index);
    }
}

impl Host for Core {
    type Data = usize;
    fn private_data(&mut self) -> usize {
        self.calls.push("read");
        0
    }
    fn clear(&mut self, data: usize) {
        self.calls.push("clear");
        self.data(data).clear();
    }
    fn apply(&mut self, data: usize, settings: usize) {
        self.calls.push("apply");
        self.data(data).apply(self.data(settings));
    }
    fn addref(&mut self, data: usize) {
        self.calls.push("retain");
        self.held.push((data, self.data(data).clone()));
    }
}

#[test]
fn replacement_keeps_storage_and_copies_settings() {
    let mut core = Core::new();
    let held = private_data::get(&mut core);
    core.data(held).set_string("old-only", Some("discard"));
    core.data(held).set_string("shared", Some("old"));
    core.data(1).set_int("shared", 42);
    core.data(1).set_bool("new-only", true);
    private_data::set(&mut core, Some(1));
    let read = private_data::get(&mut core);
    assert_eq!(read, held);
    assert_ne!(read, 1);
    assert!(!core.data(read).has_user_value("old-only"));
    assert_eq!(core.data(read).get_string("old-only"), "");
    assert_eq!(core.data(read).get_int("shared"), 42);
    assert!(core.data(read).get_bool("new-only"));
    assert_eq!(core.data(1).get_int("shared"), 42);
    assert!(core.data(1).get_bool("new-only"));
    assert!(!core.data(1).has_user_value("old-only"));
    core.data(1).set_int("shared", 99);
    assert_eq!(core.data(held).get_int("shared"), 42);
    core.objects[1] = None;
    assert!(core.data(held).get_bool("new-only"));
    core.data(read)
        .set_string("through-read", Some("shared storage"));
    assert_eq!(core.data(held).get_string("through-read"), "shared storage");
    core.release(read);
    core.release(held);
}

#[test]
fn null_and_empty_settings_clear_user_values() {
    let mut core = Core::new();
    let held = private_data::get(&mut core);
    core.data(held)
        .set_default_string("fallback", Some("default"));
    core.data(held).set_string("fallback", Some("user"));
    core.data(held).set_int("old-only", 7);
    private_data::set(&mut core, None);
    assert!(!core.data(held).has_user_value("fallback"));
    assert!(!core.data(held).has_user_value("old-only"));
    assert!(core.data(held).has_default_value("fallback"));
    assert_eq!(core.data(held).get_string("fallback"), "default");
    core.data(held).set_int("again", 13);
    private_data::set(&mut core, Some(2));
    assert!(!core.data(held).has_user_value("again"));
    assert_eq!(core.data(held).get_string("fallback"), "default");
    core.objects[2] = None;
    private_data::set(&mut core, None);
    let read = private_data::get(&mut core);
    assert_eq!(read, held);
    assert!(!core.data(read).has_user_value("again"));
    core.release(read);
    core.release(held);
}

#[test]
fn self_assignment_clears_before_applying() {
    let mut core = Core::new();
    let held = private_data::get(&mut core);
    core.data(held)
        .set_string("value", Some("cleared before apply"));
    private_data::set(&mut core, Some(held));
    assert!(!core.data(held).has_user_value("value"));
    assert_eq!(core.data(held).get_string("value"), "");
    let read = private_data::get(&mut core);
    assert_eq!(read, held);
    core.release(read);
    core.release(held);
}

fn prepare_probe(data: &Data) -> [Item; 4] {
    data.set_int("first", 1);
    data.set_int("tail", 2);
    [
        data.item("first").unwrap(),
        data.item("first").unwrap(),
        data.item("tail").unwrap(),
        data.item("tail").unwrap(),
    ]
}

fn observe_destroyed_parent([first_before, first_after, tail, _guard]: [Item; 4]) -> u8 {
    let before = u8::from(first_before.next().is_some());
    tail.remove();
    let after = u8::from(first_after.next().is_some()) * 2;
    before | after
}

#[test]
fn every_getter_retains_exactly_once() {
    let control = Data::new();
    let control_probe = prepare_probe(&control);
    drop(control);
    let destroyed = observe_destroyed_parent(control_probe);
    assert_ne!(destroyed, 1);
    let mut core = Core::new();
    let held: Vec<_> = (0..8).map(|_| private_data::get(&mut core)).collect();
    assert_eq!(core.held.len(), 8);
    for &id in &held {
        assert_eq!(id, 0);
        assert_eq!(id, held[0]);
    }
    let probe = prepare_probe(core.data(held[0]));
    core.objects[0] = None;
    for id in held {
        assert_eq!(core.data(id).get_int("first"), 1);
        assert_eq!(core.data(id).get_int("tail"), 2);
        core.release(id);
    }
    assert_eq!(observe_destroyed_parent(probe), destroyed);
}

#[test]
fn dependency_calls_follow_c_order() {
    let mut core = Core::new();
    private_data::set(&mut core, None);
    assert_eq!(core.calls, ["read", "clear"]);
    core.calls.clear();
    private_data::set(&mut core, Some(0));
    assert_eq!(core.calls, ["read", "clear", "read", "apply"]);
    core.calls.clear();
    assert_eq!(private_data::get(&mut core), 0);
    assert_eq!(core.calls, ["read", "retain"]);
}
