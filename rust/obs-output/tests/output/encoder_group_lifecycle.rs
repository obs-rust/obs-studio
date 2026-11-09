//! Tier 1 mirrors test_encoder_group_lifecycle.c without FFI or unsafe code.
use obs_output::encoder_group_lifecycle::{self as core, Backend};

#[derive(Default, Debug)]
pub struct Model {
    pub fail_init: bool,
    pub live: bool,
    pub locked: bool,
    pub initialized: bool,
    pub started: u32,
    pub deferred: bool,
    pub encoders: Vec<bool>,
    pub trace: Vec<u32>,
}
impl Backend for Model {
    type Group = ();
    fn allocate_zeroed(&mut self) {
        assert!(!self.live);
        self.live = true;
        self.trace.push(1);
    }
    fn init_mutex(&mut self, (): ()) -> bool {
        self.trace.push(2);
        self.initialized = !self.fail_init;
        self.initialized
    }
    fn free_group(&mut self, (): ()) {
        assert!(!self.initialized && !self.locked && self.encoders.is_empty());
        self.live = false;
        self.trace.push(8);
    }
    fn lock(&mut self, (): ()) {
        assert!(self.live && self.initialized && !self.locked);
        self.locked = true;
        self.trace.push(3);
    }
    fn unlock(&mut self, (): ()) {
        assert!(self.locked);
        self.locked = false;
        self.trace.push(6);
    }
    fn destroy_mutex(&mut self, (): ()) {
        assert!(self.initialized && !self.locked);
        self.initialized = false;
        self.trace.push(7);
    }
    fn started(&mut self, (): ()) -> u32 {
        self.started
    }
    fn defer(&mut self, (): ()) {
        self.deferred = true;
        self.trace.push(4);
    }
    fn count(&mut self, (): ()) -> usize {
        self.encoders.len()
    }
    fn clear_back_pointer(&mut self, (): (), index: usize) {
        assert!(self.locked && self.initialized && self.encoders[index]);
        self.encoders[index] = false;
        self.trace.push(100 + index as u32);
    }
    fn release(&mut self, (): (), index: usize) {
        assert!(self.locked && self.initialized && !self.encoders[index]);
        self.trace.push(200 + index as u32);
    }
    fn free_list(&mut self, (): ()) {
        assert!(self.locked && self.encoders.iter().all(|v| !v));
        self.encoders.clear();
        self.trace.push(5);
    }
}

#[test]
fn test_encoder_group_lifecycle_creation_and_failure() {
    let mut backend = Model {
        fail_init: true,
        ..Model::default()
    };
    assert_eq!(core::create(&mut backend), None);
    assert!(!backend.live);
    assert_eq!(backend.trace, [1, 2, 8]);
    let mut backend = Model::default();
    assert_eq!(core::create(&mut backend), Some(()));
    assert!(backend.live && backend.initialized && !backend.locked);
    assert_eq!(backend.started, 0);
    assert!(!backend.deferred);
    assert!(backend.encoders.is_empty());
    core::destroy(&mut backend, Some(()));
    assert!(!backend.live);
    assert_eq!(backend.trace, [1, 2, 3, 5, 6, 7, 8]);
}

#[test]
fn test_encoder_group_lifecycle_deferred_and_ordered_teardown() {
    for count in 0..=3 {
        let mut backend = Model::default();
        core::create(&mut backend).unwrap();
        backend.encoders = vec![true; count];
        backend.started = 2;
        for _ in 0..2 {
            core::destroy(&mut backend, Some(()));
            assert!(backend.live && backend.initialized && !backend.locked && backend.deferred);
            assert_eq!(backend.started, 2);
            assert_eq!(backend.encoders, vec![true; count]);
        }
        assert_eq!(backend.trace, [1, 2, 3, 4, 6, 3, 4, 6]);
        backend.started = 0;
        if count % 2 == 1 {
            backend.lock(());
            core::actually_destroy(&mut backend, ());
        } else {
            core::destroy(&mut backend, Some(()));
        }
        let mut expected = vec![1, 2, 3, 4, 6, 3, 4, 6, 3];
        for i in 0..count as u32 {
            expected.extend([100 + i, 200 + i]);
        }
        expected.extend([5, 6, 7, 8]);
        assert_eq!(backend.trace, expected);
        assert!(!backend.live && !backend.locked && !backend.initialized);
    }
}

#[test]
fn test_encoder_group_lifecycle_null_destroy() {
    let mut backend = Model::default();
    core::destroy(&mut backend, None);
    assert!(backend.trace.is_empty());
}
