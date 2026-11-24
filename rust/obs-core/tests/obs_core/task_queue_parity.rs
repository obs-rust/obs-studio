//! Tier 3: the unchanged original C (`libobs/obs-task-queue.c.h`, oracle
//! `oracle_obs_*`) versus the production Rust bridge/core
//! (`in_task_thread_with` / `queue_task_with` / `wait_for_destroy_queue_with`)
//! over one real-header core read through the real field adapters, with one
//! recorder for every call (`oracle/task_queue.c`): the UI thread mark, the
//! UI handler, the `os_task_queue_*`, `os_event_*` and `os_sleep_ms` calls,
//! `blog` (level and formatted text), and the graphics and audio task locks
//! and pushes. The recorder plays the other threads, so the waits complete
//! and a nested task queues from its own thread; both sides use the
//! production route (`core_setup_rules`).
//!
//! Both sides must leave the same log, result, pending tasks and live events,
//! and leave the core unchanged. With clobbering, the recorder changes the
//! destruction queue, the video thread flag, the audio output and the UI
//! handler after each call, so a field read moved past a call changes the
//! log.
//!
//! Over a grid of every entry point, type (the four, -1, 4 and 99), task
//! (NULL, plain, nested), wait, thread flags, UI handler, failed
//! `os_event_init` on each wait path, tasks pending before the call and
//! clobbering, then 512 proptest cases over every field.
//!
//! Excluded, the one intentional difference: a NULL task routed to run in
//! place (the C calls NULL; the Rust does nothing, covered by Tier 1), and
//! `obs_in_task_thread()` of an unknown type (the C asserts).
use core::ffi::{c_char, c_int, c_void};
use obs_core::ffi::task_queue::{
    Calls, Libobs, ObsTask, in_task_thread_with, queue_task_with, wait_for_destroy_queue_with,
};
use obs_core::task_queue::{LOG_ERROR, LOG_WARNING, OS_EVENT_TYPE_AUTO, OS_EVENT_TYPE_MANUAL};
use proptest::prelude::*;

const LOG: usize = 8192;
const STATE: usize = 18;

type InFn = unsafe extern "C" fn(c_int) -> bool;
type QueueFn = unsafe extern "C" fn(c_int, ObsTask, *mut c_void, bool);
type DestroyFn = unsafe extern "C" fn() -> bool;

unsafe extern "C" {
    fn oracle_obs_in_task_thread(task_type: c_int) -> bool;
    fn oracle_obs_queue_task(task_type: c_int, task: ObsTask, param: *mut c_void, wait: bool);
    fn oracle_obs_wait_for_destroy_queue() -> bool;
    fn oracle_task_queue_run(
        in_fn: InFn,
        queue: QueueFn,
        destroy: DestroyFn,
        state: *const c_int,
        log: *mut c_char,
        log_len: usize,
        report: *mut c_int,
    );
    fn oracle_task_queue_values(out: *mut c_int);

    fn oracle_tq_ui_thread() -> bool;
    fn oracle_tq_inside(queue: *mut c_void) -> bool;
    fn oracle_tq_queue_task(queue: *mut c_void, task: ObsTask, param: *mut c_void) -> bool;
    fn oracle_tq_wait(queue: *mut c_void) -> bool;
    fn oracle_tq_event_init(event: *mut *mut c_void, kind: c_int) -> c_int;
    fn oracle_tq_event_signal(event: *mut c_void) -> c_int;
    fn oracle_tq_event_wait(event: *mut c_void) -> c_int;
    fn oracle_tq_event_destroy(event: *mut c_void);
    fn oracle_tq_sleep_ms(ms: u32);
    fn oracle_tq_blog(level: c_int, format: *const c_char, ...);
}

/// The real adapters over the fixture and the C side's recording stand-ins,
/// as the bridge's [`Calls`].
enum Rec {}

