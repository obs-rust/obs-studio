//! Safe equivalents of routed dispatch exercised by test_hotkey_pair_latch.
use obs_hotkey::routed_callback::{Registry, trigger};

#[derive(Default)]
struct Fake {
    initialized: bool,
    enabled: bool,
    depth: usize,
    nested: bool,
    events: Vec<(usize, bool, usize)>,
    locks: usize,
    unlocks: usize,
    finds: usize,
}
impl Registry for Fake {
    type Hotkey = usize;
    fn lock(&mut self) -> bool {
        self.locks += 1;
        if self.initialized {
            self.depth += 1;
        }
        self.initialized
    }
    fn enabled(&self) -> bool {
        assert!(self.depth > 0);
        self.enabled
    }
    fn find(&mut self, id: usize) -> Option<usize> {
        assert!(self.depth > 0);
        self.finds += 1;
        (id < 2).then_some(id)
    }
    fn call(&mut self, hotkey: usize, id: usize, pressed: bool) {
        assert_eq!(hotkey, id);
        assert!(self.depth > 0);
        self.events.push((id, pressed, self.depth));
        if self.nested && self.depth == 1 {
            trigger(self, 1 - id, !pressed);
        }
        assert!(self.depth > 0);
    }
    fn unlock(&mut self) {
        assert!(self.depth > 0);
        self.depth -= 1;
        self.unlocks += 1;
    }
}

#[test]
fn test_hotkey_pair_latch_routed_press_release_and_reentrant_dispatch() {
    let mut r = Fake {
        initialized: true,
        enabled: true,
        ..Fake::default()
    };
    trigger(&mut r, 0, true);
    trigger(&mut r, 1, false);
    assert_eq!(r.events, [(0, true, 1), (1, false, 1)]);
    r.events.clear();
    r.nested = true;
    trigger(&mut r, 0, true);
    assert_eq!(r.events, [(0, true, 1), (1, false, 2)]);
    assert_eq!((r.depth, r.locks, r.unlocks), (0, 4, 4));
}

#[test]
fn absent_core_disabled_and_unknown_id_are_noops() {
    for initialized in [false, true] {
        for enabled in [false, true] {
            for id in [0, 1, 2, usize::MAX] {
                for pressed in [false, true] {
                    let mut r = Fake {
                        initialized,
                        enabled,
                        ..Fake::default()
                    };
                    trigger(&mut r, id, pressed);
                    assert_eq!(r.locks, 1);
                    assert_eq!(r.unlocks, usize::from(initialized));
                    assert_eq!(r.finds, usize::from(initialized && enabled));
                    assert_eq!(
                        r.events.len(),
                        usize::from(initialized && enabled && id < 2)
                    );
                    assert_eq!(r.depth, 0);
                }
            }
        }
    }
}
