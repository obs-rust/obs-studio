//! Tier 1 mirrors of test_source_active_child.c. The C the core reaches is
//! modelled here: nodes listing children through the full-tree walk (with
//! the C's stop source), `[show_refs, activate_refs]` per node, and
//! `obs_source_activate()` / `obs_source_deactivate()` on them, recording
//! the counters each time the activation's tree walk runs (once per counter
//! it changes), as the node source type of the C test does.
use core::ffi::{CStr, c_long};
use obs_source::active_child::{
    ADD_FUNCTION, AUX_VIEW, CHILD, Libobs, MAIN_VIEW, PARENT, REMOVE_FUNCTION, SELF_WARNING, add,
    names, remove, view_type,
};

const NODES: usize = 5;

/// One C call or log line, in call order.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Walk { root: usize, stop: usize },
    ShowRefs(usize),
    ActivateRefs(usize),
    Activate(usize, u32),
    Deactivate(usize, u32),
    Log(&'static CStr, &'static CStr),
    Warn,
}

#[derive(Default)]
struct Model {
    children: [Vec<usize>; NODES],
    report_as: [Option<usize>; NODES],
    walks: [u32; NODES],
    refs: [[c_long; 2]; NODES],
    records: [Vec<[c_long; 2]>; NODES],
    events: Vec<Event>,
}

impl Model {
    fn list(&mut self, parent: usize, child: usize) {
        self.children[parent].push(child);
    }

    /// `enum_source_full_tree_callback()` for one edge.
    fn edge(
        &mut self,
        parent: usize,
        child: usize,
        stop: usize,
        visit: &mut dyn FnMut(Option<usize>, Option<usize>),
    ) {
        if child == stop {
            visit(Some(parent), Some(child));
            return;
        }
        self.descend(child, stop, visit);
        visit(Some(parent), Some(child));
    }

    /// A node's enum_all_sources: one walk, one edge per child.
    fn descend(
        &mut self,
        node: usize,
        stop: usize,
        visit: &mut dyn FnMut(Option<usize>, Option<usize>),
    ) {
        self.walks[node] += 1;
        let reported = self.report_as[node].unwrap_or(node);
        for child in self.children[node].clone() {
            self.edge(reported, child, stop, visit);
        }
    }

    /// The node's enum_active_sources: records its counters.
    fn record(&mut self, node: usize) {
        self.records[node].push(self.refs[node]);
    }
}

impl Libobs for Model {
    type Source = usize;

    fn enum_full_tree(
        &mut self,
        root: usize,
        stop: usize,
        visit: &mut dyn FnMut(Option<usize>, Option<usize>),
    ) {
        self.events.push(Event::Walk { root, stop });
        self.descend(root, stop, visit);
    }

    fn show_refs(&mut self, source: usize) -> c_long {
        self.events.push(Event::ShowRefs(source));
        self.refs[source][0]
    }

    fn activate_refs(&mut self, source: usize) -> c_long {
        self.events.push(Event::ActivateRefs(source));
        self.refs[source][1]
    }

    fn activate(&mut self, source: usize, view_type: u32) {
        self.events.push(Event::Activate(source, view_type));
        self.refs[source][0] += 1;
        self.record(source);
        if view_type == MAIN_VIEW {
            self.refs[source][1] += 1;
            self.record(source);
        }
    }

    fn deactivate(&mut self, source: usize, view_type: u32) {
        self.events.push(Event::Deactivate(source, view_type));
        if self.refs[source][0] > 0 {
            self.refs[source][0] -= 1;
            self.record(source);
        }
        if view_type == MAIN_VIEW && self.refs[source][1] > 0 {
            self.refs[source][1] -= 1;
            self.record(source);
        }
    }

    fn log_null(&mut self, function: &'static CStr, parameter: &'static CStr) {
        self.events.push(Event::Log(function, parameter));
    }

    fn warn_self(&mut self) {
        self.events.push(Event::Warn);
    }
}

fn logs(model: &Model) -> Vec<Event> {
    model
        .events
        .iter()
        .filter(|e| matches!(e, Event::Log(..) | Event::Warn))
        .cloned()
        .collect()
}

#[test]
fn view_type_is_main_below_activate_refs() {
    assert_eq!(view_type(0, 1), MAIN_VIEW);
    assert_eq!(view_type(1, 1), AUX_VIEW);
    assert_eq!(view_type(4, 5), MAIN_VIEW);
    assert_eq!(view_type(0, 0), AUX_VIEW);
    assert_eq!(view_type(0, -1), AUX_VIEW);
    assert_eq!(view_type(c_long::MAX - 1, c_long::MAX), MAIN_VIEW);
}

