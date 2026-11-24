//! Safe core of `libobs/obs-task-queue.c.h`: the task queue entry points
//! `obs_in_task_thread()`, `obs_queue_task()` and
//! `obs_wait_for_destroy_queue()`, and the `task_wait_callback()` /
//! `task_wait_for_callback()` pair they share.
//!
//! The route is [`crate::core_setup_rules`]'s (`task_thread`, `task_step`);
//! this is the orchestration around it, over [`Host`], with the C's reads and
//! calls in the C's order:
//!
//! - [`in_task_thread`]: the graphics or audio thread flag, the UI thread
//!   mark, or whether the caller is inside the destruction task queue; any
//!   other type is false (the C asserts, which release builds compile out).
//! - [`queue_task`]: the step walk, each value read only on the step that
//!   asks for it (a UI task reads only the handler and never asks
//!   [`in_task_thread`]), then the route: the UI handler call, the
//!   missing-handler error, the task run in place, the wait path (a manual
//!   event; when it cannot be created, the warning and polling instead), the
//!   graphics and audio pushes under their task locks, the destroy push, or
//!   the unknown-type error.
//! - [`wait_for_destroy_queue`]: when the video thread is initialized and
//!   when audio exists, each on its own (#5405), one auto event shared by
//!   both waits, then the destruction queue's wait and its result.
//!
//! One intentional difference: a task routed to run in place with no
//! function (a NULL `obs_task_t`) does nothing, where the C calls NULL.
//! [`Host::run`] carries it.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::core_setup_rules::{
    self as rules, TASK_AUDIO, TASK_DESTROY, TASK_GRAPHICS, TASK_UI, Task as Step,
};

/// `LOG_ERROR` (`util/base.h`).
pub const LOG_ERROR: i32 = 100;
/// `LOG_WARNING`.
pub const LOG_WARNING: i32 = 200;

/// `OS_EVENT_TYPE_AUTO` (`enum os_event_type`, `util/threading.h`).
pub const OS_EVENT_TYPE_AUTO: i32 = 0;
/// `OS_EVENT_TYPE_MANUAL`.
pub const OS_EVENT_TYPE_MANUAL: i32 = 1;

/// A message the entry points log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Log {
    /// A UI task with no UI task handler.
    UiNone,
    /// `obs_queue_task(.., wait true)` could not create its event.
    QueueEventFailed,
    /// `obs_wait_for_destroy_queue()` could not create its event.
    DestroyEventFailed,
    /// A type outside the four, with its value.
    Unknown(i32),
}

impl Log {
    /// The level the C logs it at.
    #[must_use]
    pub const fn level(self) -> i32 {
        match self {
            Self::UiNone | Self::Unknown(_) => LOG_ERROR,
            Self::QueueEventFailed | Self::DestroyEventFailed => LOG_WARNING,
        }
    }

    /// The C's `blog()` format, which takes the type for [`Log::Unknown`]
    /// and no argument otherwise.
    #[must_use]
    pub const fn format(self) -> &'static core::ffi::CStr {
        match self {
            Self::UiNone => c"UI task could not be queued, there's no UI task handler!",
            Self::QueueEventFailed => {
                c"obs_queue_task: Failed to create the wait event, polling for the task instead"
            }
            Self::DestroyEventFailed => {
                c"obs_wait_for_destroy_queue: Failed to create the wait event, polling instead"
            }
            Self::Unknown(_) => c"obs_queue_task: unknown task type %d, the task was not queued",
        }
    }

    /// The message as `blog()` formats it.
    #[must_use]
    pub fn message(self) -> String {
        match self {
            Self::Unknown(task_type) => {
                format!("obs_queue_task: unknown task type {task_type}, the task was not queued")
            }
            _ => self.format().to_string_lossy().into_owned(),
        }
    }
}

/// A task queue the C pushes to under its lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locked {
    /// `obs->video.task_mutex` and `obs->video.tasks`.
    Graphics,
    /// `obs->audio.task_mutex` and `obs->audio.tasks`.
    Audio,
}

