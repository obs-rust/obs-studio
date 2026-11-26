//! Tier 1 for `obs_sceneitem_set_order()` (`item_set_order`): the core over
//! a model scene whose links, lock, reference and signal behave as the
//! C-owned ones do (`detach_sceneitem()` and `attach_sceneitem()` copied
//! onto indexes). Every movement on every item of lists of 1 to 6 items,
//! the NULL, detached and dying cases, and the C's order of calls: the
//! reference before the lock, the neighbours read under it, detach before
//! the anchor's own reads, attach, unlock, `reorder` on the parent after the
//! unlock, release last.
use obs_scene::item_order::{
    OBS_ORDER_MOVE_BOTTOM, OBS_ORDER_MOVE_DOWN, OBS_ORDER_MOVE_TOP, OBS_ORDER_MOVE_UP,
};
use obs_scene::item_set_order::{SetOrderScene, set_order};

const SCENE: u8 = 7;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Item {
    prev: Option<usize>,
    next: Option<usize>,
    parent: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Ref(Option<u8>),
    Lock,
    Next(usize),
    Prev(usize),
    Detach(usize),
    Attach(usize, Option<usize>),
    Unlock,
    Signal(usize, Option<u8>, bool),
    Release,
}

struct Model {
    items: Vec<Item>,
    first: Option<usize>,
    refs: u32,
    locked: bool,
    trace: Vec<Event>,
}

impl Model {
    /// `count` items linked bottom to top, all in the scene.
    fn new(count: usize) -> Self {
        let items = (0..count)
            .map(|i| Item {
                prev: i.checked_sub(1),
                next: (i + 1 < count).then_some(i + 1),
                parent: Some(SCENE),
            })
            .collect();
        Self {
            items,
            first: (count > 0).then_some(0),
            refs: 1,
            locked: false,
            trace: Vec::new(),
        }
    }

    /// The list from `first_item` up, checking every link and parent.
    fn order(&self) -> Vec<usize> {
        let mut order = Vec::new();
        let mut prev = None;
        let mut at = self.first;
        while let Some(i) = at {
            let item = self.items[i];
            assert_eq!(item.prev, prev, "prev link of {i}");
            assert_eq!(item.parent, Some(SCENE), "parent of {i}");
            assert!(order.len() < self.items.len(), "cycle");
            order.push(i);
            prev = at;
            at = item.next;
        }
        order
    }
}

impl SetOrderScene for Model {
    type Item = usize;
    type Scene = u8;
    fn parent_ref(&mut self, item: usize) -> Option<u8> {
        assert!(!self.locked);
        let parent = self.items[item].parent;
        self.trace.push(Event::Ref(parent));
        let parent = parent?;
        if self.refs == 0 {
            return None;
        }
        self.refs += 1;
        Some(parent)
    }
    fn lock(&mut self, scene: u8) {
        assert_eq!(scene, SCENE);
        assert!(!self.locked);
        self.locked = true;
        self.trace.push(Event::Lock);
    }
    fn next(&mut self, item: usize) -> Option<usize> {
        assert!(self.locked, "next read outside the lock");
        self.trace.push(Event::Next(item));
        self.items[item].next
    }
    fn prev(&mut self, item: usize) -> Option<usize> {
        assert!(self.locked, "prev read outside the lock");
        self.trace.push(Event::Prev(item));
        self.items[item].prev
    }
    fn detach(&mut self, item: usize) {
        assert!(self.locked);
        self.trace.push(Event::Detach(item));
        let Item { prev, next, .. } = self.items[item];
        match prev {
            Some(prev) => self.items[prev].next = next,
            None => self.first = next,
        }
        if let Some(next) = next {
            self.items[next].prev = prev;
        }
        self.items[item].parent = None;
    }
    fn attach(&mut self, scene: u8, item: usize, after: Option<usize>) {
        assert!(self.locked);
        self.trace.push(Event::Attach(item, after));
        self.items[item].prev = after;
        self.items[item].parent = Some(scene);
        match after {
            Some(after) => {
                let next = self.items[after].next;
                self.items[item].next = next;
                if let Some(next) = next {
                    self.items[next].prev = Some(item);
                }
                self.items[after].next = Some(item);
            }
            None => {
                let first = self.first;
                self.items[item].next = first;
                if let Some(first) = first {
                    self.items[first].prev = Some(item);
                }
                self.first = Some(item);
            }
        }
    }
    fn unlock(&mut self, scene: u8) {
        assert_eq!(scene, SCENE);
        assert!(self.locked);
        self.locked = false;
        self.trace.push(Event::Unlock);
    }
    fn signal_reorder(&mut self, item: usize) {
        let parent = self.items[item].parent;
        self.trace.push(Event::Signal(item, parent, !self.locked));
    }
    fn release(&mut self, scene: u8) {
        assert_eq!(scene, SCENE);
        assert!(!self.locked);
        self.refs -= 1;
        self.trace.push(Event::Release);
    }
}

