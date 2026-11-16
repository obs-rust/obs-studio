//! Tier 1: every assertion of test_shutdown_detach.c, without FFI, over a
//! model source table, model scenes and groups and a model destruction
//! queue that log each call.
use obs_core::shutdown_detach::{Core, detach_scene_items, remove_scene_item};
use std::cell::RefCell;
use std::collections::VecDeque;

/// What a model source is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Input,
    Scene,
    Group,
}

#[derive(Clone, Debug)]
pub struct Source {
    pub kind: Kind,
    /// Strong references; 0 once its destroy is queued.
    pub refs: u32,
    /// Its items' sources (scenes and groups only), in item order.
    pub items: Vec<usize>,
    pub destroyed: bool,
}

/// A destruction queue task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    /// The deferred destroy of a source whose last reference went.
    Destroy(usize),
    /// test_shutdown_detach's `first_task`, queued by an input's destroy.
    First,
    /// Its `second_task`, queued by `first_task`.
    Second,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Lock,
    Unlock,
    Visit(usize),
    GetRef(usize),
    SceneFrom(usize),
    GroupFrom(usize),
    Enum(usize),
    Remove(usize, usize),
    Release(usize),
    Wait,
}

#[derive(Default)]
pub struct State {
    pub sources: Vec<Source>,
    /// The UUID table, in table order.
    pub table: Vec<usize>,
    pub locked: bool,
    pub queue: VecDeque<Task>,
    /// Waits that report work although the queue is empty (another
    /// producer's work the model doesn't run).
    pub extra_waits: u32,
    pub calls: Vec<Call>,
    /// test_shutdown_detach's counters.
    pub removed: u32,
    pub destroyed: u32,
    pub tasks: u32,
    pub unlocked: bool,
    pub retained: bool,
    /// The sources whose reference the snapshot holds, as get_ref returned
    /// them: every one must be alive at each removal.
    pub snapshot: Vec<usize>,
}

pub struct Model(pub RefCell<State>);

impl Model {
    pub fn new(kinds: &[Kind]) -> Self {
        let sources = kinds
            .iter()
            .map(|&kind| Source {
                kind,
                refs: 0,
                items: Vec::new(),
                destroyed: false,
            })
            .collect();
        Model(RefCell::new(State {
            sources,
            table: (0..kinds.len()).collect(),
            unlocked: true,
            retained: true,
            ..State::default()
        }))
    }
    /// `obs_scene_add()`: the item holds a reference to its source.
    pub fn add(&self, scene: usize, source: usize) {
        let mut s = self.0.borrow_mut();
        s.sources[scene].items.push(source);
        s.sources[source].refs += 1;
    }
    /// A reference the host (or an owner outside the model) holds.
    pub fn hold(&self, source: usize) {
        self.0.borrow_mut().sources[source].refs += 1;
    }
    pub fn state(&self) -> std::cell::Ref<'_, State> {
        self.0.borrow()
    }
}

/// Drop a reference; the last one queues the source's destroy.
fn drop_ref(s: &mut State, source: usize) {
    let src = &mut s.sources[source];
    assert!(src.refs > 0, "source {source} released without a reference");
    src.refs -= 1;
    if src.refs == 0 {
        s.queue.push_back(Task::Destroy(source));
    }
}

/// Run one destruction task, the way the destruction thread does.
fn run_task(s: &mut State, task: Task) {
    match task {
        Task::Destroy(source) => {
            s.sources[source].destroyed = true;
            s.table.retain(|&t| t != source);
            // A destroyed scene's remaining items release their sources.
            let items = std::mem::take(&mut s.sources[source].items);
            for item in items {
                drop_ref(s, item);
            }
            if s.sources[source].kind == Kind::Input {
                s.destroyed += 1;
                s.queue.push_back(Task::First);
            }
        }
        Task::First => {
            s.tasks += 1;
            s.queue.push_back(Task::Second);
        }
        Task::Second => s.tasks += 1,
    }
}

/// Run what was queued before the wait; whether there was any.
fn drain_once(s: &mut State) -> bool {
    let batch: Vec<Task> = s.queue.drain(..).collect();
    let any = !batch.is_empty();
    for task in batch {
        run_task(s, task);
    }
    any
}

