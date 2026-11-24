//! Tier 1 for `obs-core::task_queue` (`libobs/obs-task-queue.c.h`): every
//! `test_task_queue_entry_points` case over a model of the libobs threads,
//! plus what the C test cannot reach: the UI routes, a failed event on both
//! wait paths, the video-only and audio-only destroy waits, the unknown type,
//! the call order and the NULL task.
//!
//! The model plays each thread: a pushed or queued task waits in its queue
//! until a wait or a sleep on the calling thread lets that queue's thread
//! run it (oldest first), with the model's current thread set to it while
//! it runs, so the thread flags, the UI mark and `os_task_queue_inside()`
//! answer as they would on that thread. Every read and call is logged.
use std::collections::VecDeque;
use std::sync::atomic::Ordering;

use obs_core::core_setup_rules::{TASK_AUDIO, TASK_DESTROY, TASK_GRAPHICS, TASK_UI};
use obs_core::task_queue::{
    self as tq, Host, LOG_ERROR, LOG_WARNING, Locked, Log, OS_EVENT_TYPE_AUTO,
    OS_EVENT_TYPE_MANUAL, Waiter,
};

const UNKNOWN: i32 = 99;
const QUEUES: [i32; 3] = [TASK_GRAPHICS, TASK_AUDIO, TASK_DESTROY];
const DESTRUCTION: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Thread {
    Test,
    Other,
    Graphics,
    Audio,
    Destroy,
}

fn thread_of(task_type: i32) -> Thread {
    match task_type {
        TASK_GRAPHICS => Thread::Graphics,
        TASK_AUDIO => Thread::Audio,
        TASK_DESTROY => Thread::Destroy,
        _ => unreachable!(),
    }
}

fn queue_index(task_type: i32) -> usize {
    match task_type {
        TASK_GRAPHICS => 0,
        TASK_AUDIO => 1,
        _ => 2,
    }
}

type Wait = Waiter<Job, usize>;

/// A task and its param.
#[derive(Clone, Copy, Debug)]
enum Job {
    /// Records what it saw in `seen[i]`.
    Record(usize),
    /// Queues `Record(inner)` to its own thread's type, with `wait`.
    Nested {
        task_type: i32,
        inner: usize,
        wait: bool,
        out: usize,
    },
    /// `task_wait_callback` with this waiter.
    Wait(*const Wait),
    /// A NULL `obs_task_t`.
    Null,
}

#[derive(Clone, Copy, Debug, Default)]
struct Seen {
    runs: u32,
    thread: Option<Thread>,
    /// `obs_in_task_thread()` for each of [`QUEUES`], read inside the task.
    inside: [bool; 3],
}

#[derive(Clone, Copy, Debug, Default)]
struct NestedOut {
    finished: bool,
    inner_ran_before_return: bool,
    same_thread: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Handler {
    /// Runs the task in place (`inline_ui_handler`).
    Inline,
    /// Only records the call.
    Record,
}

#[derive(Clone, Copy, Debug)]
struct Event {
    manual: bool,
    signaled: bool,
    live: bool,
}

struct Model {
    current: Thread,
    ui_mark: Option<Thread>,
    handler: Option<Handler>,
    queues: [VecDeque<Job>; 3],
    events: Vec<Event>,
    fail_inits: u32,
    inits: u32,
    video_initialized: bool,
    audio: bool,
    locked: Option<Locked>,
    seen: Vec<Seen>,
    nested: Vec<NestedOut>,
    calls: Vec<String>,
    logs: Vec<(i32, String)>,
}

impl Model {
    /// A live core: video and audio threads up, no UI handler, no mark.
    fn new() -> Self {
        Self {
            current: Thread::Test,
            ui_mark: None,
            handler: None,
            queues: Default::default(),
            events: Vec::new(),
            fail_inits: 0,
            inits: 0,
            video_initialized: true,
            audio: true,
            locked: None,
            seen: vec![Seen::default(); 8],
            nested: vec![NestedOut::default(); 4],
            calls: Vec::new(),
            logs: Vec::new(),
        }
    }

    fn call(&mut self, what: impl Into<String>) {
        self.calls.push(what.into());
    }

    fn pending(&self) -> usize {
        self.queues.iter().map(VecDeque::len).sum()
    }

    fn run_on(&mut self, thread: Thread, job: Job) {
        let was = std::mem::replace(&mut self.current, thread);
        self.run(job);
        self.current = was;
    }

    /// Lets the thread of queue `i` run its oldest task.
    fn run_queue(&mut self, i: usize) -> bool {
        let Some(job) = self.queues[i].pop_front() else {
            return false;
        };
        let thread = [Thread::Graphics, Thread::Audio, Thread::Destroy][i];
        self.run_on(thread, job);
        true
    }