/// The list `obs_sceneitem_set_order()` leaves, from the movement's meaning.
fn expected(count: usize, index: usize, movement: i32) -> Vec<usize> {
    let mut list: Vec<usize> = (0..count).collect();
    match movement {
        OBS_ORDER_MOVE_UP if index + 1 < count => list.swap(index, index + 1),
        OBS_ORDER_MOVE_DOWN if index > 0 => list.swap(index, index - 1),
        OBS_ORDER_MOVE_TOP => {
            let item = list.remove(index);
            list.push(item);
        }
        OBS_ORDER_MOVE_BOTTOM => {
            let item = list.remove(index);
            list.insert(0, item);
        }
        _ => {}
    }
    list
}

const MOVEMENTS: [i32; 9] = [
    OBS_ORDER_MOVE_UP,
    OBS_ORDER_MOVE_DOWN,
    OBS_ORDER_MOVE_TOP,
    OBS_ORDER_MOVE_BOTTOM,
    -1,
    4,
    5,
    i32::MIN,
    i32::MAX,
];

fn in_enum(movement: i32) -> bool {
    (OBS_ORDER_MOVE_UP..=OBS_ORDER_MOVE_BOTTOM).contains(&movement)
}

#[test]
fn every_movement_on_every_item() {
    for count in 1..=6 {
        for index in 0..count {
            for movement in MOVEMENTS {
                let mut model = Model::new(count);
                set_order(&mut model, Some(index), movement);
                let case = format!("count {count} index {index} movement {movement}");
                assert_eq!(model.order(), expected(count, index, movement), "{case}");
                assert!(!model.locked, "{case}");
                assert_eq!(model.refs, 1, "{case}");
                let trace = &model.trace;
                assert_eq!(trace[..2], [Event::Ref(Some(SCENE)), Event::Lock], "{case}");
                assert_eq!(trace.last(), Some(&Event::Release), "{case}");
                let signals: Vec<_> = trace
                    .iter()
                    .filter(|e| matches!(e, Event::Signal(..)))
                    .collect();
                let detaches = trace
                    .iter()
                    .filter(|e| matches!(e, Event::Detach(_)))
                    .count();
                let attaches = trace
                    .iter()
                    .filter(|e| matches!(e, Event::Attach(..)))
                    .count();
                if in_enum(movement) {
                    // Detached and attached once, unlock, signal, release.
                    assert_eq!((detaches, attaches), (1, 1), "{case}");
                    assert_eq!(
                        signals,
                        [&Event::Signal(index, Some(SCENE), true)],
                        "{case}"
                    );
                    assert_eq!(
                        trace[trace.len() - 3..],
                        [
                            Event::Unlock,
                            Event::Signal(index, Some(SCENE), true),
                            Event::Release
                        ],
                        "{case}"
                    );
                } else {
                    // Stay: the neighbours read, then unlock and release.
                    assert_eq!((detaches, attaches), (0, 0), "{case}");
                    assert!(signals.is_empty(), "{case}");
                    assert_eq!(
                        trace[2..],
                        [
                            Event::Next(index),
                            Event::Prev(index),
                            Event::Unlock,
                            Event::Release
                        ],
                        "{case}"
                    );
                }
            }
        }
    }
}

