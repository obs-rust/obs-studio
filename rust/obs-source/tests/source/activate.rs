//! Tier 1 mirrors of test_source_activate.c, plus the NULL source the C test
//! can't reach (every public caller checks first). The model is the C test's
//! tree (a lists b and d, b lists c) with the counters of each node; its walk
//! reports the children post-order, as `obs_source_enum_active_tree()`
//! does, and records every step with all eight counters at that moment.
use core::ffi::{CStr, c_long};
use obs_source::activate::{
    ACTIVATE_FUNCTION, AUX_VIEW, Counter, Counters, DEACTIVATE_FUNCTION, MAIN_VIEW, Tree, activate,
    deactivate, visit,
};

const A: usize = 0;
const B: usize = 1;
const C: usize = 2;
const D: usize = 3;
const NODES: usize = 4;

/// The children each node lists, as the C test's fixture.
fn children(node: usize) -> &'static [usize] {
    match node {
        A => &[B, D],
        B => &[C],
        _ => &[],
    }
}

/// `[show_refs; 4]`, `[activate_refs; 4]`.
type Refs = ([c_long; NODES], [c_long; NODES]);

#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    Inc(usize, Counter),
    Dec(usize, Counter),
    Load(usize, Counter),
    /// A walk from the root with its callback, and the counters it began on.
    Walk(usize, Tree, Refs),
    Log(&'static str),
}

#[derive(Default)]
struct Model {
    refs: Refs,
    steps: Vec<Step>,
}

impl Model {
    fn new(refs: Refs) -> Self {
        Self {
            refs,
            steps: Vec::new(),
        }
    }

    fn counter(&mut self, node: usize, counter: Counter) -> &mut c_long {
        match counter {
            Counter::Show => &mut self.refs.0[node],
            Counter::Activate => &mut self.refs.1[node],
        }
    }

    fn report(&mut self, node: usize, tree: Tree) {
        for &child in children(node) {
            self.report(child, tree);
            visit(self, child, tree);
        }
    }
}

impl Counters for Model {
    type Source = usize;
    fn inc(&mut self, source: usize, counter: Counter) {
        self.steps.push(Step::Inc(source, counter));
        *self.counter(source, counter) += 1;
    }
    fn dec(&mut self, source: usize, counter: Counter) {
        self.steps.push(Step::Dec(source, counter));
        *self.counter(source, counter) -= 1;
    }
    fn load(&mut self, source: usize, counter: Counter) -> c_long {
        self.steps.push(Step::Load(source, counter));
        *self.counter(source, counter)
    }
    fn walk(&mut self, source: usize, tree: Tree) {
        self.steps.push(Step::Walk(source, tree, self.refs));
        self.report(source, tree);
    }
    fn log_null(&mut self, function: &'static CStr) {
        self.steps.push(Step::Log(function.to_str().unwrap()));
    }
}

/// The steps a walk of `tree` from A makes on the children: each child's
/// counter, post-order.
fn walk_steps(tree: Tree) -> Vec<Step> {
    let counter = tree.counter();
    let each = |n| {
        if tree.raises() {
            Step::Inc(n, counter)
        } else {
            Step::Dec(n, counter)
        }
    };
    vec![each(C), each(B), each(D)]
}

fn run(call: fn(&mut Model, Option<usize>, u32), view_type: u32, refs: Refs) -> Model {
    let mut model = Model::new(refs);
    call(&mut model, Some(A), view_type);
    model
}

/// The NULL source the public callers never pass: one line naming the
/// entry, nothing else.
#[test]
fn null_source_logs_once_and_changes_nothing() {
    for view_type in [MAIN_VIEW, AUX_VIEW, 0, 7] {
        let mut model = Model::new(([3; NODES], [2; NODES]));
        activate(&mut model, None, view_type);
        deactivate(&mut model, None, view_type);
        assert_eq!(
            model.steps,
            [
                Step::Log("obs_source_activate"),
                Step::Log("obs_source_deactivate")
            ]
        );
        assert_eq!(model.refs, ([3; NODES], [2; NODES]));
    }
    assert_eq!(ACTIVATE_FUNCTION, c"obs_source_activate");
    assert_eq!(DEACTIVATE_FUNCTION, c"obs_source_deactivate");
}

