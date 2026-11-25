//! Tier 1 mirrors of test_source_enum_active.c, plus the transition, dying
//! and data-less arms the C test reaches only through the parity oracle.
//! The C the core reaches is modelled here: nodes whose
//! `enum_active_sources` lists children, transitions whose two slots
//! `obs_transition_enum_sources()` reports, and the reference calls. The
//! caller's callback and the tree walk's are the model's two sinks; the
//! tree sink runs [`tree_edge`] and then reports the edge, as
//! `enum_source_active_tree_callback()` does.
use core::ffi::CStr;
use obs_source::enum_active::{Libobs, SOURCES_FUNCTION, TREE_FUNCTION, enumerate, tree_edge};

const NODES: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sink {
    User,
    Tree,
}

/// One C call, report or log line, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    GetRef(usize),
    Release(usize),
    Transition(usize, Sink),
    EnumActive(usize, Sink),
    Edge(usize, usize),
    Log(&'static CStr),
}

struct Model {
    children: [Vec<usize>; NODES],
    slots: [Vec<usize>; NODES],
    transition: [bool; NODES],
    enum_active: [bool; NODES],
    data: [bool; NODES],
    dying: [bool; NODES],
    events: Vec<Event>,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            children: Default::default(),
            slots: Default::default(),
            transition: [false; NODES],
            enum_active: [true; NODES],
            data: [true; NODES],
            dying: [false; NODES],
            events: Vec::new(),
        }
    }
}

impl Model {
    fn report(&mut self, parent: usize, child: usize, sink: Sink) {
        if sink == Sink::Tree {
            tree_edge(self, child, Sink::Tree);
        }
        self.events.push(Event::Edge(parent, child));
    }

    fn sources(&mut self, source: Option<usize>) {
        enumerate(self, SOURCES_FUNCTION, source, Sink::User);
    }

    fn tree(&mut self, source: Option<usize>) {
        enumerate(self, TREE_FUNCTION, source, Sink::Tree);
    }

    fn edges(&self) -> Vec<(usize, usize)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                Event::Edge(p, c) => Some((*p, *c)),
                _ => None,
            })
            .collect()
    }
}

impl Libobs for Model {
    type Source = usize;
    type Sink = Sink;

    fn has_data(&mut self, source: usize) -> bool {
        self.data[source]
    }

