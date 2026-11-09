//! Mirrors test_source_caption_callbacks.c without FFI or callback invocation.
use obs_source::caption_callbacks::{Registry, update};
#[derive(Default)]
struct List(Vec<(u8, u8)>);
impl Registry for List {
    type Pair = (u8, u8);
    fn count(&self) -> usize { self.0.len() }
    fn pair(&self, i: usize) -> Self::Pair { self.0[i] }
    fn append(&mut self, p: Self::Pair) { self.0.push(p); }
    fn erase(&mut self, i: usize) { self.0.remove(i); }
}
fn apply(list: &mut List, pair: (u8, u8), add: bool) {
    update(Some(list), pair, add, |_| panic!("valid source logged"));
}
#[test]
fn test_source_caption_callbacks_null_sources() {
    for (add, name) in [(true, c"obs_source_add_caption_callback"), (false, c"obs_source_remove_caption_callback")] {
        let mut logs = Vec::new();
        update::<List>(None, (1, 0), add, |name| logs.push(name));
        assert_eq!(logs, [name]);
    }
}
#[test]
fn test_source_caption_callbacks_callback_pairs() {
    let mut list = List::default();
    apply(&mut list, (1, 1), false);
    assert!(list.0.is_empty());
    for pair in [(1, 1), (2, 1), (1, 2), (1, 1)] { apply(&mut list, pair, true); }
    assert_eq!(list.0, [(1, 1), (2, 1), (1, 2), (1, 1)]);
    apply(&mut list, (1, 1), false);
    assert_eq!(list.0, [(2, 1), (1, 2), (1, 1)]);
    apply(&mut list, (2, 2), false);
    assert_eq!(list.0, [(2, 1), (1, 2), (1, 1)]);
    apply(&mut list, (1, 2), false);
    assert_eq!(list.0, [(2, 1), (1, 1)]);
    apply(&mut list, (2, 1), false);
    apply(&mut list, (1, 1), false);
    assert!(list.0.is_empty());
    apply(&mut list, (0, 0), true);
    assert_eq!(list.0, [(0, 0)]);
    apply(&mut list, (0, 0), false);
    assert!(list.0.is_empty());
}