/// aux_activate_raises_show_refs_in_one_walk.
#[test]
fn aux_activate_raises_show_refs_in_one_walk() {
    let model = run(activate, AUX_VIEW, Refs::default());
    let mut want = vec![
        Step::Inc(A, Counter::Show),
        Step::Walk(A, Tree::Show, ([1, 0, 0, 0], [0; NODES])),
    ];
    want.extend(walk_steps(Tree::Show));
    assert_eq!(model.steps, want);
    assert_eq!(model.refs, ([1; NODES], [0; NODES]));
}

/// main_activate_walks_show_then_active.
#[test]
fn main_activate_walks_show_then_active() {
    let model = run(activate, MAIN_VIEW, Refs::default());
    let mut want = vec![
        Step::Inc(A, Counter::Show),
        Step::Walk(A, Tree::Show, ([1, 0, 0, 0], [0; NODES])),
    ];
    want.extend(walk_steps(Tree::Show));
    want.push(Step::Inc(A, Counter::Activate));
    want.push(Step::Walk(A, Tree::Activate, ([1; NODES], [1, 0, 0, 0])));
    want.extend(walk_steps(Tree::Activate));
    assert_eq!(model.steps, want);
    assert_eq!(model.refs, ([1; NODES], [1; NODES]));
}

/// Any view type but MAIN_VIEW only shows.
#[test]
fn other_view_types_only_show() {
    for view_type in [0, AUX_VIEW, 3, u32::MAX] {
        let shown = run(activate, view_type, Refs::default());
        assert_eq!(shown.refs, ([1; NODES], [0; NODES]));
        let hidden = run(deactivate, view_type, ([1; NODES], [1; NODES]));
        assert_eq!(hidden.refs, ([0; NODES], [1; NODES]));
    }
}

/// activate_adds_one_to_any_count: activation never loads a counter.
#[test]
fn activate_adds_one_to_any_count() {
    let model = run(activate, MAIN_VIEW, ([-3, 0, 7, 2], [5, -1, 0, 9]));
    assert_eq!(model.refs, ([-2, 1, 8, 3], [6, 0, 1, 10]));
    assert!(!model.steps.iter().any(|s| matches!(s, Step::Load(..))));
    assert_eq!(
        model
            .steps
            .iter()
            .filter(|s| matches!(s, Step::Walk(..)))
            .count(),
        2
    );
}

/// aux_deactivate_lowers_show_refs_in_one_walk.
#[test]
fn aux_deactivate_lowers_show_refs_in_one_walk() {
    let model = run(deactivate, AUX_VIEW, ([2; NODES], [1; NODES]));
    let mut want = vec![
        Step::Load(A, Counter::Show),
        Step::Dec(A, Counter::Show),
        Step::Walk(A, Tree::Hide, ([1, 2, 2, 2], [1; NODES])),
    ];
    want.extend(walk_steps(Tree::Hide));
    assert_eq!(model.steps, want);
    assert_eq!(model.refs, ([1; NODES], [1; NODES]));
}

/// main_deactivate_walks_show_then_active.
#[test]
fn main_deactivate_walks_show_then_active() {
    let model = run(deactivate, MAIN_VIEW, ([1; NODES], [1; NODES]));
    let mut want = vec![
        Step::Load(A, Counter::Show),
        Step::Dec(A, Counter::Show),
        Step::Walk(A, Tree::Hide, ([0, 1, 1, 1], [1; NODES])),
    ];
    want.extend(walk_steps(Tree::Hide));
    want.extend([
        Step::Load(A, Counter::Activate),
        Step::Dec(A, Counter::Activate),
        Step::Walk(A, Tree::Deactivate, ([0; NODES], [0, 1, 1, 1])),
    ]);
    want.extend(walk_steps(Tree::Deactivate));
    assert_eq!(model.steps, want);
    assert_eq!(model.refs, ([0; NODES], [0; NODES]));
}

