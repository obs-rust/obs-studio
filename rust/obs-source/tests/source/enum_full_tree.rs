//! Tier 1 mirrors of test_source_enum_full_tree.c, plus the transition,
//! dying and NULL-child arms the C test reaches only through the parity
//! oracle. The C the core reaches is modelled here: nodes whose
//! `enum_all_sources` and `enum_active_sources` list children, transitions
//! whose two slots `obs_transition_enum_sources()` reports, and the
//! reference calls. The walk's callback is the model's `report`: it runs
//! [`tree_edge`] for a child that [`descends`] and then reports the edge, as
//! `enum_source_full_tree_callback()` does.
use core::ffi::CStr;
use obs_source::enum_full_tree::{FUNCTION, Libobs, descends, enumerate, tree_edge};

const NODES: usize = 7;

/// One C call, report or log line, in order. A `None` child is NULL.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    GetRef(usize),
    Release(usize),
    Transition(usize),
    EnumAll(usize),
    EnumActive(usize),
    Edge(usize, Option<usize>),
    Log(&'static CStr),
}

struct Model {
    all: [Vec<Option<usize>>; NODES],
    active: [Vec<Option<usize>>; NODES],
    slots: [Vec<Option<usize>>; NODES],
    transition: [bool; NODES],
    enum_all: [bool; NODES],
    enum_active: [bool; NODES],
    data: [bool; NODES],
    dying: [bool; NODES],
    stop: Option<usize>,
    events: Vec<Event>,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            all: Default::default(),
            active: Default::default(),
            slots: Default::default(),
            transition: [false; NODES],
            enum_all: [true; NODES],
            enum_active: [true; NODES],
            data: [true; NODES],
            dying: [false; NODES],
            stop: None,
            events: Vec::new(),
        }
    }
}

impl Model {
    fn report(&mut self, parent: usize, child: Option<usize>) {
        if descends(child, self.stop)
            && let Some(child) = child
        {
            tree_edge(self, child, ());
        }
        self.events.push(Event::Edge(parent, child));
    }

    fn walk(&mut self, source: Option<usize>) {
        enumerate(self, source, ());
    }

    fn edges(&self) -> Vec<(usize, Option<usize>)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                Event::Edge(p, c) => Some((*p, *c)),
                _ => None,
            })
            .collect()
    }

    fn calls(&self) -> Vec<Event> {
        self.events
            .iter()
            .filter(|e| !matches!(e, Event::Edge(..)))
            .cloned()
            .collect()
    }
}

impl Libobs for Model {
    type Source = usize;
    type Sink = ();

    fn has_data(&mut self, source: usize) -> bool {
        self.data[source]
    }

    fn is_transition(&mut self, source: usize) -> bool {
        self.transition[source]
    }

    fn has_enum_all(&mut self, source: usize) -> bool {
        self.enum_all[source]
    }

    fn has_enum_active(&mut self, source: usize) -> bool {
        self.enum_active[source]
    }

    fn get_ref(&mut self, source: usize) -> Option<usize> {
        self.events.push(Event::GetRef(source));
        (!self.dying[source]).then_some(source)
    }

    fn release(&mut self, source: usize) {
        self.events.push(Event::Release(source));
    }

    fn transition_enum(&mut self, source: usize, _sink: ()) {
        self.events.push(Event::Transition(source));
        for slot in self.slots[source].clone() {
            self.report(source, slot);
        }
    }

    fn enum_all(&mut self, source: usize, _sink: ()) {
        assert!(self.enum_all[source], "called a missing enum_all_sources");
        assert!(self.data[source], "called a plugin without data");
        self.events.push(Event::EnumAll(source));
        for child in self.all[source].clone() {
            self.report(source, child);
        }
    }

    fn enum_active(&mut self, source: usize, _sink: ()) {
        assert!(
            self.enum_active[source],
            "called a missing enum_active_sources"
        );
        assert!(self.data[source], "called a plugin without data");
        self.events.push(Event::EnumActive(source));
        for child in self.active[source].clone() {
            self.report(source, child);
        }
    }