impl Core for Model {
    type Source = usize;
    type Scene = usize;
    type Item = (usize, usize);
    fn lock(&self) {
        let mut s = self.0.borrow_mut();
        assert!(!s.locked, "sources_mutex taken twice");
        s.locked = true;
        s.calls.push(Call::Lock);
    }
    fn unlock(&self) {
        let mut s = self.0.borrow_mut();
        assert!(s.locked, "sources_mutex released unheld");
        s.locked = false;
        s.calls.push(Call::Unlock);
    }
    fn each_source(&self, visit: &mut dyn FnMut(usize)) {
        let table = {
            let s = self.0.borrow();
            assert!(s.locked, "source table walked unlocked");
            s.table.clone()
        };
        for source in table {
            self.0.borrow_mut().calls.push(Call::Visit(source));
            visit(source);
        }
    }
    fn get_ref(&self, source: usize) -> Option<usize> {
        let mut s = self.0.borrow_mut();
        assert!(s.locked, "snapshot reference taken unlocked");
        s.calls.push(Call::GetRef(source));
        if s.sources[source].refs == 0 {
            return None;
        }
        s.sources[source].refs += 1;
        s.snapshot.push(source);
        Some(source)
    }
    fn scene_from_source(&self, source: usize) -> Option<usize> {
        let mut s = self.0.borrow_mut();
        s.calls.push(Call::SceneFrom(source));
        (s.sources[source].kind == Kind::Scene).then_some(source)
    }
    fn group_from_source(&self, source: usize) -> Option<usize> {
        let mut s = self.0.borrow_mut();
        s.calls.push(Call::GroupFrom(source));
        (s.sources[source].kind == Kind::Group).then_some(source)
    }
    fn enum_items(&self, scene: usize, visit: &mut dyn FnMut((usize, usize)) -> bool) {
        let items = {
            let mut s = self.0.borrow_mut();
            assert!(!s.locked, "items enumerated under sources_mutex");
            s.calls.push(Call::Enum(scene));
            s.sources[scene].items.clone()
        };
        for item in items {
            if !visit((scene, item)) {
                break;
            }
        }
    }
    fn item_remove(&self, (scene, item): (usize, usize)) {
        let mut s = self.0.borrow_mut();
        s.calls.push(Call::Remove(scene, item));
        if s.locked {
            s.unlocked = false;
        }
        s.removed += 1;
        let pos = s.sources[scene]
            .items
            .iter()
            .position(|&i| i == item)
            .expect("removed item is listed");
        s.sources[scene].items.remove(pos);
        drop_ref(&mut s, item);
        // Like item_removed(): finish queued destruction, then check that
        // every snapshot member is still alive and no input was destroyed.
        while drain_once(&mut s) {}
        let snapshot = s.snapshot.clone();
        if snapshot.iter().any(|&m| s.sources[m].destroyed) {
            s.retained = false;
        }
        assert_eq!(s.destroyed, 0, "an input destroyed while items remain");
    }
    fn release(&self, source: usize) {
        let mut s = self.0.borrow_mut();
        assert!(!s.locked, "snapshot released under sources_mutex");
        s.calls.push(Call::Release(source));
        drop_ref(&mut s, source);
    }
    fn wait_destruction(&self) -> bool {
        let mut s = self.0.borrow_mut();
        assert!(!s.locked, "destruction waited on under sources_mutex");
        s.calls.push(Call::Wait);
        if drain_once(&mut s) {
            return true;
        }
        if s.extra_waits > 0 {
            s.extra_waits -= 1;
            return true;
        }
        false
    }
}

const OUTER: usize = 0;
const INNER: usize = 1;
const GROUP: usize = 2;
const NESTED: usize = 3;
const FIRST: usize = 4;
const SECOND: usize = 5;

/// test_shutdown_detaches_snapshot's graph: the host leaks `outer`, which
/// lists `inner` and the shared private input; `inner` lists a group, which
/// lists a nested group, and the public input; the nested group lists both
/// inputs. The host released everything but `outer`.
fn snapshot_graph() -> Model {
    use Kind::*;
    let model = Model::new(&[Scene, Scene, Group, Group, Input, Input]);
    model.hold(OUTER);
    model.add(INNER, GROUP);
    model.add(GROUP, NESTED);
    model.add(OUTER, INNER);
    model.add(OUTER, FIRST);
    model.add(INNER, SECOND);
    model.add(NESTED, FIRST);
    model.add(NESTED, SECOND);
    model
}

