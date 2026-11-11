//! Tier 1: every assertion in unchanged test_module_post_load.c, over the
//! existing module-list model. No FFI or private C representation.
use super::{Model, ModelModule, Read};
use obs_core::module_post_load::{PostLoad, post_load};

impl PostLoad for Model {
    type Callback = u8;
    fn host(&mut self) {
        assert!(self.post_load_visits.is_empty() || self.host_calls > 0);
        self.host_calls += 1;
        self.reads.borrow_mut().clear();
    }
    fn callback(&self, module: usize) -> Option<u8> {
        assert!(self.host_calls > 0, "host must precede traversal");
        self.modules[module].post_load
    }
    fn call(&mut self, callback: u8) {
        match callback {
            1 => self.modules[0].loaded = false,
            2 => assert!(!self.modules[0].loaded),
            3 => (),
            4 => panic!("disabled/detached module callback"),
            5 => self.modules[0].next = None,
            6 => self.modules[1].post_load = Some(3),
            _ => unreachable!(),
        }
        self.post_load_visits.push(callback);
    }
}

fn fixture() -> Model {
    Model {
        modules: vec![ModelModule::default(); 6],
        ..Model::started()
    }
}

#[test]
fn test_module_post_load_empty_and_excluded() {
    let mut model = fixture();
    model.modules[0].post_load = Some(4);
    model.modules[0].loaded = false;
    model.modules[1].post_load = Some(4);
    model.modules[1].load_state = 3; // OBS_MODULE_DISABLED
    model.modules[1].next = Some(2);
    model.modules[2].post_load = Some(4);
    model.modules[2].load_state = 5; // OBS_MODULE_FAILED_TO_OPEN
    model.first_disabled = Some(1);
    let before = model.modules.clone();
    for _ in 0..2 {
        post_load(&mut model);
    }
    assert!(model.post_load_visits.is_empty());
    assert_eq!(model.first, None);
    assert_eq!(model.first_disabled, Some(1));
    assert_eq!(model.modules, before);
    assert_eq!(model.host_calls, 2);
}

#[test]
fn test_module_post_load_null_callbacks() {
    let mut model = fixture();
    model.first = Some(0);
    model.modules[0].next = Some(1);
    model.modules[1].next = Some(2);
    let before = model.modules.clone();
    post_load(&mut model);
    assert!(model.post_load_visits.is_empty());
    assert_eq!(model.modules, before);
    assert_eq!(model.first, Some(0));
    assert_eq!(model.host_calls, 1);
}

#[test]
fn test_module_post_load_traversal_order_and_repeat() {
    let mut model = fixture();
    model.first = Some(5);
    model.modules[5].next = Some(2);
    model.modules[2].post_load = Some(3);
    model.modules[2].next = Some(3);
    model.modules[3].next = Some(0);
    model.modules[0].post_load = Some(1);
    model.modules[0].loaded = true;
    model.modules[0].next = Some(1);
    model.modules[1].post_load = Some(2);
    model.modules[1].next = Some(4);
    for i in 0..2 {
        post_load(&mut model);
        assert_eq!(model.post_load_visits.len(), (i + 1) * 3);
        assert_eq!(model.post_load_visits[i * 3], 3);
        assert_eq!(model.post_load_visits[i * 3 + 1], 1);
        assert_eq!(model.post_load_visits[i * 3 + 2], 2);
    }
    assert_eq!(model.first, Some(5));
    assert_eq!(model.modules[5].next, Some(2));
    assert_eq!(model.modules[2].next, Some(3));
    assert_eq!(model.modules[3].next, Some(0));
    assert_eq!(model.modules[0].next, Some(1));
    assert_eq!(model.modules[1].next, Some(4));
    assert_eq!(model.modules[4].next, None);
    assert_eq!(model.host_calls, 2);
    assert_eq!(model.reads.borrow()[1], Read::FirstModule);
}

#[test]
fn test_module_post_load_list_membership_controls_dispatch() {
    let mut model = fixture();
    // Characterized, not endorsed: flags never override list membership.
    model.first = Some(2);
    model.modules[2].post_load = Some(3);
    model.modules[2].loaded = false;
    model.modules[2].load_state = 3;
    model.first_disabled = Some(3);
    model.modules[3].post_load = Some(4);
    model.modules[3].loaded = true;
    model.modules[3].load_state = 1;
    model.modules[4].post_load = Some(4);
    let before = model.modules.clone();
    post_load(&mut model);
    assert_eq!(model.post_load_visits.len(), 1);
    assert_eq!(model.post_load_visits[0], 3);
    assert_eq!(model.modules, before);
    assert_eq!(model.first, Some(2));
    assert_eq!(model.first_disabled, Some(3));
}

#[test]
fn callback_changes_are_read_live() {
    let mut model = fixture();
    model.first = Some(0);
    model.modules[0].next = Some(1);
    model.modules[0].post_load = Some(5);
    model.modules[1].post_load = Some(4);
    post_load(&mut model);
    assert_eq!(model.post_load_visits, [5]);
    model.modules[0].next = Some(1);
    model.modules[0].post_load = Some(6);
    post_load(&mut model);
    assert_eq!(model.post_load_visits, [5, 6, 3]);
}