    fn log_null(&mut self, function: &'static CStr) {
        self.events.push(Event::Log(function));
    }
}

const LEAF: usize = 5;
const TR: usize = 6;

fn model() -> Model {
    let mut m = Model::default();
    m.enum_all[LEAF] = false;
    m.enum_active[LEAF] = false;
    m.transition[TR] = true;
    m.enum_all[TR] = false;
    m.enum_active[TR] = false;
    m
}

#[test]
fn null_source_logs_once() {
    let mut m = model();
    m.walk(None);
    assert_eq!(m.events, vec![Event::Log(FUNCTION)]);
    assert_eq!(FUNCTION, c"obs_source_enum_full_tree");
}

#[test]
fn root_without_data_is_silent() {
    let mut m = model();
    m.data[0] = false;
    m.all[0] = vec![Some(1)];
    m.walk(Some(0));
    assert_eq!(m.events, vec![]);
}

#[test]
fn root_without_callbacks_takes_no_reference() {
    let mut m = model();
    m.walk(Some(LEAF));
    assert_eq!(m.events, vec![]);
}

#[test]
fn dying_root_logs_and_returns() {
    let mut m = model();
    m.dying[0] = true;
    m.all[0] = vec![Some(1)];
    m.walk(Some(0));
    assert_eq!(m.events, vec![Event::GetRef(0), Event::Log(FUNCTION)]);
}

#[test]
fn root_prefers_enum_all_sources() {
    let mut m = model();
    m.all[0] = vec![Some(1)];
    m.active[0] = vec![Some(2)];
    m.walk(Some(0));
    assert_eq!(
        m.calls(),
        vec![
            Event::GetRef(0),
            Event::EnumAll(0),
            Event::EnumAll(1),
            Event::Release(0)
        ]
    );
    assert_eq!(m.edges(), vec![(0, Some(1))]);
}

#[test]
fn root_falls_back_to_enum_active_sources() {
    let mut m = model();
    m.enum_all[0] = false;
    m.active[0] = vec![Some(1)];
    m.walk(Some(0));
    assert_eq!(
        m.calls(),
        vec![
            Event::GetRef(0),
            Event::EnumActive(0),
            Event::EnumAll(1),
            Event::Release(0)
        ]
    );
    assert_eq!(m.edges(), vec![(0, Some(1))]);
}

#[test]
fn transition_root_reports_slots_then_its_callback() {
    let mut m = model();
    m.slots[TR] = vec![Some(1), Some(2)];
    m.walk(Some(TR));
    assert_eq!(
        m.calls(),
        vec![
            Event::GetRef(TR),
            Event::Transition(TR),
            Event::EnumAll(1),
            Event::EnumAll(2),
            Event::Release(TR)
        ]
    );
    assert_eq!(m.edges(), vec![(TR, Some(1)), (TR, Some(2))]);

    let mut m = model();
    m.enum_all[TR] = true;
    m.slots[TR] = vec![Some(1)];
    m.all[TR] = vec![Some(2)];
    m.walk(Some(TR));
    assert_eq!(
        m.calls(),
        vec![
            Event::GetRef(TR),
            Event::Transition(TR),
            Event::EnumAll(1),
            Event::EnumAll(TR),
            Event::EnumAll(2),
            Event::Release(TR)
        ]
    );
}

/// a(0) -> b(1) -> c(2), b -> leaf, a -> d(3)
#[test]
fn tree_is_depth_first_post_order() {
    let mut m = model();
    m.all[0] = vec![Some(1), Some(3)];
    m.all[1] = vec![Some(2), Some(LEAF)];
    m.walk(Some(0));
    assert_eq!(
        m.edges(),
        vec![(1, Some(2)), (1, Some(LEAF)), (0, Some(1)), (0, Some(3))]
    );
}

