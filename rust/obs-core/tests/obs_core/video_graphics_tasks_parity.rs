//! Tier 3: the original C graphics task drain and the Rust one through the
//! private ABI, over the same real core, task_mutex and deque, and the
//! production bridge's accessors.
//!
//! Each case is a tree of tasks: the first `roots` are queued before the
//! drain, and every other task is queued, the way another producer does
//! (taking task_mutex), by its parent while the parent runs. The tasks that
//! ran, in order, whether task_mutex was held for each, whether every push
//! got the mutex, the queue left empty and the mutex released match exactly.
//!
//! One intentional difference, which the C can't be compared on (it
//! crashes): a NULL task, which `obs_queue_task()` queues without a check
//! and the C calls, is skipped by the Rust, as the audio task drain does
//! (`null_task_is_skipped`).
use super::video_graphics_tasks::{Model, expected_calls};
use core::ffi::c_void;
use obs_c_oracle::video_graphics_tasks::*;
use obs_core::ffi::audio_callback::obs_task_info;
use obs_core::ffi::video_graphics_tasks::obs_video_graphics_tasks_execute;
use obs_core::video_graphics_tasks::execute;
use proptest::prelude::*;
use std::cell::RefCell;

/// One task call: its id and whether task_mutex was held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ran {
    id: usize,
    locked: bool,
}

struct Run {
    core: *mut c_void,
    children: Vec<Vec<usize>>,
    ran: Vec<Ran>,
    pushes_failed: usize,
}
thread_local! { static RUN: RefCell<Option<Run>> = const { RefCell::new(None) }; }

unsafe extern "C" fn task(param: *mut c_void) {
    let id = param as usize;
    let (core, children) = RUN.with_borrow_mut(|run| {
        let run = run.as_mut().expect("a run is active");
        // SAFETY: the run's core is live until run() frees it.
        let locked = unsafe { oracle_graphics_tasks_locked(run.core) };
        run.ran.push(Ran { id, locked });
        (run.core, run.children[id].clone())
    });
    // No RefCell borrow is held across the pushes.
    for child in children {
        // SAFETY: the run's live core; child is the next task's id.
        let pushed = unsafe { oracle_graphics_tasks_push(core, Some(task), child as *mut c_void) };
        if !pushed {
            RUN.with_borrow_mut(|run| run.as_mut().expect("active").pushes_failed += 1);
        }
    }
}

// The bridge's accessors, wrapped to check that the size and every pop are
// read under task_mutex.
unsafe extern "C" fn lock(core: *mut c_void) {
    // SAFETY: the run's live core.
    unsafe { oracle_graphics_tasks_lock(core) }
}
unsafe extern "C" fn unlock(core: *mut c_void) {
    // SAFETY: the run's live core, locked by lock().
    unsafe { oracle_graphics_tasks_unlock(core) }
}
unsafe extern "C" fn pending(core: *mut c_void) -> bool {
    // SAFETY: the run's live core.
    unsafe {
        assert!(oracle_graphics_tasks_locked(core), "size read unlocked");
        oracle_graphics_tasks_pending(core)
    }
}
unsafe extern "C" fn pop(core: *mut c_void, info: *mut obs_task_info) {
    // SAFETY: the run's live core; the Rust pops only after pending().
    unsafe {
        assert!(oracle_graphics_tasks_locked(core), "pop unlocked");
        oracle_graphics_tasks_pop(core, info.cast())
    }
}

/// The task tree: `children[id]` are the tasks `id` queues while it runs.
fn tree(roots: usize, parents: &[usize]) -> Vec<Vec<usize>> {
    let mut children = vec![Vec::new(); roots + parents.len()];
    for (offset, &raw) in parents.iter().enumerate() {
        let id = roots + offset;
        children[raw % id].push(id);
    }
    children
}

/// The tasks run, failed pushes, and checks the queue is left empty and the
/// mutex released.
fn run(roots: usize, children: &[Vec<usize>], original: bool) -> (Vec<Ran>, usize) {
    // SAFETY: freed below; a NULL core fails the assert first.
    let core = unsafe { oracle_graphics_tasks_new() };
    assert!(!core.is_null());
    for id in 0..roots {
        // SAFETY: the live core, unlocked.
        assert!(unsafe { oracle_graphics_tasks_push(core, Some(task), id as *mut c_void) });
    }
    RUN.with_borrow_mut(|run| {
        *run = Some(Run {
            core,
            children: children.to_vec(),
            ran: Vec::new(),
            pushes_failed: 0,
        })
    });
    // SAFETY: a live core with a queue of this run's tasks, the bridge's
    // accessors and non-unwinding tasks (an assert in a callback aborts).
    unsafe {
        if original {
            oracle_graphics_tasks_original(core)
        } else {
            obs_video_graphics_tasks_execute(core, lock, unlock, pending, pop)
        }
    }
    let done = RUN.with_borrow_mut(|run| run.take().expect("still active"));
    // SAFETY: the same live core.
    unsafe {
        assert_eq!(oracle_graphics_tasks_size(core), 0, "queue drained");
        assert!(!oracle_graphics_tasks_locked(core), "task_mutex released");
        oracle_graphics_tasks_free(core);
    }
    (done.ran, done.pushes_failed)
}