// SAFETY: each method is the real adapter (over the fixture core) or the
// recording stand-in of the C function its trait doc names, which only logs,
// plays the other threads and answers from the fixture's config.
unsafe impl Calls for Rec {
    fn graphics_thread() -> bool {
        Libobs::graphics_thread()
    }
    fn audio_thread() -> bool {
        Libobs::audio_thread()
    }
    fn ui_thread() -> bool {
        // SAFETY: logs only.
        unsafe { oracle_tq_ui_thread() }
    }
    fn destruction() -> *mut c_void {
        Libobs::destruction()
    }
    unsafe fn inside(queue: *mut c_void) -> bool {
        // SAFETY: logs the id only.
        unsafe { oracle_tq_inside(queue) }
    }
    fn has_ui_handler() -> bool {
        Libobs::has_ui_handler()
    }
    unsafe fn call_ui_handler(task: ObsTask, param: *mut c_void, wait: bool) {
        // SAFETY: the fixture's handler, which logs only.
        unsafe { Libobs::call_ui_handler(task, param, wait) }
    }
    unsafe fn log(level: c_int, format: *const c_char, value: Option<c_int>) {
        // SAFETY: see the trait; the stand-in formats %d only.
        unsafe {
            match value {
                Some(value) => oracle_tq_blog(level, format, value),
                None => oracle_tq_blog(level, format),
            }
        }
    }
    unsafe fn event_init(event: *mut *mut c_void, kind: c_int) -> c_int {
        // SAFETY: see the trait.
        unsafe { oracle_tq_event_init(event, kind) }
    }
    unsafe fn event_signal(event: *mut c_void) {
        // SAFETY: a fixture event.
        unsafe { oracle_tq_event_signal(event) };
    }
    unsafe fn event_wait(event: *mut c_void) {
        // SAFETY: a fixture event; the stand-in runs the pending tasks.
        unsafe { oracle_tq_event_wait(event) };
    }
    unsafe fn event_destroy(event: *mut c_void) {
        // SAFETY: a fixture event or NULL.
        unsafe { oracle_tq_event_destroy(event) }
    }
    fn sleep_ms(ms: u32) {
        // SAFETY: logs and runs one pending task.
        unsafe { oracle_tq_sleep_ms(ms) }
    }
    fn lock_graphics() {
        Libobs::lock_graphics();
    }
    unsafe fn push_graphics(task: ObsTask, param: *mut c_void) {
        // SAFETY: the stand-in deque push behind the real adapter.
        unsafe { Libobs::push_graphics(task, param) }
    }
    fn unlock_graphics() {
        Libobs::unlock_graphics();
    }
    fn lock_audio() {
        Libobs::lock_audio();
    }
    unsafe fn push_audio(task: ObsTask, param: *mut c_void) {
        // SAFETY: as push_graphics.
        unsafe { Libobs::push_audio(task, param) }
    }
    fn unlock_audio() {
        Libobs::unlock_audio();
    }
    unsafe fn queue_destroy(queue: *mut c_void, task: ObsTask, param: *mut c_void) -> bool {
        // SAFETY: logs and keeps the task for the fixture's destroy queue.
        unsafe { oracle_tq_queue_task(queue, task, param) }
    }
    unsafe fn wait_destroy(queue: *mut c_void) -> bool {
        // SAFETY: logs and runs the fixture's destroy queue.
        unsafe { oracle_tq_wait(queue) }
    }
    fn video_thread_initialized() -> bool {
        Libobs::video_thread_initialized()
    }
    fn has_audio() -> bool {
        Libobs::has_audio()
    }
}

unsafe extern "C" fn rust_in(task_type: c_int) -> bool {
    // SAFETY: the fixture's core is live for the run.
    unsafe { in_task_thread_with::<Rec>(task_type) }
}

unsafe extern "C" fn rust_queue(task_type: c_int, task: ObsTask, param: *mut c_void, wait: bool) {
    // SAFETY: as rust_in; the fixture's tasks take its tokens.
    unsafe { queue_task_with::<Rec>(task_type, task, param, wait) }
}

unsafe extern "C" fn rust_destroy() -> bool {
    // SAFETY: as rust_in.
    unsafe { wait_for_destroy_queue_with::<Rec>() }
}

fn run(original: bool, state: &[c_int; STATE]) -> (String, [c_int; 4]) {
    let mut log = vec![0u8; LOG];
    let mut report = [0 as c_int; 4];
    let (in_fn, queue, destroy): (InFn, QueueFn, DestroyFn) = if original {
        (
            oracle_obs_in_task_thread,
            oracle_obs_queue_task,
            oracle_obs_wait_for_destroy_queue,
        )
    } else {
        (rust_in, rust_queue, rust_destroy)
    };
    // SAFETY: an 18-int state in the fixture's ranges, a LOG-byte buffer and
    // a 4-int report. The fixture is this thread's and resets after the run.
    unsafe {
        oracle_task_queue_run(
            in_fn,
            queue,
            destroy,
            state.as_ptr(),
            log.as_mut_ptr().cast(),
            LOG,
            report.as_mut_ptr(),
        )
    };
    let len = log.iter().position(|&b| b == 0).unwrap();
    log.truncate(len);
    (String::from_utf8(log).unwrap(), report)
}