/// `struct task_wait_info`: the task a wait runs (none for
/// [`wait_for_destroy_queue`]), the event it signals, and the flag it sets
/// instead when there is no event.
#[derive(Debug)]
pub struct Waiter<T, E> {
    /// The waited task and its param.
    pub task: Option<T>,
    /// The wait event, or `None` when `os_event_init()` failed.
    pub event: Option<E>,
    /// Set instead of signalling the event when there is none.
    pub done: AtomicBool,
}

impl<T, E> Waiter<T, E> {
    /// A waiter with `done` clear, as the C's initializer.
    pub const fn new(task: Option<T>, event: Option<E>) -> Self {
        Self {
            task,
            event,
            done: AtomicBool::new(false),
        }
    }
}

/// The reads and calls of the entry points, one C expression each.
pub trait Host {
    /// A task and its param (`obs_task_t` and `void *`).
    type Task: Copy;
    /// A live `os_event_t *`.
    type Event: Copy;
    /// An `os_task_queue_t *` as libobs holds it.
    type Queue: Copy;

    /// `is_graphics_thread`.
    fn graphics_thread(&mut self) -> bool;
    /// `is_audio_thread`.
    fn audio_thread(&mut self) -> bool;
    /// `obs_core_is_ui_thread()`.
    fn ui_thread(&mut self) -> bool;
    /// `obs->destruction_task_thread`.
    fn destruction(&mut self) -> Self::Queue;
    /// `os_task_queue_inside(queue)`.
    fn inside(&mut self, queue: Self::Queue) -> bool;
    /// `obs->ui_task_handler != NULL`.
    fn has_ui_handler(&mut self) -> bool;
    /// `obs->ui_task_handler(task, param, wait)`.
    fn call_ui_handler(&mut self, task: Self::Task, wait: bool);
    /// `task(param)`; nothing for a NULL task.
    fn run(&mut self, task: Self::Task);
    /// `blog(log.level(), log.format(), ...)`.
    fn log(&mut self, log: Log);
    /// `os_event_init(&event, kind)`: whether it failed (non-zero), and the
    /// event as it left it (`None` when it wrote nothing).
    fn event_init(&mut self, kind: i32) -> (bool, Option<Self::Event>);
    /// `os_event_signal(event)`.
    fn event_signal(&mut self, event: Self::Event);
    /// `os_event_wait(event)`.
    fn event_wait(&mut self, event: Self::Event);
    /// `os_event_destroy(event)`, NULL included.
    fn event_destroy(&mut self, event: Option<Self::Event>);
    /// `os_sleep_ms(ms)`.
    fn sleep_ms(&mut self, ms: u32);
    /// `pthread_mutex_lock(&<queue>->task_mutex)`.
    fn lock(&mut self, queue: Locked);
    /// `deque_push_back(&<queue>->tasks, &info, sizeof(info))`.
    fn push(&mut self, queue: Locked, task: Self::Task);
    /// `pthread_mutex_unlock(&<queue>->task_mutex)`.
    fn unlock(&mut self, queue: Locked);
    /// `os_task_queue_queue_task(queue, (os_task_t)task, param)`, its result
    /// ignored as the C ignores it.
    fn queue_destroy(&mut self, queue: Self::Queue, task: Self::Task);
    /// `os_task_queue_wait(queue)`.
    fn wait_destroy(&mut self, queue: Self::Queue) -> bool;
    /// `obs->video.thread_initialized`.
    fn video_thread_initialized(&mut self) -> bool;
    /// `obs->audio.audio != NULL`.
    fn has_audio(&mut self) -> bool;
    /// The task `task_wait_callback` with `waiter` as its param: it calls
    /// [`task_wait_callback`] with the waiter's task and event and a setter
    /// of its `done` flag. `waiter` outlives the task, since the caller
    /// waits for it before it returns.
    fn wait_task(&mut self, waiter: &Waiter<Self::Task, Self::Event>) -> Self::Task;
}

/// `obs_in_task_thread(type)`.
pub fn in_task_thread<H: Host>(host: &mut H, task_type: i32) -> bool {
    match rules::task_thread(task_type) {
        TASK_GRAPHICS => host.graphics_thread(),
        TASK_AUDIO => host.audio_thread(),
        TASK_UI => host.ui_thread(),
        TASK_DESTROY => {
            let queue = host.destruction();
            host.inside(queue)
        }
        _ => false,
    }
}

