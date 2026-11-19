//! Mirrors test_source_audio_capture_callbacks.c without FFI, locking or
//! callback invocation (Tier 3 checks the lock and the real C list).
use obs_source::audio_capture_callbacks::{Registry, update};
#[derive(Default)]
struct List(Vec<(u8, u8)>);
impl Registry for List {
    type Pair = (u8, u8);
    fn count(&self) -> usize {
        self.0.len()
    }
    fn pair(&self, i: usize) -> Self::Pair {
        self.0[i]
    }
    fn append(&mut self, p: Self::Pair) {
        self.0.push(p);
    }
    fn erase(&mut self, i: usize) {
        self.0.remove(i);
    }
}
fn apply(list: &mut List, pair: (u8, u8), add: bool) {
    update(Some(list), pair, add, |_| panic!("valid source logged"));
}
#[test]
fn test_source_audio_capture_callbacks_invalid_source() {
    for (add, name) in [
        (true, c"obs_source_add_audio_capture_callback"),
        (false, c"obs_source_remove_audio_capture_callback"),
    ] {
        let mut logs = Vec::new();
        update::<List>(None, (1, 0), add, |name| logs.push(name));
        assert_eq!(logs, [name]);
    }
}
#[test]
fn test_source_audio_capture_callbacks_pair_identity_order_and_removal() {
    // (callback, param): callback 1 = a, 2 = b; param n = &tokens[n].
    let mut list = List::default();
    apply(&mut list, (1, 0), false);
    assert!(list.0.is_empty());
    for pair in [(1, 0), (2, 0), (1, 1), (1, 0), (2, 2)] {
        apply(&mut list, pair, true);
    }
    assert_eq!(list.0, [(1, 0), (2, 0), (1, 1), (1, 0), (2, 2)]);
    apply(&mut list, (1, 2), false);
    assert_eq!(list.0, [(1, 0), (2, 0), (1, 1), (1, 0), (2, 2)]);
    apply(&mut list, (1, 0), false);
    assert_eq!(list.0, [(2, 0), (1, 1), (1, 0), (2, 2)]);
    apply(&mut list, (1, 0), false);
    assert_eq!(list.0, [(2, 0), (1, 1), (2, 2)]);
    apply(&mut list, (1, 0), false);
    assert_eq!(list.0, [(2, 0), (1, 1), (2, 2)]);
    apply(&mut list, (1, 1), false);
    apply(&mut list, (2, 2), false);
    assert_eq!(list.0, [(2, 0)]);
    apply(&mut list, (2, 0), false);
    assert!(list.0.is_empty());
}
#[test]
fn test_source_audio_capture_callbacks_null_pairs_are_stored() {
    // 0 is NULL; param 1 = &tokens[0], 2 = &tokens[1].
    let mut list = List::default();
    for pair in [(0, 0), (1, 0), (0, 1), (0, 0)] {
        apply(&mut list, pair, true);
    }
    assert_eq!(list.0, [(0, 0), (1, 0), (0, 1), (0, 0)]);
    apply(&mut list, (0, 2), false);
    assert_eq!(list.0, [(0, 0), (1, 0), (0, 1), (0, 0)]);
    apply(&mut list, (0, 0), false);
    assert_eq!(list.0, [(1, 0), (0, 1), (0, 0)]);
    apply(&mut list, (1, 0), false);
    apply(&mut list, (0, 1), false);
    assert_eq!(list.0, [(0, 0)]);
    apply(&mut list, (0, 0), false);
    assert!(list.0.is_empty());
    apply(&mut list, (0, 0), false);
    assert!(list.0.is_empty());
}