fn check(roots: usize, parents: &[usize]) {
    let children = tree(roots, parents);
    let expected = run(roots, &children, true);
    assert_eq!(run(roots, &children, false), expected);
    // The C runs every task once, oldest queued first, unlocked, and every
    // producer gets the mutex.
    assert_eq!(expected.0.len(), children.len());
    assert!(expected.0.iter().all(|ran| !ran.locked));
    assert_eq!(expected.1, 0);
    // The model drain runs the same order with the C's calls.
    let mut model = Model {
        tasks: (0..roots as u32).collect(),
        queued: children
            .iter()
            .map(|c| c.iter().map(|&id| id as u32).collect())
            .collect(),
        ..Model::default()
    };
    execute(&mut model);
    let order: Vec<u32> = expected.0.iter().map(|ran| ran.id as u32).collect();
    assert_eq!(model.values, order);
    assert_eq!(model.calls, expected_calls(&order));
}

/// The characterization's two cases: four queued tasks, and tasks 1 and 4
/// queued with task 1 queueing task 5 while it runs.
#[test]
fn characterized_queues() {
    check(0, &[]);
    check(4, &[]);
    // ids 0 and 1 queued; 2 is queued by 0.
    check(2, &[0]);
}

/// Every tree of up to six tasks with up to three queued first.
#[test]
fn every_small_tree() {
    for roots in 1..=3usize {
        for extra in 0..=3usize {
            let mut parents = vec![0usize; extra];
            loop {
                check(roots, &parents);
                // Next parent vector: parent of task roots + i is in 0..roots + i.
                let mut i = 0;
                while i < extra {
                    parents[i] += 1;
                    if parents[i] < roots + i {
                        break;
                    }
                    parents[i] = 0;
                    i += 1;
                }
                if i == extra {
                    break;
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn arbitrary_task_trees(
        roots in 1usize..16,
        parents in proptest::collection::vec(0usize..1024, 0..48),
    ) {
        check(roots, &parents);
    }
}

/// A NULL task, which `obs_queue_task()` queues without a check and the C
/// calls and crashes on, is skipped: the tasks queued around it run, in
/// order, the queue is left empty and the mutex released. Rust only (the C
/// would crash).
#[test]
fn null_task_is_skipped() {
    // SAFETY: freed below; a NULL core fails the assert first.
    let core = unsafe { oracle_graphics_tasks_new() };
    assert!(!core.is_null());
    // SAFETY: the live core, unlocked; ids 0 and 1 around a NULL task.
    unsafe {
        let [first, second] = [0usize, 1];
        assert!(oracle_graphics_tasks_push(
            core,
            Some(task),
            first as *mut c_void
        ));
        assert!(oracle_graphics_tasks_push(
            core,
            None,
            core::ptr::null_mut()
        ));
        assert!(oracle_graphics_tasks_push(
            core,
            Some(task),
            second as *mut c_void
        ));
    }
    RUN.with_borrow_mut(|run| {
        *run = Some(Run {
            core,
            children: vec![Vec::new(); 2],
            ran: Vec::new(),
            pushes_failed: 0,
        })
    });
    // SAFETY: a live core, the bridge's accessors, non-unwinding tasks.
    unsafe { obs_video_graphics_tasks_execute(core, lock, unlock, pending, pop) };
    let done = RUN.with_borrow_mut(|run| run.take().expect("still active"));
    let ids: Vec<usize> = done.ran.iter().map(|ran| ran.id).collect();
    assert_eq!(ids, [0, 1]);
    assert!(done.ran.iter().all(|ran| !ran.locked));
    // SAFETY: the same live core.
    unsafe {
        assert_eq!(oracle_graphics_tasks_size(core), 0, "queue drained");
        assert!(!oracle_graphics_tasks_locked(core), "task_mutex released");
        oracle_graphics_tasks_free(core);
    }
}
