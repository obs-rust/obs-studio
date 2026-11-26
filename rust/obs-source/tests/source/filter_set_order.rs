//! Tier 1 mirrors of test_source_filter_set_order.c without FFI or unsafe
//! code.
use core::ffi::CStr;
use obs_source::filter_set_order::{
    FILTER_PARAMETER, FilterChain, SET_ORDER_FUNCTION, SOURCE_PARAMETER, set_order,
};
use obs_source::source_filter_chain::{ASYNC, MOVE_BOTTOM, MOVE_DOWN, MOVE_TOP, MOVE_UP, VIDEO};
use std::cell::{Cell, RefCell};

/// The most filters a chain gets here.
const FILTERS: usize = 7;
/// The id of the filter that is on no list.
const OUTSIDER: usize = 99;

const MOVEMENTS: [i32; 4] = [MOVE_UP, MOVE_DOWN, MOVE_TOP, MOVE_BOTTOM];
const OUT_OF_ENUM: [i32; 6] = [-1, 4, 5, 1000, i32::MIN, i32::MAX];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Lock,
    Count,
    At(usize),
    Flags(usize),
    Move(usize, usize),
    /// A filter and its new target (`None`: the source).
    Target(usize, Option<usize>),
    Unlock,
    Signal,
}

/// A model source: its list holds filter ids, filter `i` is async when bit
/// `i` of the mask is set, and every call is recorded in order.
struct Model {
    list: RefCell<Vec<usize>>,
    async_mask: u32,
    /// Each filter's target: `None` untouched, `Some(None)` the source.
    targets: RefCell<Vec<Option<Option<usize>>>>,
    locked: Cell<bool>,
    events: RefCell<Vec<Event>>,
}

impl Model {
    fn new(count: usize, async_mask: u32) -> Self {
        Self {
            list: RefCell::new((0..count).collect()),
            async_mask,
            targets: RefCell::new(vec![None; FILTERS]),
            locked: Cell::new(false),
            events: RefCell::new(Vec::new()),
        }
    }
    fn push(&self, event: Event) {
        self.events.borrow_mut().push(event);
    }
    fn events(&self) -> Vec<Event> {
        self.events.borrow().clone()
    }
    fn signals(&self) -> usize {
        self.events()
            .iter()
            .filter(|e| **e == Event::Signal)
            .count()
    }
    fn list(&self) -> Vec<usize> {
        self.list.borrow().clone()
    }
    /// Every listed filter targets the next one, the last one the source.
    fn assert_retargeted(&self) {
        let list = self.list();
        let targets = self.targets.borrow();
        for (i, filter) in list.iter().enumerate() {
            assert_eq!(targets[*filter], Some(list.get(i + 1).copied()));
        }
    }
    fn assert_untargeted(&self) {
        assert!(self.targets.borrow().iter().all(Option::is_none));
    }
}

impl FilterChain for Model {
    type Filter = usize;
    fn lock(&self) {
        assert!(!self.locked.replace(true), "locked twice");
        self.push(Event::Lock);
    }
    fn count(&self) -> usize {
        assert!(self.locked.get());
        self.push(Event::Count);
        self.list.borrow().len()
    }
    fn filter(&self, index: usize) -> usize {
        assert!(self.locked.get());
        self.push(Event::At(index));
        self.list.borrow()[index]
    }
    fn flags(&self, filter: usize) -> u32 {
        assert!(self.locked.get());
        self.push(Event::Flags(filter));
        let kind = if filter < 32 && self.async_mask & (1 << filter) != 0 {
            ASYNC
        } else {
            0
        };
        VIDEO | kind | 0x5a00
    }
    fn move_filter(&self, from: usize, to: usize) {
        assert!(self.locked.get());
        self.push(Event::Move(from, to));
        let mut list = self.list.borrow_mut();
        let filter = list.remove(from);
        list.insert(to, filter);
    }
    fn set_target(&self, filter: usize, target: Option<usize>) {
        assert!(self.locked.get());
        self.push(Event::Target(filter, target));
        self.targets.borrow_mut()[filter] = Some(target);
    }
    fn unlock(&self) {
        assert!(self.locked.replace(false), "unlocked while not locked");
        self.push(Event::Unlock);
    }
    fn signal(&self) {
        assert!(!self.locked.get(), "signalled under the lock");
        self.push(Event::Signal);
    }
}