/// The excluded inputs (see the module doc).
fn excluded(state: &[c_int; STATE]) -> bool {
    let (op, task_type, task) = (state[0], state[1], state[2]);
    let in_place = match task_type {
        1 => state[5] != 0,
        2 => state[6] != 0,
        3 => state[8] != 0,
        _ => false,
    };
    (op == 0 && !(0..=3).contains(&task_type)) || (op == 1 && task == 0 && in_place)
}

fn compare(state: &[c_int; STATE]) -> String {
    let c = run(true, state);
    let rust = run(false, state);
    let at = format!("state={state:?}");
    assert_eq!(rust.0, c.0, "{at}");
    assert_eq!(rust.1, c.1, "{at}");
    assert_eq!(c.1[1], 1, "the C changed the core: {at}");
    assert_eq!(c.1[3], 0, "the C left an event live: {at}");
    c.0
}

/// The real-header values the port names.
fn values_match() {
    let mut values = [0 as c_int; 11];
    // SAFETY: the probe writes exactly eleven values.
    unsafe { oracle_task_queue_values(values.as_mut_ptr()) };
    assert_eq!(
        values,
        [
            obs_core::core_setup_rules::TASK_UI,
            obs_core::core_setup_rules::TASK_GRAPHICS,
            obs_core::core_setup_rules::TASK_AUDIO,
            obs_core::core_setup_rules::TASK_DESTROY,
            OS_EVENT_TYPE_AUTO,
            OS_EVENT_TYPE_MANUAL,
            LOG_ERROR,
            LOG_WARNING,
            size_of::<c_int>() as c_int,
            size_of::<ObsTask>() as c_int,
            (2 * size_of::<usize>()) as c_int,
        ]
    );
}

/// Every flag of `obs_in_task_thread()` for the four types.
fn in_task_thread_grid() {
    for task_type in 0..4 {
        for flags in 0..16 {
            for queue in 0..3 {
                for clobber in 0..2 {
                    let mut state = [0; STATE];
                    state[1] = task_type;
                    state[5] = flags & 1;
                    state[6] = flags >> 1 & 1;
                    state[7] = flags >> 2 & 1;
                    state[8] = flags >> 3;
                    state[12] = queue;
                    state[15] = clobber;
                    compare(&state);
                }
            }
        }
    }
}

/// `obs_queue_task()` over types, tasks, waits, flags, handlers, a failed
/// event, earlier tasks and clobbering.
fn queue_task_grid() {
    let mut i = 0u64;
    let mut waited = 0usize;
    let mut failed = 0usize;
    for task_type in [-1, 0, 1, 2, 3, 4, 99] {
        for task in 0..3 {
            for waits in 0..4 {
                for flags in 0..8 {
                    for handler in 0..3 {
                        for rest in 0..8 {
                            i += 1;
                            if !obs_c_oracle::parity::keep(i) {
                                continue;
                            }
                            let mut state = [0; STATE];
                            state[0] = 1;
                            state[1] = task_type;
                            state[2] = task;
                            state[3] = waits & 1;
                            state[17] = waits >> 1;
                            state[4] = if flags & 4 != 0 { 2 } else { -1 };
                            state[5] = flags & 1;
                            state[6] = flags >> 1 & 1;
                            state[7] = 1;
                            state[8] = flags & 1;
                            state[9] = handler;
                            state[13] = rest & 1;
                            state[16] = if rest & 2 != 0 { 7 } else { 0 };
                            state[15] = rest >> 2;
                            state[12] = (rest % 3) as c_int;
                            if excluded(&state) {
                                continue;
                            }
                            let log = compare(&state);
                            waited += usize::from(log.contains("wait_end"));
                            failed += usize::from(log.contains("Failed to create"));
                        }
                    }
                }
            }
        }
    }
    assert!(waited > 0, "no grid case waited");
    assert!(failed > 0, "no grid case failed its event");
}