    /// Lets the other threads run one task each, graphics first.
    fn run_any(&mut self) -> bool {
        (0..3).any(|i| self.run_queue(i))
    }

    /// `drain()` of the C test: everything queued to the type has run.
    fn drain(&mut self, task_type: i32) {
        while self.run_queue(queue_index(task_type)) {}
    }

    fn live_events(&self) -> usize {
        self.events.iter().filter(|e| e.live).count()
    }

    fn reads(&self, what: &str) -> usize {
        self.calls.iter().filter(|c| c.starts_with(what)).count()
    }
}

impl Host for Model {
    type Task = Job;
    type Event = usize;
    type Queue = usize;

    fn graphics_thread(&mut self) -> bool {
        self.call("graphics_thread");
        self.current == Thread::Graphics
    }
    fn audio_thread(&mut self) -> bool {
        self.call("audio_thread");
        self.current == Thread::Audio
    }
    fn ui_thread(&mut self) -> bool {
        self.call("ui_thread");
        self.ui_mark == Some(self.current)
    }
    fn destruction(&mut self) -> usize {
        self.call("destruction");
        DESTRUCTION
    }
    fn inside(&mut self, queue: usize) -> bool {
        self.call(format!("inside {queue}"));
        self.current == Thread::Destroy && queue == DESTRUCTION
    }
    fn has_ui_handler(&mut self) -> bool {
        self.call("has_ui_handler");
        self.handler.is_some()
    }
    fn call_ui_handler(&mut self, task: Job, wait: bool) {
        self.call(format!("ui_handler {wait}"));
        match self.handler.expect("called without a handler") {
            Handler::Inline => self.run(task),
            Handler::Record => {}
        }
    }
    fn run(&mut self, task: Job) {
        self.call(format!("run {:?}", self.current));
        match task {
            Job::Record(i) => {
                let inside = QUEUES.map(|t| tq::in_task_thread(self, t));
                let seen = &mut self.seen[i];
                seen.runs += 1;
                seen.thread = Some(self.current);
                seen.inside = inside;
            }
            Job::Nested {
                task_type,
                inner,
                wait,
                out,
            } => {
                tq::queue_task(self, task_type, Job::Record(inner), wait);
                let ran = self.seen[inner].runs == 1;
                self.nested[out] = NestedOut {
                    finished: true,
                    inner_ran_before_return: ran,
                    same_thread: ran && self.seen[inner].thread == Some(self.current),
                };
            }
            Job::Wait(waiter) => {
                // SAFETY: the waiter waits on its stack until this sets its
                // flag or signals its event.
                let waiter = unsafe { &*waiter };
                tq::task_wait_callback(self, waiter.task, waiter.event, || {
                    waiter.done.store(true, Ordering::SeqCst);
                });
            }
            Job::Null => {}
        }
    }
    fn log(&mut self, log: Log) {
        self.call("log");
        self.logs.push((log.level(), log.message()));
    }
    fn event_init(&mut self, kind: i32) -> (bool, Option<usize>) {
        self.call(format!("event_init {kind}"));
        let n = self.inits;
        self.inits += 1;
        if self.fail_inits & (1 << n) != 0 {
            return (true, None);
        }
        self.events.push(Event {
            manual: kind == OS_EVENT_TYPE_MANUAL,
            signaled: false,
            live: true,
        });
        (false, Some(self.events.len() - 1))
    }
    fn event_signal(&mut self, event: usize) {
        self.call(format!("signal {event}"));
        assert!(self.events[event].live);
        self.events[event].signaled = true;
    }
    fn event_wait(&mut self, event: usize) {
        self.call(format!("wait {event}"));
        while !self.events[event].signaled {
            assert!(self.run_any(), "waiting for an event nothing will signal");
        }
        if !self.events[event].manual {
            self.events[event].signaled = false;
        }
    }
    fn event_destroy(&mut self, event: Option<usize>) {
        self.call(format!("event_destroy {event:?}"));
        if let Some(event) = event {
            assert!(self.events[event].live);
            self.events[event].live = false;
        }
    }
    fn sleep_ms(&mut self, ms: u32) {
        self.call(format!("sleep {ms}"));
        assert!(self.run_any(), "polling for a task nothing will run");
    }
    fn lock(&mut self, queue: Locked) {
        self.call(format!("lock {queue:?}"));
        assert_eq!(self.locked.replace(queue), None);
    }
    fn push(&mut self, queue: Locked, task: Job) {
        self.call(format!("push {queue:?}"));
        assert_eq!(self.locked, Some(queue), "pushed without its lock");
        self.queues[queue as usize].push_back(task);
    }
    fn unlock(&mut self, queue: Locked) {
        self.call(format!("unlock {queue:?}"));
        assert_eq!(self.locked.take(), Some(queue));
    }
    fn queue_destroy(&mut self, queue: usize, task: Job) {
        self.call(format!("queue_destroy {queue}"));
        self.queues[2].push_back(task);
    }
    fn wait_destroy(&mut self, queue: usize) -> bool {
        self.call(format!("wait_destroy {queue}"));
        // os_task_queue_wait(): whether a task ran after the wait was queued.
        let mut processed = false;
        while self.run_queue(2) {
            processed = true;
        }
        processed
    }
    fn video_thread_initialized(&mut self) -> bool {
        self.call("video_thread_initialized");
        self.video_initialized
    }
    fn has_audio(&mut self) -> bool {
        self.call("has_audio");
        self.audio
    }
    fn wait_task(&mut self, waiter: &Wait) -> Job {
        Job::Wait(std::ptr::from_ref(waiter))
    }
}

fn expect_on_own_thread(m: &Model, i: usize, task_type: i32) {
    let seen = m.seen[i];
    assert_eq!(seen.runs, 1);
    assert_eq!(seen.thread, Some(thread_of(task_type)));
    for (j, &t) in QUEUES.iter().enumerate() {
        assert_eq!(
            seen.inside[j],
            t == task_type,
            "obs_in_task_thread({t}) inside a {task_type} task"
        );
    }
}

// --- test_task_queue_entry_points ------------------------------------------

#[test]
fn not_in_task_thread_on_test_thread() {
    let mut m = Model::new();
    for t in [TASK_GRAPHICS, TASK_AUDIO, TASK_DESTROY, TASK_UI] {
        assert!(!tq::in_task_thread(&mut m, t), "type {t}");
    }
}

#[test]
fn ui_follows_registered_ui_thread() {
    let mut m = Model::new();
    m.ui_mark = Some(Thread::Test);
    assert!(tq::in_task_thread(&mut m, TASK_UI));
    m.current = Thread::Other;
    assert!(!tq::in_task_thread(&mut m, TASK_UI));
    m.current = Thread::Test;
    for t in QUEUES {
        assert!(!tq::in_task_thread(&mut m, t));
    }
}

#[test]
fn queue_without_wait_runs_once_on_type_thread() {
    for (i, t) in QUEUES.into_iter().enumerate() {
        let mut m = Model::new();
        tq::queue_task(&mut m, t, Job::Record(i), false);
        assert_eq!(m.seen[i].runs, 0, "ran on the caller's thread");
        assert_eq!(m.pending(), 1);
        m.drain(t);
        expect_on_own_thread(&m, i, t);
    }
}

#[test]
fn queue_with_wait_returns_after_task_ran() {
    for (i, t) in QUEUES.into_iter().enumerate() {
        let mut m = Model::new();
        m.current = Thread::Other;
        tq::queue_task(&mut m, t, Job::Record(i), true);
        // returned only after the task ran, on its own thread
        expect_on_own_thread(&m, i, t);
        assert_eq!(m.pending(), 0);
        assert_eq!(m.live_events(), 0);
        assert!(m.events[0].manual);
    }
}

#[test]
fn queue_from_own_thread_runs_inline() {
    for wait in [true, false] {
        for (i, t) in QUEUES.into_iter().enumerate() {
            let mut m = Model::new();
            let job = Job::Nested {
                task_type: t,
                inner: i,
                wait,
                out: 0,
            };
            tq::queue_task(&mut m, t, job, false);
            m.drain(t);
            let out = m.nested[0];
            assert!(out.finished);
            assert!(
                out.inner_ran_before_return && out.same_thread,
                "type {t} wait {wait}: {out:?}"
            );
            expect_on_own_thread(&m, i, t);
            // inline: no event, nothing left behind
            assert_eq!(m.inits, 0);
            assert_eq!(m.pending(), 0);
        }
    }
}

#[test]
fn unknown_type_queues_nothing() {
    for wait in [false, true] {
        let mut m = Model::new();
        tq::queue_task(&mut m, UNKNOWN, Job::Record(0), wait);
        for t in QUEUES {
            m.drain(t);
        }
        assert_eq!(m.seen[0].runs, 0);
        assert_eq!(
            m.logs,
            [(
                LOG_ERROR,
                "obs_queue_task: unknown task type 99, the task was not queued".to_string()
            )]
        );
        // routed at the type step: no handler, thread or wait read
        assert_eq!(m.calls, ["log"]);
    }
}

fn slow_jobs(m: &mut Model) {
    tq::queue_task(m, TASK_GRAPHICS, Job::Record(0), false);
    tq::queue_task(m, TASK_AUDIO, Job::Record(1), false);
    tq::queue_task(m, TASK_DESTROY, Job::Record(2), false);
}

#[test]
fn wait_for_destroy_queue_runs_pending_tasks() {
    let mut m = Model::new();
    slow_jobs(&mut m);
    m.current = Thread::Other;
    tq::wait_for_destroy_queue(&mut m);
    for (i, t) in QUEUES.into_iter().enumerate() {
        expect_on_own_thread(&m, i, t);
    }
    assert_eq!(m.pending(), 0);
}

#[test]
fn wait_for_destroy_queue_idle_returns_false() {
    let mut m = Model::new();
    assert!(!tq::wait_for_destroy_queue(&mut m));
}

#[test]
fn wait_for_destroy_queue_true_after_destroy_task_ran() {
    let mut m = Model::new();
    tq::queue_task(&mut m, TASK_DESTROY, Job::Record(0), false);
    m.current = Thread::Other;
    assert!(tq::wait_for_destroy_queue(&mut m));
    expect_on_own_thread(&m, 0, TASK_DESTROY);
}

// --- beyond the C test ------------------------------------------------------

#[test]
fn ui_task_goes_to_the_handler_and_never_asks_the_thread() {
    for wait in [false, true] {
        let mut m = Model::new();
        m.handler = Some(Handler::Record);
        m.ui_mark = Some(Thread::Test);
        tq::queue_task(&mut m, TASK_UI, Job::Record(0), wait);
        assert_eq!(
            m.calls,
            ["has_ui_handler", format!("ui_handler {wait}").as_str()]
        );
        assert_eq!(m.seen[0].runs, 0);
        assert_eq!(m.inits, 0);
    }
}

#[test]
fn ui_task_with_inline_handler_runs_in_place() {
    let mut m = Model::new();
    m.handler = Some(Handler::Inline);
    tq::queue_task(&mut m, TASK_UI, Job::Record(0), true);
    assert_eq!(m.seen[0].runs, 1);
    assert_eq!(m.seen[0].thread, Some(Thread::Test));
}

#[test]
fn ui_task_without_handler_logs_error() {
    for wait in [false, true] {
        let mut m = Model::new();
        tq::queue_task(&mut m, TASK_UI, Job::Record(0), wait);
        assert_eq!(
            m.logs,
            [(
                LOG_ERROR,
                "UI task could not be queued, there's no UI task handler!".to_string()
            )]
        );
        assert_eq!(m.calls, ["has_ui_handler", "log"]);
        assert_eq!(m.seen[0].runs, 0);
    }
}

#[test]
fn queue_wait_order() {
    let mut m = Model::new();
    m.current = Thread::Other;
    tq::queue_task(&mut m, TASK_GRAPHICS, Job::Record(0), true);
    assert_eq!(
        m.calls,
        [
            "graphics_thread",
            "event_init 1",
            "graphics_thread",
            "lock Graphics",
            "push Graphics",
            "unlock Graphics",
            "wait 0",
            "run Graphics",
            "run Graphics",
            "graphics_thread",
            "audio_thread",
            "destruction",
            "inside 7",
            "signal 0",
            "event_destroy Some(0)",
        ]
    );
}

#[test]
fn queue_wait_with_failed_event_polls() {
    for t in QUEUES {
        let mut m = Model::new();
        m.current = Thread::Other;
        m.fail_inits = 1;
        tq::queue_task(&mut m, t, Job::Record(0), true);
        expect_on_own_thread(&m, 0, t);
        assert_eq!(
            m.logs,
            [(
                LOG_WARNING,
                "obs_queue_task: Failed to create the wait event, polling for the task instead"
                    .to_string()
            )]
        );
        assert_eq!(m.reads("sleep 1"), 1);
        assert_eq!(m.calls.last().unwrap(), "event_destroy None");
        assert_eq!(m.reads("signal"), 0);
    }
}

#[test]
fn destroy_wait_order() {
    let mut m = Model::new();
    m.current = Thread::Other;
    assert!(!tq::wait_for_destroy_queue(&mut m));
    assert_eq!(
        m.calls,
        [
            "video_thread_initialized",
            "has_audio",
            "event_init 0",
            "graphics_thread",
            "lock Graphics",
            "push Graphics",
            "unlock Graphics",
            "wait 0",
            "run Graphics",
            "signal 0",
            "audio_thread",
            "lock Audio",
            "push Audio",
            "unlock Audio",
            "wait 0",
            "run Audio",
            "signal 0",
            "event_destroy Some(0)",
            "destruction",
            "wait_destroy 7",
        ]
    );
}

#[test]
fn destroy_wait_with_failed_event_polls_each_thread() {
    let mut m = Model::new();
    tq::queue_task(&mut m, TASK_AUDIO, Job::Record(1), false);
    m.current = Thread::Other;
    m.fail_inits = 1;
    tq::wait_for_destroy_queue(&mut m);
    assert_eq!(
        m.logs,
        [(
            LOG_WARNING,
            "obs_wait_for_destroy_queue: Failed to create the wait event, polling instead"
                .to_string()
        )]
    );
    // done is cleared after the video wait, so the audio wait polls again
    // and returns only after the audio queue ran past the earlier task
    assert!(m.reads("sleep 1") >= 2);
    expect_on_own_thread(&m, 1, TASK_AUDIO);
    assert_eq!(m.pending(), 0);
    assert_eq!(m.calls.iter().rev().nth(2).unwrap(), "event_destroy None");
}

#[test]
fn destroy_wait_each_thread_on_its_own() {
    for (video, audio) in [(true, false), (false, true), (false, false)] {
        let mut m = Model::new();
        m.video_initialized = video;
        m.audio = audio;
        m.current = Thread::Other;
        tq::wait_for_destroy_queue(&mut m);
        assert_eq!(m.reads("push Graphics"), usize::from(video));
        assert_eq!(m.reads("push Audio"), usize::from(audio));
        assert_eq!(m.inits, u32::from(video || audio));
        assert_eq!(m.calls.last().unwrap(), "wait_destroy 7");
        assert_eq!(m.live_events(), 0);
    }
}

#[test]
fn destroy_wait_from_the_graphics_thread_runs_inline() {
    let mut m = Model::new();
    m.current = Thread::Graphics;
    m.audio = false;
    tq::wait_for_destroy_queue(&mut m);
    // the callback ran in place and signalled before the wait
    assert_eq!(m.reads("push"), 0);
    assert_eq!(m.reads("signal 0"), 1);
    assert_eq!(m.reads("wait 0"), 1);
}

#[test]
fn pushes_take_their_lock() {
    for (t, queue) in [(TASK_GRAPHICS, "Graphics"), (TASK_AUDIO, "Audio")] {
        let mut m = Model::new();
        tq::queue_task(&mut m, t, Job::Record(0), false);
        let tail = &m.calls[m.calls.len() - 3..];
        assert_eq!(
            tail,
            [
                format!("lock {queue}"),
                format!("push {queue}"),
                format!("unlock {queue}")
            ]
        );
    }
    let mut m = Model::new();
    tq::queue_task(&mut m, TASK_DESTROY, Job::Record(0), false);
    assert_eq!(
        m.calls,
        ["destruction", "inside 7", "destruction", "queue_destroy 7"]
    );
}

#[test]
fn null_task_in_place_does_nothing() {
    // Intentional difference: the C calls NULL.
    let mut m = Model::new();
    m.current = Thread::Graphics;
    tq::queue_task(&mut m, TASK_GRAPHICS, Job::Null, false);
    assert_eq!(m.calls, ["graphics_thread", "run Graphics"]);
    // a NULL task still waits through the queue on another thread
    let mut m = Model::new();
    tq::queue_task(&mut m, TASK_AUDIO, Job::Null, true);
    assert_eq!(m.reads("signal 0"), 1);
    assert_eq!(m.live_events(), 0);
}

#[test]
fn in_task_thread_unknown_is_false_and_reads_nothing() {
    for t in [-1, 4, UNKNOWN, i32::MIN, i32::MAX] {
        let mut m = Model::new();
        m.current = Thread::Graphics;
        assert!(!tq::in_task_thread(&mut m, t));
        assert!(m.calls.is_empty());
    }
}

#[test]
fn log_formats_and_levels() {
    assert_eq!((OS_EVENT_TYPE_AUTO, OS_EVENT_TYPE_MANUAL), (0, 1));
    assert_eq!(Log::UiNone.level(), LOG_ERROR);
    assert_eq!(Log::Unknown(3).level(), LOG_ERROR);
    assert_eq!(Log::QueueEventFailed.level(), LOG_WARNING);
    assert_eq!(Log::DestroyEventFailed.level(), LOG_WARNING);
    assert_eq!(
        Log::Unknown(-7).format(),
        c"obs_queue_task: unknown task type %d, the task was not queued"
    );
    assert_eq!(
        Log::Unknown(-7).message(),
        "obs_queue_task: unknown task type -7, the task was not queued"
    );
}
