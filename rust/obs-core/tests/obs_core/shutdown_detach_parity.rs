//! Tier 3: the original C shutdown detachment and the Rust one through the
//! private ABI and the production bridge's calls, over the same real UUID
//! source table.
//!
//! The libobs calls the C makes (the table mutex, `obs_source_get_ref`,
//! `obs_scene_from_source`, `obs_group_from_source`, `obs_scene_enum_items`,
//! `obs_sceneitem_remove`, `obs_source_release` and the destruction queue
//! wait) go, on both sides, to a model of the sources, scenes, groups and the
//! destruction queue (the Tier 1 model). Each case is a random graph:
//! sources of each kind, the references their holders keep (none makes the
//! snapshot reference fail), scenes and groups listing any other source,
//! any table order, and waits that report work the model doesn't run. The
//! log of calls (locks, references, scene and group checks, enumerations,
//! removals, releases, waits) and the end state of the model match exactly.
//!
//! There are no intentional differences.
use super::shutdown_detach::{Call, Kind, Model, State};
use core::ffi::c_void;
use obs_c_oracle::shutdown_detach::*;
use obs_core::ffi::shutdown_detach::{Calls, obs_shutdown_detach_scene_items_run};
use obs_core::shutdown_detach::Core;
use proptest::prelude::*;

/// The item token for `source` listed by `scene`; never 0.
fn token(scene: usize, source: usize) -> usize {
    scene * 4096 + source + 1
}

fn model<'a>(model: *mut c_void) -> &'a Model {
    // SAFETY: the hooks' model is the case's live Model.
    unsafe { &*model.cast::<Model>() }
}

unsafe extern "C" fn lock(m: *mut c_void) {
    model(m).lock();
}
unsafe extern "C" fn unlock(m: *mut c_void) {
    model(m).unlock();
}
unsafe extern "C" fn get_ref(m: *mut c_void, source: usize) -> bool {
    model(m).get_ref(source).is_some()
}
unsafe extern "C" fn scene_from_source(m: *mut c_void, source: usize) -> bool {
    model(m).scene_from_source(source).is_some()
}
unsafe extern "C" fn group_from_source(m: *mut c_void, source: usize) -> bool {
    model(m).group_from_source(source).is_some()
}
unsafe extern "C" fn enum_items(
    m: *mut c_void,
    scene: usize,
    handle: *mut c_void,
    callback: ItemCallback,
    param: *mut c_void,
) {
    model(m).enum_items(scene, &mut |(scene, source)| {
        // SAFETY: the code under test's own callback and param, called
        // with the scene's handle and an item token, as obs_scene_enum_items
        // does; no model borrow is held.
        unsafe { callback(handle, token(scene, source) as *mut c_void, param) }
    });
}
unsafe extern "C" fn item_remove(m: *mut c_void, item: usize) {
    let item = item.checked_sub(1).expect("item token is never 0");
    model(m).item_remove((item / 4096, item % 4096));
}
unsafe extern "C" fn release(m: *mut c_void, source: usize) {
    model(m).release(source);
}
unsafe extern "C" fn wait(m: *mut c_void) -> bool {
    model(m).wait_destruction()
}

/// One random case.
#[derive(Clone, Debug)]
struct Case {
    kinds: Vec<Kind>,
    holds: Vec<u32>,
    /// (scene or group, listed source) pairs; pairs on an input are dropped.
    items: Vec<(usize, usize)>,
    order: Vec<usize>,
    extra_waits: u32,
}

impl Case {
    fn model(&self) -> Model {
        let model = Model::new(&self.kinds);
        for (source, &holds) in self.holds.iter().enumerate() {
            for _ in 0..holds {
                model.hold(source);
            }
        }
        for &(scene, source) in &self.items {
            if self.kinds[scene] != Kind::Input && scene != source {
                model.add(scene, source);
            }
        }
        model.0.borrow_mut().extra_waits = self.extra_waits;
        model
    }
}

fn case() -> impl Strategy<Value = Case> {
    (0usize..=8).prop_flat_map(|n| {
        let kind = prop_oneof![Just(Kind::Input), Just(Kind::Scene), Just(Kind::Group)];
        let pairs = if n == 0 {
            Just(Vec::new()).boxed()
        } else {
            prop::collection::vec((0..n, 0..n), 0..=12).boxed()
        };
        (
            prop::collection::vec(kind, n),
            prop::collection::vec(0u32..=2, n),
            pairs,
            Just((0..n).collect::<Vec<_>>()).prop_shuffle(),
            0u32..=3,
        )
            .prop_map(|(kinds, holds, items, order, extra_waits)| Case {
                kinds,
                holds,
                items,
                order,
                extra_waits,
            })
    })
}

