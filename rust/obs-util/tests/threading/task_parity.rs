//! Tier 3: the task-queue C ABI against the original C, compiled as an
//! oracle.
//!
//! `os_task_queue_t` is opaque in `util/task.h`, so this port adds no
//! `#[repr(C)]` struct and no layout test.
//!
//! What is compared: the observable execution log — which tasks ran, in
//! what order, and what `inside` reported from worker vs. caller threads.
//! A single producer thread plus FIFO semantics makes the log
//! deterministic.
//!
//! Excluded from comparison, not generated here:
//! - `os_task_queue_wait`'s return value in general: it is true only if a
//!   non-marker task ran while the `waiting` flag was set, which races with
//!   the worker draining the queue (see the C test's header comment). The
//!   one deterministic case — a wait on a just-drained queue returns false —
//!   *is* asserted on both sides.
//! - A NULL `os_task_t`: C calls the pointer unconditionally and crashes;
//!   the shim aborts.
//! - `os_task_queue_inside(NULL)`: C dereferences the pointer with no NULL
//!   check (UB); the shim returns false. Likewise a NULL queue passed to
//!   `queue_task` (C dereferences it; the shim returns false).
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::c_void;
use std::ptr;
use std::sync::Mutex;

use obs_c_oracle::task::{
    OracleTaskQueue, oracle_os_task_queue_create, oracle_os_task_queue_destroy,
    oracle_os_task_queue_inside, oracle_os_task_queue_queue_task, oracle_os_task_queue_wait,
};
use obs_util::ffi::task::{
    os_task_queue_create, os_task_queue_destroy, os_task_queue_inside, os_task_queue_queue_task,
    os_task_queue_wait,
};
use proptest::prelude::*;

/// One entry in the observable log, identical for both implementations.
#[derive(Debug, PartialEq, Eq)]
enum Observed {
    Ran(i32),
    Inside(bool),
}

/// `param` handed to every queued task: where to log, and how to ask its
/// own queue whether it is inside it (per-implementation function).
#[repr(C)]
struct TaskCtx {
    log: *mut Mutex<Vec<Observed>>,
    queue: *mut c_void,
    inside: unsafe extern "C" fn(*mut c_void) -> bool,
    tag: i32,
}

unsafe extern "C" fn record(param: *mut c_void) {
    // SAFETY: `param` is a live `TaskCtx` installed at queue time; the test
    // outlives the queue (wait drains before destroy).
    let ctx = unsafe { &*(param as *const TaskCtx) };
    // SAFETY: `ctx.log` points at the test's log.
    unsafe { &*ctx.log }
        .lock()
        .unwrap()
        .push(Observed::Ran(ctx.tag));
}

unsafe extern "C" fn check_inside(param: *mut c_void) {
    // SAFETY: as above.
    let ctx = unsafe { &*(param as *const TaskCtx) };
    let inside = unsafe { (ctx.inside)(ctx.queue) };
    // SAFETY: `ctx.log` points at the test's log.
    unsafe { &*ctx.log }
        .lock()
        .unwrap()
        .push(Observed::Inside(inside));
}

/// One operation a single producer thread performs on a queue.
#[derive(Debug, Clone)]
enum Op {
    /// Queue a recording task.
    Task(i32),
    /// Queue a task that logs what `inside` reports on the worker.
    InsideTask,
    /// Block until the marker passes; then issue a second wait and record
    /// that it returned false (the deterministic case only).
    Wait,
}

/// The C-callable surface under test, once per implementation.
struct Api {
    create: unsafe fn() -> *mut c_void,
    queue_task:
        unsafe fn(*mut c_void, Option<unsafe extern "C" fn(*mut c_void)>, *mut c_void) -> bool,
    destroy: unsafe fn(*mut c_void),
    wait: unsafe fn(*mut c_void) -> bool,
    /// Passed to tasks as an `extern "C"` callback, so it stays `extern`.
    inside: unsafe extern "C" fn(*mut c_void) -> bool,
}

unsafe extern "C" fn rust_inside(q: *mut c_void) -> bool {
    unsafe { os_task_queue_inside(q.cast()) }
}
unsafe extern "C" fn oracle_inside(q: *mut c_void) -> bool {
    unsafe { oracle_os_task_queue_inside(q.cast()) }
}

fn rust_api() -> Api {
    unsafe fn wrap_create() -> *mut c_void {
        unsafe { os_task_queue_create() }.cast()
    }
    unsafe fn wrap_queue(
        tq: *mut c_void,
        task: Option<unsafe extern "C" fn(*mut c_void)>,
        p: *mut c_void,
    ) -> bool {
        unsafe { os_task_queue_queue_task(tq.cast(), task, p) }
    }
    unsafe fn wrap_destroy(tq: *mut c_void) {
        unsafe { os_task_queue_destroy(tq.cast()) }
    }
    unsafe fn wrap_wait(tq: *mut c_void) -> bool {
        unsafe { os_task_queue_wait(tq.cast()) }
    }
    Api {
        create: wrap_create,
        queue_task: wrap_queue,
        destroy: wrap_destroy,
        wait: wrap_wait,
        inside: rust_inside,
    }
}

