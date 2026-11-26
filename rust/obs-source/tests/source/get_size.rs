//! Tier 1: `obs_source_get_width()` and `obs_source_get_height()` on a model
//! of what the core reaches: sources with data or not, a type, filters, a
//! base width and height each, and the `filter_mutex` lock and unlock,
//! recorded in call order. The Tier 2 cmocka tests that reach these exports
//! (test_filter_size_no_target, test_async_rotation_normalize,
//! test_group_empty_bounds, ...) cover the real C.
use core::ffi::CStr;
use obs_source::get_size::{Axis, HEIGHT_FUNCTION, Libobs, SOURCE, WIDTH_FUNCTION, get};
use obs_source::size_route::{TYPE_FILTER, TYPE_TRANSITION};

const INPUT: i32 = 0;
const SCENE: i32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Log(&'static CStr),
    Lock(usize),
    Unlock(usize),
    FirstFilter(usize),
    Base(usize, Axis),
}

#[derive(Clone)]
struct Node {
    data: bool,
    kind: i32,
    filters: Vec<usize>,
    size: [u32; 2],
}

struct Model {
    nodes: Vec<Node>,
    events: Vec<Event>,
    locked: Vec<usize>,
}

impl Model {
    fn new(nodes: Vec<Node>) -> Self {
        Self {
            nodes,
            events: Vec::new(),
            locked: Vec::new(),
        }
    }
}

fn node(kind: i32, filters: &[usize], size: [u32; 2]) -> Node {
    Node {
        data: true,
        kind,
        filters: filters.to_vec(),
        size,
    }
}

impl Libobs for Model {
    type Source = usize;

    fn log_null(&mut self, function: &'static CStr) {
        self.events.push(Event::Log(function));
    }
    fn has_data(&mut self, source: usize) -> bool {
        self.nodes[source].data
    }
    fn source_type(&mut self, source: usize) -> i32 {
        self.nodes[source].kind
    }
    fn lock_filters(&mut self, source: usize) {
        assert!(!self.locked.contains(&source), "locked twice");
        self.locked.push(source);
        self.events.push(Event::Lock(source));
    }
    fn unlock_filters(&mut self, source: usize) {
        assert_eq!(self.locked.pop(), Some(source), "unlocked out of order");
        self.events.push(Event::Unlock(source));
    }
    fn has_filters(&mut self, source: usize) -> bool {
        assert!(self.locked.contains(&source), "filters read unlocked");
        !self.nodes[source].filters.is_empty()
    }
    fn first_filter(&mut self, source: usize) -> Option<usize> {
        assert!(self.locked.contains(&source), "filters read unlocked");
        self.events.push(Event::FirstFilter(source));
        self.nodes[source].filters.first().copied()
    }
    fn base_size(&mut self, source: usize, axis: Axis) -> u32 {
        self.events.push(Event::Base(source, axis));
        self.nodes[source].size[match axis {
            Axis::Width => 0,
            Axis::Height => 1,
        }]
    }
}

const AXES: [Axis; 2] = [Axis::Width, Axis::Height];

#[test]
fn axis_names_the_call() {
    assert_eq!(Axis::Width.function(), WIDTH_FUNCTION);
    assert_eq!(Axis::Height.function(), HEIGHT_FUNCTION);
    assert_eq!(WIDTH_FUNCTION, c"obs_source_get_width");
    assert_eq!(HEIGHT_FUNCTION, c"obs_source_get_height");
    assert_eq!(SOURCE, c"source");
}

#[test]
fn null_source_logs_its_function_and_answers_zero() {
    for axis in AXES {
        let mut m = Model::new(vec![]);
        assert_eq!(get(&mut m, None, axis), 0);
        assert_eq!(m.events, [Event::Log(axis.function())]);
    }
}

#[test]
fn no_data_answers_zero_silently() {
    for kind in [INPUT, TYPE_FILTER, TYPE_TRANSITION, SCENE] {
        for axis in AXES {
            let mut n = node(kind, &[1], [7, 9]);
            n.data = false;
            let mut m = Model::new(vec![n, node(TYPE_FILTER, &[], [1, 2])]);
            assert_eq!(get(&mut m, Some(0), axis), 0);
            assert!(m.events.is_empty(), "{:?}", m.events);
        }
    }
}

#[test]
fn filter_answers_its_own_base_without_a_lock() {
    for axis in AXES {
        // A filter's own filter list is never read.
        let mut m = Model::new(vec![
            node(TYPE_FILTER, &[1], [640, 360]),
            node(TYPE_FILTER, &[], [1, 2]),
        ]);
        let want = if axis == Axis::Width { 640 } else { 360 };
        assert_eq!(get(&mut m, Some(0), axis), want);
        assert_eq!(m.events, [Event::Base(0, axis)]);
    }
}

#[test]
fn unfiltered_source_answers_its_base_under_its_lock() {
    for kind in [INPUT, TYPE_TRANSITION, SCENE, 4, -1] {
        for axis in AXES {
            let mut m = Model::new(vec![node(kind, &[], [1920, 1080])]);
            let want = if axis == Axis::Width { 1920 } else { 1080 };
            assert_eq!(get(&mut m, Some(0), axis), want);
            assert_eq!(
                m.events,
                [Event::Lock(0), Event::Base(0, axis), Event::Unlock(0)]
            );
            assert!(m.locked.is_empty());
        }
    }
}

#[test]
fn filtered_source_starts_at_its_first_filter() {
    for kind in [INPUT, TYPE_TRANSITION, SCENE] {
        for axis in AXES {
            let mut m = Model::new(vec![
                node(kind, &[2, 1], [1920, 1080]),
                node(TYPE_FILTER, &[], [11, 12]),
                node(TYPE_FILTER, &[], [1280, 720]),
            ]);
            let want = if axis == Axis::Width { 1280 } else { 720 };
            assert_eq!(get(&mut m, Some(0), axis), want);
            assert_eq!(
                m.events,
                [
                    Event::Lock(0),
                    Event::FirstFilter(0),
                    Event::Base(2, axis),
                    Event::Unlock(0)
                ]
            );
        }
    }
}

#[test]
fn zero_base_size_passes_through() {
    for axis in AXES {
        let mut m = Model::new(vec![node(INPUT, &[], [0, 0])]);
        assert_eq!(get(&mut m, Some(0), axis), 0);
        assert_eq!(
            m.events,
            [Event::Lock(0), Event::Base(0, axis), Event::Unlock(0)]
        );
    }
}

#[test]
fn every_u32_passes_through() {
    for size in [1, 0x7fff_ffff, 0x8000_0000, u32::MAX] {
        let mut m = Model::new(vec![node(INPUT, &[], [size, size ^ 1])]);
        assert_eq!(get(&mut m, Some(0), Axis::Width), size);
        assert_eq!(get(&mut m, Some(0), Axis::Height), size ^ 1);
    }
}