#[test]
fn an_edge_names_its_target_as_parent_or_child() {
    assert!(names(Some(1), Some(2), 1));
    assert!(names(Some(1), Some(2), 2));
    assert!(!names(Some(1), Some(2), 3));
    assert!(names(None, Some(3), 3));
    assert!(names(Some(3), None, 3));
    assert!(!names(None::<usize>, None, 3));
}

#[test]
fn null_parent_or_child_logs_once() {
    for (parent, child, which) in [
        (None, Some(1), PARENT),
        (Some(0), None, CHILD),
        (None, None, PARENT),
    ] {
        let mut model = Model::default();
        model.refs[0] = [2, 1];
        assert!(!add(&mut model, parent, child));
        assert_eq!(model.events, [Event::Log(ADD_FUNCTION, which)]);

        let mut model = Model::default();
        model.refs[0] = [2, 1];
        remove(&mut model, parent, child);
        assert_eq!(model.events, [Event::Log(REMOVE_FUNCTION, which)]);
        assert_eq!(model.refs[1], [0, 0]);
    }
    assert_eq!(
        SELF_WARNING.to_str().unwrap(),
        "obs_source_add_active_child: parent == child"
    );
}

#[test]
fn add_parent_equals_child_warns() {
    let mut model = Model::default();
    model.refs[0] = [2, 1];
    model.list(0, 1);
    assert!(!add(&mut model, Some(0), Some(0)));
    assert_eq!(model.events, [Event::Warn]);
    assert_eq!(model.walks, [0; NODES]);
    assert_eq!(model.refs[0], [2, 1]);
}

#[test]
fn remove_parent_equals_child_deactivates_itself() {
    let mut model = Model::default();
    model.refs[0] = [2, 1];
    remove(&mut model, Some(0), Some(0));
    assert!(logs(&model).is_empty());
    assert_eq!(model.records[0], [[1, 1], [1, 0], [0, 0]]);
    assert_eq!(model.walks, [0; NODES]);
    assert_eq!(model.refs[0], [0, 0]);
}

/// Adds `child` under `parent` (counters 3 and 2) and checks it is refused
/// after exactly one walk of the child with the parent as the stop source,
/// with nothing loaded, activated or logged.
fn refuse(model: &mut Model, parent: usize, child: usize) {
    model.refs[parent] = [3, 2];
    assert!(!add(model, Some(parent), Some(child)));
    assert_eq!(
        model.events,
        [Event::Walk {
            root: child,
            stop: parent
        }]
    );
    assert!(model.records.iter().all(Vec::is_empty));
    for (i, refs) in model.refs.iter().enumerate() {
        if i != parent {
            assert_eq!(*refs, [0, 0]);
        }
    }
    assert_eq!(model.refs[parent], [3, 2]);
}

#[test]
fn parent_listed_by_child_is_refused() {
    let mut model = Model::default();
    model.list(1, 0);
    refuse(&mut model, 0, 1);
    assert_eq!(model.walks[1], 1);
    assert_eq!(model.walks[0], 0);
}

#[test]
fn parent_deep_in_child_tree_is_refused() {
    let mut model = Model::default();
    model.list(1, 2);
    model.list(2, 3);
    model.list(3, 0);
    refuse(&mut model, 0, 1);
    assert_eq!(model.walks, [0, 1, 1, 1, 0]);
}

#[test]
fn parent_reported_as_edge_parent_is_refused() {
    let mut model = Model::default();
    model.list(1, 2);
    model.list(2, 4);
    model.report_as[2] = Some(0);
    refuse(&mut model, 0, 1);
    assert_eq!(model.walks[2], 1);
    assert_eq!(model.walks[4], 1);
}

#[test]
fn real_cycle_is_refused_without_walking_parent() {
    let mut model = Model::default();
    model.list(0, 1);
    model.list(1, 0);
    refuse(&mut model, 0, 1);
    assert_eq!(model.walks[1], 1);
    assert_eq!(model.walks[0], 0);
}

#[test]
fn unrelated_tree_is_accepted() {
    let mut model = Model::default();
    model.list(1, 2);
    model.list(2, 3);
    model.refs[0] = [1, 1];
    assert!(add(&mut model, Some(0), Some(1)));
    assert!(logs(&model).is_empty());
    assert_eq!(model.walks, [0, 1, 1, 1, 0]);
    assert_eq!(model.records[1], [[1, 0], [1, 1]]);
    assert_eq!(model.refs[1], [1, 1]);
    assert_eq!(model.refs[2], [0, 0]);
    // The walk, then each counter loaded once, show before active, then the
    // activations.
    assert_eq!(
        model.events,
        [
            Event::Walk { root: 1, stop: 0 },
            Event::ShowRefs(0),
            Event::ActivateRefs(0),
            Event::Activate(1, MAIN_VIEW),
        ]
    );
}