fn no_log(function: &'static CStr, parameter: &'static CStr) {
    panic!("logged {function:?}: {parameter:?}");
}

/// One move on a fresh model: the list ends as `order` (`None`: unchanged,
/// nothing retargeted, no signal).
fn check_move(
    count: usize,
    async_mask: u32,
    idx: usize,
    movement: i32,
    order: Option<&[usize]>,
) -> Model {
    let model = Model::new(count, async_mask);
    set_order(Some(&model), Some(idx), movement, no_log);
    assert!(!model.locked.get());
    let events = model.events();
    assert_eq!(events.first(), Some(&Event::Lock));
    match order {
        Some(order) => {
            assert_eq!(
                model.list(),
                order,
                "{count} {async_mask:#b} {idx} {movement}"
            );
            model.assert_retargeted();
            assert_eq!(model.signals(), 1);
            // The signal comes last, after the unlock.
            assert_eq!(events[events.len() - 2..], [Event::Unlock, Event::Signal]);
        }
        None => {
            assert_eq!(model.list(), (0..count).collect::<Vec<_>>());
            model.assert_untargeted();
            assert_eq!(model.signals(), 0);
            assert_eq!(events.last(), Some(&Event::Unlock));
            assert!(!events.iter().any(|e| matches!(e, Event::Move(..))));
        }
    }
    model
}

/// Mirrors null_arguments_log_once_and_do_nothing.
#[test]
fn null_arguments_log_once_and_do_nothing() {
    let model = Model::new(3, 0);
    let mut logs = Vec::new();

    set_order(None::<&Model>, Some(0), MOVE_UP, |f, p| logs.push((f, p)));
    // The source is checked first.
    set_order(None::<&Model>, None, MOVE_TOP, |f, p| logs.push((f, p)));
    for movement in MOVEMENTS {
        set_order(Some(&model), None, movement, |f, p| logs.push((f, p)));
    }

    let mut expected = vec![
        (SET_ORDER_FUNCTION, SOURCE_PARAMETER),
        (SET_ORDER_FUNCTION, SOURCE_PARAMETER),
    ];
    expected.extend([(SET_ORDER_FUNCTION, FILTER_PARAMETER); 4]);
    assert_eq!(logs, expected);
    // Nothing of the source was read, locked, written or signalled.
    assert!(model.events().is_empty());
    model.assert_untargeted();
    assert_eq!(SET_ORDER_FUNCTION, c"obs_source_filter_set_order");
    assert_eq!(SOURCE_PARAMETER, c"source");
    assert_eq!(FILTER_PARAMETER, c"filter");
}

/// Mirrors unlisted_filter_moves_nothing: the lookup reads every entry
/// under the lock and finds nothing.
#[test]
fn unlisted_filter_moves_nothing() {
    for count in 0..=FILTERS {
        for movement in MOVEMENTS.into_iter().chain(OUT_OF_ENUM) {
            let model = Model::new(count, 0b10101);
            set_order(Some(&model), Some(OUTSIDER), movement, no_log);
            let mut expected = vec![Event::Lock];
            for index in 0..count {
                expected.extend([Event::Count, Event::At(index)]);
            }
            expected.extend([Event::Count, Event::Unlock]);
            assert_eq!(model.events(), expected, "{count} {movement}");
            assert_eq!(model.list(), (0..count).collect::<Vec<_>>());
            model.assert_untargeted();
        }
    }
}