/// descendants_are_not_floor_checked.
#[test]
fn descendants_are_not_floor_checked() {
    let model = run(deactivate, MAIN_VIEW, ([1, 0, -4, 0], [1, 0, 0, 3]));
    assert_eq!(model.refs, ([0, -1, -5, -1], [0, -1, -1, 2]));
}

/// deactivate_at_zero_does_nothing: the counters are loaded, nothing else.
#[test]
fn deactivate_at_zero_does_nothing() {
    for refs in [([0, 3, 3, 3], [0, 3, 3, 3]), ([-2, 3, 3, 3], [-1, 3, 3, 3])] {
        let model = run(deactivate, MAIN_VIEW, refs);
        assert_eq!(
            model.steps,
            [
                Step::Load(A, Counter::Show),
                Step::Load(A, Counter::Activate)
            ]
        );
        assert_eq!(model.refs, refs);
        let model = run(deactivate, AUX_VIEW, refs);
        assert_eq!(model.steps, [Step::Load(A, Counter::Show)]);
        assert_eq!(model.refs, refs);
    }
}

/// hidden_but_active_root_walks_once.
#[test]
fn hidden_but_active_root_walks_once() {
    let model = run(deactivate, MAIN_VIEW, ([0, 2, 2, 2], [1, 2, 2, 2]));
    let mut want = vec![
        Step::Load(A, Counter::Show),
        Step::Load(A, Counter::Activate),
        Step::Dec(A, Counter::Activate),
        Step::Walk(A, Tree::Deactivate, ([0, 2, 2, 2], [0, 2, 2, 2])),
    ];
    want.extend(walk_steps(Tree::Deactivate));
    assert_eq!(model.steps, want);
    assert_eq!(model.refs, ([0, 2, 2, 2], [0, 1, 1, 1]));
}

/// aux_deactivate_ignores_activate_refs.
#[test]
fn aux_deactivate_ignores_activate_refs() {
    let model = run(deactivate, AUX_VIEW, ([0, 2, 2, 2], [4, 2, 2, 2]));
    assert_eq!(model.steps, [Step::Load(A, Counter::Show)]);
    assert_eq!(model.refs, ([0, 2, 2, 2], [4, 2, 2, 2]));
}

/// balanced_sequence_ends_at_zero.
#[test]
fn balanced_sequence_ends_at_zero() {
    let mut model = Model::default();
    activate(&mut model, Some(A), MAIN_VIEW);
    activate(&mut model, Some(A), AUX_VIEW);
    activate(&mut model, Some(A), MAIN_VIEW);
    assert_eq!(model.refs, ([3; NODES], [2; NODES]));
    deactivate(&mut model, Some(A), AUX_VIEW);
    deactivate(&mut model, Some(A), MAIN_VIEW);
    deactivate(&mut model, Some(A), MAIN_VIEW);
    assert_eq!(model.refs, ([0; NODES], [0; NODES]));
    model.steps.clear();
    deactivate(&mut model, Some(A), MAIN_VIEW);
    assert!(!model.steps.iter().any(|s| matches!(s, Step::Walk(..))));
    assert_eq!(model.refs, ([0; NODES], [0; NODES]));
}

/// The four callbacks: one counter of the child, +1 or -1, never loaded.
#[test]
fn each_callback_moves_one_counter_of_the_child() {
    for tree in Tree::ALL {
        for start in [c_long::MIN + 1, -1, 0, 1, 5] {
            let mut model = Model::new(([start; NODES], [start; NODES]));
            visit(&mut model, C, tree);
            let step = if tree.raises() {
                Step::Inc(C, tree.counter())
            } else {
                Step::Dec(C, tree.counter())
            };
            assert_eq!(model.steps, [step]);
        }
    }
    assert_eq!(Tree::Show.counter(), Counter::Show);
    assert_eq!(Tree::Hide.counter(), Counter::Show);
    assert_eq!(Tree::Activate.counter(), Counter::Activate);
    assert_eq!(Tree::Deactivate.counter(), Counter::Activate);
}