/// test_shutdown_detaches_snapshot: all seven items of the scenes, groups
/// and nested groups are removed with the table unlocked, every snapshot
/// member stays alive until the last removal, both inputs are destroyed and
/// their four queued tasks run before the detachment returns; only the
/// host's scene stays alive.
#[test]
fn shutdown_detaches_snapshot() {
    let model = snapshot_graph();
    detach_scene_items(&model);
    let s = model.state();
    assert_eq!(s.removed, 7);
    assert!(s.unlocked);
    assert!(s.retained);
    assert_eq!(s.destroyed, 2);
    assert_eq!(s.tasks, 4);
    assert!(s.queue.is_empty());
    assert!(!s.locked);
    assert!(!s.sources[OUTER].destroyed);
    assert_eq!(s.sources[OUTER].refs, 1);
    for source in [INNER, GROUP, NESTED, FIRST, SECOND] {
        assert!(s.sources[source].destroyed, "source {source} left alive");
    }
    assert_eq!(s.table, [OUTER]);
}

/// test_shutdown_releases_item_owned_input: an input created before the
/// host's scene and owned only by its item is detached, released and
/// destroyed (with its two tasks) before the detachment returns, so only
/// the scene is left in the table.
#[test]
fn shutdown_releases_item_owned_input() {
    let model = Model::new(&[Kind::Input, Kind::Scene]);
    model.hold(1);
    model.add(1, 0);
    detach_scene_items(&model);
    let s = model.state();
    assert_eq!(s.destroyed, 1);
    assert_eq!(s.tasks, 2);
    assert_eq!(s.table, [1]);
    assert_eq!(s.sources[1].refs, 1);
    assert!(s.sources[1].items.is_empty());
}

/// The C's order: the snapshot under the lock, a reference per member in
/// table order; then, unlocked, per member the scene check, the group check
/// only when it isn't a scene, and the enumeration with its removals; then
/// every release, in snapshot order; then the waits until one reports no
/// work.
#[test]
fn calls_keep_the_c_order() {
    use Call::*;
    let model = Model::new(&[Kind::Scene, Kind::Input, Kind::Group]);
    model.hold(0);
    model.add(0, 1);
    model.add(0, 2);
    model.add(2, 1);
    model.0.borrow_mut().extra_waits = 2;
    detach_scene_items(&model);
    let s = model.state();
    assert_eq!(
        s.calls,
        [
            Lock,
            Visit(0),
            GetRef(0),
            Visit(1),
            GetRef(1),
            Visit(2),
            GetRef(2),
            Unlock,
            SceneFrom(0),
            Enum(0),
            Remove(0, 1),
            Remove(0, 2),
            SceneFrom(1),
            GroupFrom(1),
            SceneFrom(2),
            GroupFrom(2),
            Enum(2),
            Remove(2, 1),
            Release(0),
            Release(1),
            Release(2),
            Wait,
            Wait,
            Wait,
            Wait,
            Wait,
            Wait,
        ]
    );
    assert_eq!(s.destroyed, 1);
    assert_eq!(s.tasks, 2);
}

/// A member whose strong reference fails (its destroy is already under way)
/// is skipped: it is not checked, enumerated or released.
#[test]
fn failed_reference_is_skipped() {
    let model = Model::new(&[Kind::Scene, Kind::Scene, Kind::Input]);
    model.hold(1);
    model.add(1, 2);
    // Source 0 has no reference left: get_ref fails.
    detach_scene_items(&model);
    let s = model.state();
    assert!(s.calls.contains(&Call::GetRef(0)));
    for call in [
        Call::SceneFrom(0),
        Call::GroupFrom(0),
        Call::Enum(0),
        Call::Release(0),
    ] {
        assert!(!s.calls.contains(&call), "{call:?} for a failed reference");
    }
    assert_eq!(s.snapshot, [1, 2]);
    assert_eq!(s.destroyed, 1);
    assert!(s.retained);
}

/// An empty table takes and releases the lock, then waits once.
#[test]
fn empty_table_waits_once() {
    let model = Model::new(&[]);
    detach_scene_items(&model);
    assert_eq!(model.state().calls, [Call::Lock, Call::Unlock, Call::Wait]);
}

/// The wait loop has no bound: it repeats while the queue reports work.
#[test]
fn drain_repeats_until_quiet() {
    let model = Model::new(&[]);
    model.0.borrow_mut().extra_waits = 100;
    detach_scene_items(&model);
    let s = model.state();
    assert_eq!(s.calls.iter().filter(|&&c| c == Call::Wait).count(), 101);
}

/// obs_shutdown_remove_scene_item removes the item and always continues.
#[test]
fn remove_scene_item_continues() {
    let model = Model::new(&[Kind::Scene, Kind::Input]);
    model.add(0, 1);
    model.hold(1);
    assert!(remove_scene_item(&model, (0, 1)));
    let s = model.state();
    assert_eq!(s.calls, [Call::Remove(0, 1)]);
    assert!(s.sources[0].items.is_empty());
    assert_eq!(s.sources[1].refs, 1);
}
