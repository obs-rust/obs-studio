use obs_hotkey::update_atomic::{AtomicUpdate, update};

#[derive(Default)]
struct Registry {
    initialized: bool,
    depth: usize,
    events: Vec<(&'static str, usize)>,
}

impl AtomicUpdate for Registry {
    fn lock(&mut self) -> bool {
        self.events.push(("lock", self.depth));
        if self.initialized {
            self.depth += 1;
        }
        self.initialized
    }

    fn unlock(&mut self) {
        assert!(self.depth > 0);
        self.events.push(("unlock", self.depth));
        self.depth -= 1;
    }
}

#[test]
fn failed_lock_is_noop() {
    let mut registry = Registry::default();
    update(&mut registry, |_| panic!("failed lock called callback"));
    assert_eq!(registry.events, [("lock", 0)]);
    assert_eq!(registry.depth, 0);
}

#[test]
fn synchronous_callback_and_recursive_update() {
    let mut registry = Registry {
        initialized: true,
        ..Registry::default()
    };
    let mut calls = 0;
    update(&mut registry, |registry| {
        calls += 1;
        registry.events.push(("callback", registry.depth));
        update(registry, |registry| {
            calls += 1;
            registry.events.push(("nested", registry.depth));
        });
        assert_eq!(registry.depth, 1);
    });
    assert_eq!(calls, 2);
    assert_eq!(registry.depth, 0);
    assert_eq!(
        registry.events,
        [
            ("lock", 0),
            ("callback", 1),
            ("lock", 1),
            ("nested", 2),
            ("unlock", 2),
            ("unlock", 1),
        ]
    );
}