/// Mirrors moves_walk_to_the_same_kind_and_the_ends.
#[test]
fn moves_walk_to_the_same_kind_and_the_ends() {
    let a = |i: u32| 1u32 << i;
    // Up: to the next filter, whatever is past it.
    check_move(4, 0, 1, MOVE_UP, Some(&[0, 2, 1, 3]));
    // Up skips filters of the other kind.
    check_move(5, a(2) | a(3), 1, MOVE_UP, Some(&[0, 2, 3, 4, 1]));
    check_move(5, a(0) | a(3), 0, MOVE_UP, Some(&[1, 2, 3, 0, 4]));
    // No filter of the same kind above: nothing.
    check_move(4, a(1), 1, MOVE_UP, None);
    check_move(4, 0, 3, MOVE_UP, None);

    // Down mirrors up.
    check_move(4, 0, 2, MOVE_DOWN, Some(&[0, 2, 1, 3]));
    check_move(5, a(1) | a(2), 3, MOVE_DOWN, Some(&[3, 0, 1, 2, 4]));
    check_move(5, a(4) | a(1), 4, MOVE_DOWN, Some(&[0, 4, 1, 2, 3]));
    check_move(4, a(2), 2, MOVE_DOWN, None);
    check_move(4, 0, 0, MOVE_DOWN, None);

    // Top and bottom go to the ends, whatever the kinds.
    check_move(5, a(1), 1, MOVE_TOP, Some(&[0, 2, 3, 4, 1]));
    check_move(5, a(1), 3, MOVE_BOTTOM, Some(&[3, 0, 1, 2, 4]));
    check_move(3, 0, 2, MOVE_TOP, None);
    check_move(3, 0, 0, MOVE_BOTTOM, None);

    // A single filter goes nowhere.
    for movement in MOVEMENTS {
        check_move(1, 0, 0, movement, None);
    }
}

/// Where the filter at `idx` goes, as the movement rules say.
fn model_move(count: usize, async_mask: u32, idx: usize, movement: i32) -> Option<Vec<usize>> {
    let kind = |i: usize| async_mask & (1 << i) != 0;
    let to = match movement {
        MOVE_UP => (idx + 1..count).find(|&j| kind(j) == kind(idx))?,
        MOVE_DOWN => (0..idx).rev().find(|&j| kind(j) == kind(idx))?,
        MOVE_TOP => (idx != count - 1).then_some(count - 1)?,
        MOVE_BOTTOM => (idx != 0).then_some(0)?,
        _ => return None,
    };
    let mut order: Vec<usize> = (0..count).collect();
    let filter = order.remove(idx);
    order.insert(to, filter);
    Some(order)
}

/// Mirrors every_chain_moves_as_the_rules_say, on chains of up to 7.
#[test]
fn every_chain_moves_as_the_rules_say() {
    for count in 1..=FILTERS {
        for async_mask in 0..1u32 << count {
            for idx in 0..count {
                for movement in MOVEMENTS {
                    let order = model_move(count, async_mask, idx, movement);
                    check_move(count, async_mask, idx, movement, order.as_deref());
                }
            }
        }
    }
}

/// Mirrors out_of_enum_movement_moves_no_filter: the port moves,
/// retargets and signals nothing (#2981).
#[test]
fn out_of_enum_movement_moves_nothing() {
    for count in 1..=FILTERS {
        for idx in 0..count {
            for movement in OUT_OF_ENUM {
                check_move(count, 0b10010, idx, movement, None);
            }
        }
    }
}

/// The C's reads and writes, in order: the lookup, the walk reading the
/// moved filter's flags again at every step, the move, then each filter's
/// target from the first.
#[test]
fn a_move_reads_and_writes_in_the_c_order() {
    // Filter 1 (sync) goes up past async 2 to sync 3.
    let model = check_move(4, 0b0100, 1, MOVE_UP, Some(&[0, 2, 3, 1]));
    use Event::*;
    assert_eq!(
        model.events(),
        [
            Lock,
            // da_find
            Count,
            At(0),
            Count,
            At(1),
            // find_next_filter(), one step per filter
            Flags(1),
            Count,
            At(2),
            Flags(2),
            Flags(1),
            Count,
            At(3),
            Flags(3),
            Move(1, 3),
            // reorder_filter_targets()
            Count,
            Count,
            At(1),
            At(0),
            Target(0, Some(2)),
            Count,
            Count,
            At(2),
            At(1),
            Target(2, Some(3)),
            Count,
            Count,
            At(3),
            At(2),
            Target(3, Some(1)),
            Count,
            Count,
            At(3),
            Target(1, None),
            Count,
            Unlock,
            Signal,
        ]
    );

    // Top reads no flags.
    let model = check_move(3, 0b001, 0, MOVE_TOP, Some(&[1, 2, 0]));
    assert!(!model.events().iter().any(|e| matches!(e, Flags(_))));
    assert!(model.events().contains(&Move(0, 2)));
}