#[test]
fn shared_child_is_walked_per_edge() {
    let mut m = model();
    m.all[0] = vec![Some(1), Some(2)];
    m.all[1] = vec![Some(3)];
    m.all[2] = vec![Some(3)];
    m.walk(Some(0));
    assert_eq!(
        m.edges(),
        vec![(1, Some(3)), (0, Some(1)), (2, Some(3)), (0, Some(2))]
    );
    let walks = m.events.iter().filter(|e| **e == Event::EnumAll(3)).count();
    assert_eq!(walks, 2);
}

#[test]
fn child_prefers_enum_all_and_falls_back() {
    let mut m = model();
    m.all[0] = vec![Some(1), Some(2)];
    m.all[1] = vec![Some(3)];
    m.active[1] = vec![Some(4)];
    m.enum_all[2] = false;
    m.active[2] = vec![Some(4)];
    m.walk(Some(0));
    assert_eq!(
        m.edges(),
        vec![(1, Some(3)), (0, Some(1)), (2, Some(4)), (0, Some(2))]
    );
    assert!(!m.events.contains(&Event::EnumActive(1)));
    assert!(m.events.contains(&Event::EnumActive(2)));
}

#[test]
fn child_without_data_is_reported_not_walked() {
    let mut m = model();
    m.all[0] = vec![Some(1)];
    m.data[1] = false;
    m.all[1] = vec![Some(2)];
    m.walk(Some(0));
    assert_eq!(m.edges(), vec![(0, Some(1))]);
    assert!(!m.events.contains(&Event::EnumAll(1)));
}

#[test]
fn transition_child_reports_slots_even_without_data() {
    let mut m = model();
    m.all[0] = vec![Some(TR)];
    m.slots[TR] = vec![Some(1), None];
    m.data[TR] = false;
    m.enum_all[TR] = true;
    m.walk(Some(0));
    assert_eq!(
        m.calls(),
        vec![
            Event::GetRef(0),
            Event::EnumAll(0),
            Event::Transition(TR),
            Event::EnumAll(1),
            Event::Release(0)
        ]
    );
    assert_eq!(m.edges(), vec![(TR, Some(1)), (TR, None), (0, Some(TR))]);
}

#[test]
fn child_without_callbacks_is_reported_not_walked() {
    let mut m = model();
    m.all[0] = vec![Some(LEAF)];
    m.walk(Some(0));
    assert_eq!(
        m.calls(),
        vec![Event::GetRef(0), Event::EnumAll(0), Event::Release(0)]
    );
    assert_eq!(m.edges(), vec![(0, Some(LEAF))]);
}

/// b(1) lists a(0), a lists c(2): with a as the stop source, a's edge is
/// reported and a is never walked, wherever it turns up.
#[test]
fn stop_source_is_reported_not_walked() {
    let mut m = model();
    m.stop = Some(0);
    m.all[1] = vec![Some(3), Some(0)];
    m.all[3] = vec![Some(0)];
    m.all[0] = vec![Some(2)];
    m.walk(Some(1));
    assert_eq!(m.edges(), vec![(3, Some(0)), (1, Some(3)), (1, Some(0))]);
    assert!(!m.events.contains(&Event::EnumAll(0)));

    m.stop = None;
    m.events.clear();
    m.walk(Some(1));
    assert_eq!(
        m.edges(),
        vec![
            (0, Some(2)),
            (3, Some(0)),
            (1, Some(3)),
            (0, Some(2)),
            (1, Some(0))
        ]
    );
}

#[test]
fn null_child_is_reported_without_a_walk() {
    let mut m = model();
    m.all[0] = vec![None, Some(1)];
    m.walk(Some(0));
    assert_eq!(m.edges(), vec![(0, None), (0, Some(1))]);
}

#[test]
fn descends_into_every_child_but_the_stop_source() {
    assert!(descends(Some(1), None));
    assert!(descends(Some(1), Some(2)));
    assert!(!descends(Some(1), Some(1)));
    assert!(!descends::<usize>(None, None));
}
