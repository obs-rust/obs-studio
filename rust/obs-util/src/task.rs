//! Safe core for `libobs/util/task.c`.
//!
//! The C ABI shim lives in [`crate::ffi::task`]. The queue owns one worker
//! thread that runs tasks FIFO; [`TaskQueue::wait`] queues a marker behind
//! the pending tasks and blocks the caller until the worker reaches it.
//!
//! Reproduced from the C, including its quirks: a wait or stop marker found
//! with tasks still behind it is re-queued once (it yields to exactly one
//! task, not all of them), and `wait` reports whether a non-marker task ran
//! while the `waiting` flag was set — a racy value callers may not rely on.

use std::cell::Cell;
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::threading::{Event, EventType, Sem};

/// Boxed task, the safe equivalent of `os_task_t` + `void *param`.
pub type Task = Box<dyn FnOnce() + Send + 'static>;

// `static THREAD_LOCAL` in the C: one instance per thread, shared by every
// queue. `THREAD_ID` is only nonzero on a queue's worker thread;
// `EXIT_THREAD` is only ever set on a worker by its own stop marker.
thread_local! {
    static EXIT_THREAD: Cell<bool> = const { Cell::new(false) };
    static THREAD_ID: Cell<i64> = const { Cell::new(0) };
}

static THREAD_ID_COUNTER: AtomicI64 = AtomicI64::new(1);

/// `os_sem_init(&tq->sem, 0)`. The same semaphore the exported `os_sem_*`
/// calls use: `sem_t` or a Mach port on Unix, a Win32 semaphore on Windows.
fn sem_init_zero() -> Option<Pin<Box<Sem>>> {
    // `Sem::try_new` is the same failure channel the C's `os_sem_init`
    // check uses; on failure the C frees the queue and returns NULL.
    Sem::try_new(0)
}

/// A queued entry. `wait` and `stop` are distinct markers so the worker can
/// apply the C reorder rule (`ti.task == wait_for_thread` / `stop_thread`).
enum Entry {
    Task(Task),
    WaitMarker,
    StopMarker,
}

struct State {
    tasks: VecDeque<Entry>,
    waiting: bool,
    tasks_processed: bool,
    /// The worker found the queue empty and is (or is about to be) blocked
    /// on the semaphore. Only a push that clears it posts, so a burst of
    /// pushes costs one wakeup instead of one futex call each (#3216).
    worker_idle: bool,
}

struct Inner {
    id: i64,
    sem: Pin<Box<Sem>>,
    wait_event: Event,
    state: Mutex<State>,
}

/// `struct os_task_queue`.
pub struct TaskQueue {
    inner: Arc<Inner>,
    worker: Option<JoinHandle<()>>,
}

impl TaskQueue {
    /// `os_task_queue_create`.
    pub fn new() -> Option<Self> {
        // The C returns NULL on any init failure — os_sem_init,
        // os_event_init or pthread_create — after freeing the partial
        // queue. Event::new panics only if the kernel object cannot be
        // created. Sem::try_new and the thread spawn carry the C's NULL
        // channel here.
        let inner = Arc::new(Inner {
            id: THREAD_ID_COUNTER.fetch_add(1, Ordering::SeqCst),
            sem: sem_init_zero()?,
            wait_event: Event::new(EventType::Auto),
            state: Mutex::new(State {
                tasks: VecDeque::new(),
                waiting: false,
                tasks_processed: false,
                worker_idle: true,
            }),
        });

        let worker_inner = Arc::clone(&inner);
        let worker = std::thread::Builder::new()
            .name("tiny_tubular_task_thread".to_owned())
            .spawn(move || worker_loop(&worker_inner))
            .ok()?;

        Some(Self {
            inner,
            worker: Some(worker),
        })
    }

    /// `os_task_queue_queue_task`.
    pub fn queue_task(&self, task: Task) -> bool {
        self.inner.push(Entry::Task(task));
        true
    }

    /// `os_task_queue_wait`.
    pub fn wait(&self) -> bool {
        {
            let mut state = self.inner.state.lock().unwrap();
            state.waiting = true;
            state.tasks_processed = false;
        }
        self.inner.push(Entry::WaitMarker);
        self.inner.wait_event.wait();

        self.inner.state.lock().unwrap().tasks_processed
    }

    /// `os_task_queue_inside`.
    pub fn inside(&self) -> bool {
        THREAD_ID.with(|id| id.get() == self.inner.id)
    }
}

impl Inner {
    /// Queues `entry` and wakes the worker only if it went idle. The flag
    /// is read and cleared under the same lock the worker sets it under, so
    /// no wakeup is lost and at most one post is outstanding.
    fn push(&self, entry: Entry) {
        let wake = {
            let mut state = self.state.lock().unwrap();
            state.tasks.push_back(entry);
            std::mem::replace(&mut state.worker_idle, false)
        };
        if wake {
            self.sem.post();
        }
    }
}

fn worker_loop(inner: &Inner) {
    THREAD_ID.with(|id| id.set(inner.id));

    // `while (!exit_thread && os_sem_wait(tq->sem) == 0)`: an EINTR or a
    // destroyed semaphore ends the worker, same as the C.
    // Each wakeup drains the queue one entry at a time (the C's per-entry
    // order and marker rules are unchanged), then marks the worker idle.
    while !EXIT_THREAD.with(Cell::get) && inner.sem.wait() == 0 {
        while !EXIT_THREAD.with(Cell::get) && run_next(inner) {}
    }
}

/// Pops and runs one entry. False when the queue was empty; the worker is
/// then idle and the next push posts.
fn run_next(inner: &Inner) -> bool {
    {
        let entry = {
            let mut state = inner.state.lock().unwrap();
            if state.tasks.is_empty() {
                state.worker_idle = true;
                return false;
            }
            let mut entry = state.tasks.pop_front();
            // A marker with tasks still behind it is re-queued once and the
            // next entry runs first (faithful to the C push/pop pair).
            if !state.tasks.is_empty() && matches!(entry, Some(Entry::WaitMarker)) {
                state.tasks.push_back(entry.take().unwrap());
                entry = state.tasks.pop_front();
            }
            if !state.tasks.is_empty() && matches!(entry, Some(Entry::StopMarker)) {
                state.tasks.push_back(entry.take().unwrap());
                entry = state.tasks.pop_front();
            }
            if state.waiting {
                match entry {
                    Some(Entry::WaitMarker) => state.waiting = false,
                    _ => state.tasks_processed = true,
                }
            }
            entry
        };

        match entry {
            Some(Entry::Task(task)) => task(),
            Some(Entry::WaitMarker) => inner.wait_event.signal(),
            Some(Entry::StopMarker) => EXIT_THREAD.with(|exit| exit.set(true)),
            // Unreachable: the queue was checked non-empty under the lock.
            None => {}
        }
    }
    true
}

impl Drop for TaskQueue {
    /// `os_task_queue_destroy`: queue the stop marker behind any pending
    /// tasks, then join the worker. Pending tasks run to completion first.
    fn drop(&mut self) {
        self.inner.push(Entry::StopMarker);
        if let Some(worker) = self.worker.take() {
            // Joining self would deadlock; the C API has the same constraint
            // (pthread_join on the caller's own thread).
            let _ = worker.join();
        }
    }
}
