//! Safe mirrors of the frozen test_hotkey_unregister_entry.c recorder.
use obs_hotkey::unregister_entry::{Registry, Removal, unregister};

#[derive(Default)]
pub(super) struct Recorder {
    pub present: bool,
    pub locked: bool,
    pub order: Vec<u8>,
    pub singles: Vec<usize>,
    pub pairs: Vec<usize>,
    pub next_pair: usize,
    pub pair: Option<(usize, [usize; 2])>,
    pub freed: usize,
}

impl Registry for Recorder {
    fn lock(&mut self) -> bool {
        assert!(!self.locked);
        if !self.present {
            return false;
        }
        self.locked = true;
        self.order.push(b'L');
        true
    }
    fn single(&mut self, id: usize) {
        assert!(self.locked);
        self.singles.push(id);
        self.order.push(b'R');
    }
    fn pair(&mut self, id: usize) {
        assert!(self.locked);
        self.pairs.push(id);
        if id >= self.next_pair {
            return;
        }
        if let Some((pair_id, members)) = self.pair
            && pair_id == id
        {
            self.single(members[0]);
            assert!(self.pair.is_some());
            self.single(members[1]);
            assert!(self.pair.is_some());
            self.pair = None;
            self.freed += 1;
        }
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        self.locked = false;
        self.order.push(b'U');
    }
}

#[test]
fn mirrors_test_hotkey_unregister_entry_no_core() {
    let mut r = Recorder::default();
    unregister(&mut r, usize::MAX, Removal::Single);
    unregister(&mut r, usize::MAX, Removal::Pair);
    assert!(r.order.is_empty());
    assert!(r.singles.is_empty());
    assert!(r.pairs.is_empty());
    assert!(!r.locked);
}

#[test]
fn mirrors_test_hotkey_unregister_entry_single_forwarding() {
    let mut r = Recorder {
        present: true,
        ..Default::default()
    };
    let ids = [0, (u32::MAX as usize).wrapping_add(1), usize::MAX, 0];
    for id in ids {
        unregister(&mut r, id, Removal::Single);
    }
    assert_eq!(r.singles, ids);
    assert_eq!(r.order, b"LRULRULRULRU");
    assert!(r.pairs.is_empty());
    assert!(!r.locked);
}

#[test]
fn mirrors_test_hotkey_unregister_entry_missing_pair() {
    let mut r = Recorder {
        present: true,
        next_pair: 4,
        ..Default::default()
    };
    for id in [0, 3, 4, usize::MAX] {
        unregister(&mut r, id, Removal::Pair);
    }
    assert!(r.singles.is_empty());
    assert_eq!(r.pairs, [0, 3, 4, usize::MAX]);
    assert_eq!(r.order, b"LULULULU");
    assert!(!r.locked);
}

#[test]
fn mirrors_test_hotkey_unregister_entry_pair_member_order() {
    let id = (u32::MAX as usize).wrapping_add(1);
    let members = [usize::MAX, id];
    let mut r = Recorder {
        present: true,
        next_pair: id + 1,
        pair: Some((id, members)),
        ..Default::default()
    };
    unregister(&mut r, id, Removal::Pair);
    assert_eq!(r.singles, members);
    assert_eq!(r.order, b"LRRU");
    assert!(r.pair.is_none());
    assert_eq!(r.freed, 1);
    unregister(&mut r, id, Removal::Pair);
    assert_eq!(r.singles, members);
    assert_eq!(r.order, b"LRRULU");
    assert_eq!(r.freed, 1);
    assert!(!r.locked);
}