/// The exact calls of each anchor on the list 0 1 2 3.
#[test]
fn call_order_of_each_anchor() {
    use Event::*;
    let head = [Ref(Some(SCENE)), Lock];
    let tail = |item| [Unlock, Signal(item, Some(SCENE), true), Release];
    let cases: [(usize, i32, &[Event]); 8] = [
        // Down with an item below: after prev->prev, read after the detach.
        (
            2,
            OBS_ORDER_MOVE_DOWN,
            &[Next(2), Prev(2), Detach(2), Prev(1), Attach(2, Some(0))],
        ),
        // Down from the bottom: at the bottom.
        (
            0,
            OBS_ORDER_MOVE_DOWN,
            &[Next(0), Prev(0), Detach(0), Attach(0, None)],
        ),
        // Up with an item above: after next.
        (
            1,
            OBS_ORDER_MOVE_UP,
            &[Next(1), Prev(1), Detach(1), Attach(1, Some(2))],
        ),
        // Up from the top: back after prev.
        (
            3,
            OBS_ORDER_MOVE_UP,
            &[Next(3), Prev(3), Detach(3), Attach(3, Some(2))],
        ),
        // Top: the walk from next to the last item, after the detach.
        (
            0,
            OBS_ORDER_MOVE_TOP,
            &[
                Next(0),
                Prev(0),
                Detach(0),
                Next(1),
                Next(2),
                Next(3),
                Attach(0, Some(3)),
            ],
        ),
        // Top from the top: no walk, back after prev.
        (
            3,
            OBS_ORDER_MOVE_TOP,
            &[Next(3), Prev(3), Detach(3), Attach(3, Some(2))],
        ),
        // Bottom.
        (
            2,
            OBS_ORDER_MOVE_BOTTOM,
            &[Next(2), Prev(2), Detach(2), Attach(2, None)],
        ),
        // Down from position 1: prev->prev is NULL, so the bottom.
        (
            1,
            OBS_ORDER_MOVE_DOWN,
            &[Next(1), Prev(1), Detach(1), Prev(0), Attach(1, None)],
        ),
    ];
    for (index, movement, middle) in cases {
        let mut model = Model::new(4);
        set_order(&mut model, Some(index), movement);
        let mut want = head.to_vec();
        want.extend_from_slice(middle);
        want.extend_from_slice(&tail(index));
        assert_eq!(model.trace, want, "index {index} movement {movement}");
    }
}

#[test]
fn a_single_item_stays_and_signals() {
    for movement in [
        OBS_ORDER_MOVE_UP,
        OBS_ORDER_MOVE_DOWN,
        OBS_ORDER_MOVE_TOP,
        OBS_ORDER_MOVE_BOTTOM,
    ] {
        let mut model = Model::new(1);
        set_order(&mut model, Some(0), movement);
        assert_eq!(model.order(), [0]);
        assert_eq!(
            model
                .trace
                .iter()
                .filter(|e| matches!(e, Event::Signal(..)))
                .count(),
            1
        );
    }
}

#[test]
fn null_item_does_nothing() {
    let mut model = Model::new(3);
    set_order(&mut model, None, OBS_ORDER_MOVE_UP);
    assert!(model.trace.is_empty());
    assert_eq!(model.order(), [0, 1, 2]);
}

#[test]
fn detached_item_stops_at_the_reference() {
    let mut model = Model::new(3);
    model.items[1].parent = None;
    set_order(&mut model, Some(1), OBS_ORDER_MOVE_TOP);
    assert_eq!(model.trace, [Event::Ref(None)]);
    assert_eq!(model.refs, 1);
}

#[test]
fn dying_scene_stops_at_the_reference() {
    let mut model = Model::new(3);
    model.refs = 0;
    set_order(&mut model, Some(1), OBS_ORDER_MOVE_TOP);
    assert_eq!(model.trace, [Event::Ref(Some(SCENE))]);
    assert_eq!(model.refs, 0);
    assert_eq!(model.order(), [0, 1, 2]);
}

/// The sequence of test_scene_canvas_characterization's scene_item_set_order
/// on a, b, c from the bottom.
#[test]
fn characterization_sequence() {
    let (a, b, c) = (0, 1, 2);
    let reorders = |model: &Model| {
        model
            .trace
            .iter()
            .filter(|e| matches!(e, Event::Signal(..)))
            .count()
    };
    let mut model = Model::new(3);
    set_order(&mut model, None, OBS_ORDER_MOVE_UP);
    assert_eq!(model.order(), [a, b, c]);
    assert_eq!(reorders(&model), 0);
    set_order(&mut model, Some(a), OBS_ORDER_MOVE_UP);
    assert_eq!(model.order(), [b, a, c]);
    assert_eq!(reorders(&model), 1);
    set_order(&mut model, Some(a), OBS_ORDER_MOVE_TOP);
    assert_eq!(model.order(), [b, c, a]);
    // The top item moved up stays on top, and still signals.
    set_order(&mut model, Some(a), OBS_ORDER_MOVE_UP);
    assert_eq!(model.order(), [b, c, a]);
    assert_eq!(reorders(&model), 3);
    set_order(&mut model, Some(a), OBS_ORDER_MOVE_BOTTOM);
    assert_eq!(model.order(), [a, b, c]);
    set_order(&mut model, Some(a), OBS_ORDER_MOVE_DOWN);
    assert_eq!(model.order(), [a, b, c]);
    set_order(&mut model, Some(c), OBS_ORDER_MOVE_DOWN);
    assert_eq!(model.order(), [a, c, b]);
    assert_eq!(reorders(&model), 6);
    assert_eq!(model.refs, 1);
    assert!(!model.locked);
}