/// `task_wait_callback(info)`: the task, if any, then the event signalled,
/// or `set_done` without an event. Nothing of the waiter is read after the
/// signal or the flag, since the waiting thread may then return.
pub fn task_wait_callback<H: Host>(
    host: &mut H,
    task: Option<H::Task>,
    event: Option<H::Event>,
    set_done: impl FnOnce(),
) {
    if let Some(task) = task {
        host.run(task);
    }
    match event {
        Some(event) => host.event_signal(event),
        None => set_done(),
    }
}

/// `task_wait_for_callback(info)`: the event's wait, or, without one,
/// polling `done` every millisecond and clearing it after.
pub fn task_wait_for_callback<H: Host>(host: &mut H, event: Option<H::Event>, done: &AtomicBool) {
    if let Some(event) = event {
        host.event_wait(event);
        return;
    }
    while !done.load(Ordering::SeqCst) {
        host.sleep_ms(1);
    }
    done.store(false, Ordering::SeqCst);
}

/// The walk of [`rules::task_step`] as `obs_queue_task()` makes it: each
/// value is read only on the step that asks for it.
fn route<H: Host>(host: &mut H, task_type: i32, wait: bool) -> Step {
    let mut step = rules::task_step(Step::Type, task_type, false);
    if step == Step::UiHandler {
        let value = host.has_ui_handler();
        step = rules::task_step(step, task_type, value);
    }
    if step == Step::InThread {
        let value = in_task_thread(host, task_type);
        step = rules::task_step(step, task_type, value);
    }
    if step == Step::Wait {
        step = rules::task_step(step, task_type, wait);
    }
    step
}

fn push<H: Host>(host: &mut H, queue: Locked, task: H::Task) {
    host.lock(queue);
    host.push(queue, task);
    host.unlock(queue);
}

/// `obs_queue_task(type, task, param, wait)`.
pub fn queue_task<H: Host>(host: &mut H, task_type: i32, task: H::Task, wait: bool) {
    match route(host, task_type, wait) {
        Step::UiCall => host.call_ui_handler(task, wait),
        Step::UiNone => host.log(Log::UiNone),
        Step::Run => host.run(task),
        Step::WaitQueue => {
            let (failed, event) = host.event_init(OS_EVENT_TYPE_MANUAL);
            if failed {
                host.log(Log::QueueEventFailed);
            }
            let waiter = Waiter::new(Some(task), event);
            let wrapped = host.wait_task(&waiter);
            queue_task(host, task_type, wrapped, false);
            task_wait_for_callback(host, waiter.event, &waiter.done);
            host.event_destroy(waiter.event);
        }
        Step::PushGraphics => push(host, Locked::Graphics, task),
        Step::PushAudio => push(host, Locked::Audio, task),
        Step::PushDestroy => {
            let queue = host.destruction();
            host.queue_destroy(queue, task);
        }
        Step::Unknown => host.log(Log::Unknown(task_type)),
        Step::Drop | Step::Type | Step::UiHandler | Step::InThread | Step::Wait => {}
    }
}

/// `obs_wait_for_destroy_queue()`.
pub fn wait_for_destroy_queue<H: Host>(host: &mut H) -> bool {
    let wait_video = host.video_thread_initialized();
    let wait_audio = host.has_audio();
    if wait_video || wait_audio {
        let (failed, event) = host.event_init(OS_EVENT_TYPE_AUTO);
        if failed {
            host.log(Log::DestroyEventFailed);
        }
        let waiter = Waiter::new(None, event);
        if wait_video {
            let wrapped = host.wait_task(&waiter);
            queue_task(host, TASK_GRAPHICS, wrapped, false);
            task_wait_for_callback(host, waiter.event, &waiter.done);
        }
        if wait_audio {
            let wrapped = host.wait_task(&waiter);
            queue_task(host, TASK_AUDIO, wrapped, false);
            task_wait_for_callback(host, waiter.event, &waiter.done);
        }
        host.event_destroy(waiter.event);
    }

    let queue = host.destruction();
    host.wait_destroy(queue)
}