#[test]
fn hidden_parent_accepts_without_activating() {
    let mut model = Model::default();
    assert!(add(&mut model, Some(0), Some(1)));
    assert_eq!(model.walks[1], 1);
    assert!(model.records[1].is_empty());
    assert_eq!(model.refs[1], [0, 0]);
}

#[test]
fn add_activates_main_then_aux() {
    let mut model = Model::default();
    model.refs[0] = [3, 1];
    assert!(add(&mut model, Some(0), Some(1)));
    assert_eq!(model.records[1], [[1, 0], [1, 1], [2, 1], [3, 1]]);
    assert_eq!(model.refs[1], [3, 1]);
    assert_eq!(model.refs[0], [3, 1]);
    assert_eq!(
        model.events[3..],
        [
            Event::Activate(1, MAIN_VIEW),
            Event::Activate(1, AUX_VIEW),
            Event::Activate(1, AUX_VIEW),
        ]
    );
}

#[test]
fn add_with_more_active_than_shown_is_all_main() {
    let mut model = Model::default();
    model.refs[0] = [2, 5];
    assert!(add(&mut model, Some(0), Some(1)));
    assert_eq!(model.records[1], [[1, 0], [1, 1], [2, 1], [2, 2]]);
    assert_eq!(model.refs[1], [2, 2]);
}

#[test]
fn add_with_negative_counts() {
    let mut model = Model::default();
    model.refs[0] = [-1, 3];
    assert!(add(&mut model, Some(0), Some(1)));
    assert!(model.records[1].is_empty());
    assert_eq!(model.refs[1], [0, 0]);

    model.refs[0] = [2, -1];
    assert!(add(&mut model, Some(0), Some(2)));
    assert_eq!(model.records[2], [[1, 0], [2, 0]]);
    assert_eq!(model.refs[2], [2, 0]);
}

#[test]
fn add_twice_activates_twice() {
    let mut model = Model::default();
    model.refs[0] = [1, 1];
    assert!(add(&mut model, Some(0), Some(1)));
    assert!(add(&mut model, Some(0), Some(1)));
    assert_eq!(model.walks[1], 2);
    assert_eq!(model.refs[1], [2, 2]);
}

#[test]
fn remove_deactivates_main_then_aux() {
    let mut model = Model::default();
    model.list(1, 0);
    model.refs[0] = [3, 1];
    model.refs[1] = [3, 1];
    remove(&mut model, Some(0), Some(1));
    assert!(logs(&model).is_empty());
    assert_eq!(model.records[1], [[2, 1], [2, 0], [1, 0], [0, 0]]);
    assert_eq!(model.walks, [0; NODES]);
    assert_eq!(model.refs[1], [0, 0]);
    assert_eq!(model.refs[0], [3, 1]);
    assert_eq!(
        model.events,
        [
            Event::ShowRefs(0),
            Event::ActivateRefs(0),
            Event::Deactivate(1, MAIN_VIEW),
            Event::Deactivate(1, AUX_VIEW),
            Event::Deactivate(1, AUX_VIEW),
        ]
    );
}

#[test]
fn add_then_remove_balances() {
    let mut model = Model::default();
    model.refs[0] = [4, 2];
    assert!(add(&mut model, Some(0), Some(1)));
    assert_eq!(model.refs[1], [4, 2]);
    remove(&mut model, Some(0), Some(1));
    assert_eq!(model.refs[1], [0, 0]);
}

#[test]
fn remove_from_inactive_child_stays_at_zero() {
    let mut model = Model::default();
    model.refs[0] = [2, 2];
    remove(&mut model, Some(0), Some(1));
    assert!(model.records[1].is_empty());
    assert_eq!(model.refs[1], [0, 0]);

    model.refs[1] = [1, 0];
    remove(&mut model, Some(0), Some(1));
    assert_eq!(model.records[1], [[0, 0]]);
    assert_eq!(model.refs[1], [0, 0]);
}

#[test]
fn remove_with_hidden_parent_does_nothing() {
    let mut model = Model::default();
    model.refs[0] = [0, 3];
    model.refs[1] = [2, 2];
    remove(&mut model, Some(0), Some(1));
    assert!(model.records[1].is_empty());
    assert_eq!(model.refs[1], [2, 2]);
    assert_eq!(model.events, [Event::ShowRefs(0), Event::ActivateRefs(0)]);
}