    fn is_transition(&mut self, source: usize) -> bool {
        self.transition[source]
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

    fn transition_enum(&mut self, source: usize, sink: Sink) {
        self.events.push(Event::Transition(source, sink));
        for slot in self.slots[source].clone() {
            self.report(source, slot, sink);
        }
    }

    fn enum_active(&mut self, source: usize, sink: Sink) {
        assert!(self.enum_active[source], "called a missing callback");
        self.events.push(Event::EnumActive(source, sink));
        for child in self.children[source].clone() {
            self.report(source, child, sink);
        }
    }

    fn log_null(&mut self, function: &'static CStr) {
        self.events.push(Event::Log(function));
    }
}

const LEAF: usize = 4;

/// a(0) -> b(1) -> c(2), a -> leaf, b -> d(3); the leaf has no callback.
fn tree_model() -> Model {
    let mut m = Model::default();
    m.enum_active[LEAF] = false;
    m.children[0] = vec![1, LEAF];
    m.children[1] = vec![2, 3];
    m
}

#[test]
fn null_source_logs_once() {
    let mut m = Model::default();
    m.sources(None);
    m.tree(None);
    assert_eq!(
        m.events,
        [Event::Log(SOURCES_FUNCTION), Event::Log(TREE_FUNCTION)]
    );
}

#[test]
fn source_without_callback_does_nothing() {
    let mut m = tree_model();
    m.sources(Some(LEAF));
    m.tree(Some(LEAF));
    assert!(m.events.is_empty());
}

#[test]
fn source_without_data_does_nothing() {
    let mut m = tree_model();
    m.data[0] = false;
    m.sources(Some(0));
    m.tree(Some(0));
    assert!(m.events.is_empty());
}

#[test]
fn node_without_children_calls_plugin_once() {
    let mut m = Model::default();
    m.sources(Some(0));
    assert_eq!(
        m.events,
        [
            Event::GetRef(0),
            Event::EnumActive(0, Sink::User),
            Event::Release(0)
        ]
    );
}

#[test]
fn sources_reports_direct_children_only() {
    let mut m = tree_model();
    m.sources(Some(0));
    assert_eq!(m.edges(), [(0, 1), (0, LEAF)]);
    assert!(!m.events.contains(&Event::EnumActive(1, Sink::User)));
}

#[test]
fn tree_is_depth_first_post_order() {
    let mut m = tree_model();
    m.tree(Some(0));
    assert_eq!(
        m.events,
        [
            Event::GetRef(0),
            Event::EnumActive(0, Sink::Tree),
            Event::EnumActive(1, Sink::Tree),
            Event::EnumActive(2, Sink::Tree),
            Event::Edge(1, 2),
            Event::EnumActive(3, Sink::Tree),
            Event::Edge(1, 3),
            Event::Edge(0, 1),
            Event::Edge(0, LEAF),
            Event::Release(0),
        ]
    );
}

#[test]
fn shared_child_is_reported_per_edge() {
    let mut m = Model::default();
    m.children[0] = vec![1, 2];
    m.children[1] = vec![3];
    m.children[2] = vec![3];
    m.tree(Some(0));
    assert_eq!(m.edges(), [(1, 3), (0, 1), (2, 3), (0, 2)]);
}

#[test]
fn tree_from_inner_node() {
    let mut m = tree_model();
    m.tree(Some(1));
    assert_eq!(m.edges(), [(1, 2), (1, 3)]);
}

#[test]
fn child_without_data_is_reported_but_not_walked() {
    let mut m = tree_model();
    m.data[1] = false;
    m.tree(Some(0));
    assert_eq!(m.edges(), [(0, 1), (0, LEAF)]);
    assert!(!m.events.contains(&Event::EnumActive(1, Sink::Tree)));
}

#[test]
fn dying_source_logs_after_taking_no_reference() {
    let mut m = tree_model();
    m.dying[0] = true;
    m.sources(Some(0));
    m.tree(Some(0));
    assert_eq!(
        m.events,
        [
            Event::GetRef(0),
            Event::Log(SOURCES_FUNCTION),
            Event::GetRef(0),
            Event::Log(TREE_FUNCTION),
        ]
    );
}

#[test]
fn transition_reports_slots_before_its_own_children() {
    let mut m = Model::default();
    m.transition[0] = true;
    m.slots[0] = vec![1, 2];
    m.children[0] = vec![3];
    m.sources(Some(0));
    assert_eq!(
        m.events,
        [
            Event::GetRef(0),
            Event::Transition(0, Sink::User),
            Event::Edge(0, 1),
            Event::Edge(0, 2),
            Event::EnumActive(0, Sink::User),
            Event::Edge(0, 3),
            Event::Release(0),
        ]
    );
}

#[test]
fn transition_without_callback_still_reports_slots() {
    let mut m = Model::default();
    m.transition[0] = true;
    m.enum_active[0] = false;
    m.slots[0] = vec![1];
    m.sources(Some(0));
    assert_eq!(
        m.events,
        [
            Event::GetRef(0),
            Event::Transition(0, Sink::User),
            Event::Edge(0, 1),
            Event::Release(0),
        ]
    );
}

#[test]
fn tree_descends_into_transition_children() {
    // a -> t(1, transition, slots 2 and 3, no callback); 2 -> 5.
    let mut m = Model::default();
    m.children[0] = vec![1];
    m.transition[1] = true;
    m.enum_active[1] = false;
    m.slots[1] = vec![2, 3];
    m.children[2] = vec![5];
    m.tree(Some(0));
    assert_eq!(
        m.events,
        [
            Event::GetRef(0),
            Event::EnumActive(0, Sink::Tree),
            Event::Transition(1, Sink::Tree),
            Event::EnumActive(2, Sink::Tree),
            Event::EnumActive(5, Sink::Tree),
            Event::Edge(2, 5),
            Event::Edge(1, 2),
            Event::EnumActive(3, Sink::Tree),
            Event::Edge(1, 3),
            Event::Edge(0, 1),
            Event::Release(0),
        ]
    );
}

#[test]
fn transition_child_without_data_still_reports_slots() {
    let mut m = Model::default();
    m.children[0] = vec![1];
    m.transition[1] = true;
    m.data[1] = false;
    m.slots[1] = vec![2];
    m.tree(Some(0));
    assert_eq!(m.edges(), [(1, 2), (0, 1)]);
    assert!(m.events.contains(&Event::Transition(1, Sink::Tree)));
    assert!(!m.events.contains(&Event::EnumActive(1, Sink::Tree)));
}