/// `obs_wait_for_destroy_queue()` over the video and audio threads, a
/// failed event, its result, earlier tasks, the caller's thread and
/// clobbering.
fn wait_for_destroy_queue_grid() {
    for threads in 0..4 {
        for fail in 0..2 {
            for result in 0..2 {
                for pending in 0..8 {
                    for flags in 0..4 {
                        for clobber in 0..2 {
                            let mut state = [0; STATE];
                            state[0] = 2;
                            state[10] = threads & 1;
                            state[11] = threads >> 1;
                            state[13] = fail;
                            state[14] = result;
                            state[16] = pending;
                            state[5] = flags & 1;
                            state[6] = flags >> 1;
                            state[15] = clobber;
                            state[12] = pending % 3;
                            compare(&state);
                        }
                    }
                }
            }
        }
    }
}

type Random = (
    (c_int, c_int, c_int, c_int),
    (c_int, [c_int; 4]),
    (c_int, c_int, c_int, c_int),
    (c_int, c_int, c_int, c_int, c_int),
);

/// Every field at random.
fn random_states_match() {
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(512),
        failure_persistence: None,
        ..Default::default()
    });
    runner
        .run(
            &(
                (0..3, prop_oneof![-2..6, Just(99), any::<i32>()], 0..3, 0..2),
                (-1..4, prop::array::uniform4(0..2)),
                (0..3, 0..2, 0..2, 0..3),
                (0..4, 0..2, 0..2, 0..8, 0..2),
            ),
            |(
                (op, task_type, task, wait),
                (param, flags),
                (handler, video, audio, queue),
                (fail, result, clobber, pending, nested_wait),
            ): Random| {
                let task_type = if op == 0 {
                    task_type.rem_euclid(4)
                } else {
                    task_type
                };
                let state: [c_int; STATE] = [
                    op,
                    task_type,
                    task,
                    wait,
                    param,
                    flags[0],
                    flags[1],
                    flags[2],
                    flags[3],
                    handler,
                    video,
                    audio,
                    queue,
                    fail,
                    result,
                    clobber,
                    pending,
                    nested_wait,
                ];
                if !excluded(&state) {
                    compare(&state);
                }
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn original_c_calls_match() {
    // A todo!() or panic inside the extern "C" bridge aborts the process,
    // so the cases run in a child.
    const CHILD: &str = "OBS_TASK_QUEUE_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(format!(
                "{}::original_c_calls_match",
                module_path!().split_once("::").unwrap().1
            ))
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    values_match();
    in_task_thread_grid();
    queue_task_grid();
    wait_for_destroy_queue_grid();
    random_states_match();
}

/// The two paths that only a failed `os_event_init()` reaches, pinned on
/// their own: each logs its warning and polls.
#[test]
fn failed_event_polls_on_both_paths() {
    const CHILD: &str = "OBS_TASK_QUEUE_PARITY_FAILED_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(format!(
                "{}::failed_event_polls_on_both_paths",
                module_path!().split_once("::").unwrap().1
            ))
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let mut queue = [0; STATE];
    queue[0] = 1;
    queue[1] = 1;
    queue[2] = 1;
    queue[3] = 1;
    queue[4] = -1;
    queue[13] = 1;
    let log = compare(&queue);
    assert!(
        log.contains(
            "log 200 obs_queue_task: Failed to create the wait event, polling for the task instead;"
        ),
        "{log}"
    );
    assert!(
        log.contains("sleep 1;") && log.contains("task -1;"),
        "{log}"
    );

    let mut destroy = [0; STATE];
    destroy[0] = 2;
    destroy[10] = 1;
    destroy[11] = 1;
    destroy[13] = 1;
    let log = compare(&destroy);
    assert!(
        log.contains(
            "log 200 obs_wait_for_destroy_queue: Failed to create the wait event, polling instead;"
        ),
        "{log}"
    );
    assert_eq!(log.matches("sleep 1;").count(), 2, "{log}");

    let mut unknown = [0; STATE];
    unknown[0] = 1;
    unknown[1] = 99;
    unknown[2] = 1;
    let log = compare(&unknown);
    assert_eq!(
        log,
        "log 100 obs_queue_task: unknown task type 99, the task was not queued;"
    );
}