fn oracle_api() -> Api {
    unsafe fn wrap_create() -> *mut c_void {
        unsafe { oracle_os_task_queue_create() }.cast()
    }
    unsafe fn wrap_queue(
        tq: *mut c_void,
        task: Option<unsafe extern "C" fn(*mut c_void)>,
        p: *mut c_void,
    ) -> bool {
        unsafe { oracle_os_task_queue_queue_task(tq.cast::<OracleTaskQueue>(), task, p) }
    }
    unsafe fn wrap_destroy(tq: *mut c_void) {
        unsafe { oracle_os_task_queue_destroy(tq.cast()) }
    }
    unsafe fn wrap_wait(tq: *mut c_void) -> bool {
        unsafe { oracle_os_task_queue_wait(tq.cast()) }
    }
    Api {
        create: wrap_create,
        queue_task: wrap_queue,
        destroy: wrap_destroy,
        wait: wrap_wait,
        inside: oracle_inside,
    }
}

/// Run the op sequence on one implementation; return the observable log.
/// `log` must outlive the queue: tasks hold a raw pointer to it.
fn run(api: &Api, ops: &[Op], log: &Mutex<Vec<Observed>>) {
    let tq = unsafe { (api.create)() };
    assert!(!tq.is_null());

    // The producer thread is never inside the queue.
    assert!(!unsafe { (api.inside)(tq) });

    for op in ops {
        match *op {
            Op::Task(tag) => {
                let ctx = Box::into_raw(Box::new(TaskCtx {
                    log: log as *const Mutex<Vec<Observed>> as *mut _,
                    queue: tq,
                    inside: api.inside,
                    tag,
                }));
                assert!(unsafe { (api.queue_task)(tq, Some(record), ctx.cast()) });
            }
            Op::InsideTask => {
                let ctx = Box::into_raw(Box::new(TaskCtx {
                    log: log as *const Mutex<Vec<Observed>> as *mut _,
                    queue: tq,
                    inside: api.inside,
                    tag: 0,
                }));
                assert!(unsafe { (api.queue_task)(tq, Some(check_inside), ctx.cast()) });
            }
            Op::Wait => {
                let _ = unsafe { (api.wait)(tq) };
                // The queue is drained now; a wait on an idle queue is
                // deterministically false on both sides.
                assert!(!unsafe { (api.wait)(tq) });
            }
        }
    }

    // Flush whatever is still queued, then stop the worker.
    let _ = unsafe { (api.wait)(tq) };
    unsafe { (api.destroy)(tq) };

    // The task ctx boxes are only read while the queue is alive; reclaim the
    // ones the worker may not have consumed (none — every queued task runs
    // before destroy returns). Leaked ctx boxes would be a test bug.
}

/// NULL-argument behavior the C test pins: identical on both sides.
#[test]
fn null_args_match() {
    unsafe {
        assert!(!os_task_queue_queue_task(
            ptr::null_mut(),
            None,
            ptr::null_mut()
        ));
        assert!(!oracle_os_task_queue_queue_task(
            ptr::null_mut(),
            None,
            ptr::null_mut()
        ));
        assert!(!os_task_queue_wait(ptr::null_mut()));
        assert!(!oracle_os_task_queue_wait(ptr::null_mut()));
        os_task_queue_destroy(ptr::null_mut());
        oracle_os_task_queue_destroy(ptr::null_mut());
    }
}

/// An idle queue's wait is deterministically false on both sides.
#[test]
fn idle_wait_false_matches() {
    for api in [rust_api(), oracle_api()] {
        let tq = unsafe { (api.create)() };
        assert!(!unsafe { (api.wait)(tq) });
        assert!(!unsafe { (api.wait)(tq) });
        unsafe { (api.destroy)(tq) };
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(64)))]

    /// Random op sequences produce identical observable logs.
    #[test]
    fn parity(
        ops in prop::collection::vec(
            prop_oneof![
                4 => any::<i32>().prop_map(Op::Task),
                1 => Just(Op::InsideTask),
                2 => Just(Op::Wait),
            ],
            0..48,
        )
    ) {
        let rust_log = Mutex::new(Vec::new());
        let oracle_log = Mutex::new(Vec::new());
        run(&rust_api(), &ops, &rust_log);
        run(&oracle_api(), &ops, &oracle_log);

        let rust_log = rust_log.into_inner().unwrap();
        let oracle_log = oracle_log.into_inner().unwrap();
        assert_eq!(rust_log, oracle_log);
    }
}