/// What a run leaves: the calls and the model's end state.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    calls: Vec<Call>,
    refs: Vec<u32>,
    items: Vec<Vec<usize>>,
    destroyed: Vec<bool>,
    removed: u32,
    tasks: u32,
    unlocked: bool,
    retained: bool,
    locked: bool,
}

fn outcome(s: &State) -> Outcome {
    Outcome {
        calls: s.calls.clone(),
        refs: s.sources.iter().map(|src| src.refs).collect(),
        items: s.sources.iter().map(|src| src.items.clone()).collect(),
        destroyed: s.sources.iter().map(|src| src.destroyed).collect(),
        removed: s.removed,
        tasks: s.tasks,
        unlocked: s.unlocked,
        retained: s.retained,
        locked: s.locked,
    }
}

/// Run `case` through the original C (`rust` false) or the Rust.
fn run(case: &Case, rust: bool) -> Outcome {
    let model = case.model();
    let hooks = Hooks {
        model: (&raw const model).cast_mut().cast(),
        lock,
        unlock,
        get_ref,
        scene_from_source,
        group_from_source,
        enum_items,
        item_remove,
        release,
        wait,
    };
    // SAFETY: order lists each of the case's indices once; the fixture,
    // hooks and model outlive the run, and the hooks are cleared before
    // the fixture is freed.
    unsafe {
        let fixture = oracle_shutdown_detach_new(case.kinds.len(), case.order.as_ptr());
        assert!(!fixture.is_null(), "out of memory");
        oracle_shutdown_detach_install(fixture, &hooks);
        if rust {
            obs_shutdown_detach_scene_items_run(
                oracle_shutdown_detach_data(fixture),
                oracle_shutdown_detach_calls().cast::<Calls>(),
            );
        } else {
            oracle_shutdown_detach_original(fixture);
        }
        oracle_shutdown_detach_install(core::ptr::null_mut(), core::ptr::null());
        oracle_shutdown_detach_free(fixture);
    }
    outcome(&model.state())
}

fn snapshot_case() -> Case {
    use Kind::*;
    // test_shutdown_detaches_snapshot's graph (Tier 1's snapshot_graph).
    Case {
        kinds: vec![Scene, Scene, Group, Group, Input, Input],
        holds: vec![1, 0, 0, 0, 0, 0],
        items: vec![(1, 2), (2, 3), (0, 1), (0, 4), (1, 5), (3, 4), (3, 5)],
        order: (0..6).collect(),
        extra_waits: 0,
    }
}

/// The characterization's graphs and the C's order: the same calls and end
/// state on both sides.
#[test]
fn characterization_graphs_match() {
    use Kind::*;
    let cases = [
        snapshot_case(),
        // test_shutdown_releases_item_owned_input.
        Case {
            kinds: vec![Input, Scene],
            holds: vec![0, 1],
            items: vec![(1, 0)],
            order: vec![0, 1],
            extra_waits: 0,
        },
        // A reversed table, a failed reference and a busy queue.
        Case {
            kinds: vec![Scene, Input, Group, Scene],
            holds: vec![1, 0, 0, 0],
            items: vec![(0, 1), (0, 2), (2, 1)],
            order: vec![3, 2, 1, 0],
            extra_waits: 2,
        },
        Case {
            kinds: vec![],
            holds: vec![],
            items: vec![],
            order: vec![],
            extra_waits: 1,
        },
    ];
    for case in cases {
        let c = run(&case, false);
        let rust = run(&case, true);
        assert_eq!(c, rust, "{case:?}");
    }
    let c = run(&snapshot_case(), false);
    assert_eq!(c.removed, 7);
    assert!(c.unlocked && c.retained && !c.locked);
    assert_eq!(c.tasks, 4);
    assert!(c.calls.contains(&Call::Unlock));
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    /// Random graphs: the same calls and end state on both sides.
    #[test]
    fn random_graphs_match(case in case()) {
        prop_assert_eq!(run(&case, false), run(&case, true));
    }
}
