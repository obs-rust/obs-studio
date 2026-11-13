//! The graphics thread's task drain (`execute_graphics_tasks()`), over the
//! graphics task queue.
//!
//! `obs_graphics_thread_loop()` runs it once per frame, after the displays
//! render. `obs_queue_task(OBS_TASK_GRAPHICS, ...)` from another thread
//! appends to `obs->video.tasks` under `task_mutex`. The C takes the mutex,
//! and while the queue is not empty pops the oldest task, releases the mutex,
//! runs the task and takes the mutex again before it checks the queue once
//! more; so a task queued while another runs, by that task or by another
//! thread, runs in the same pass. The loop has no bound, as in the C. This
//! keeps the same order of calls.

/// The graphics task queue as the drain sees it: `task_mutex`,
/// `tasks.size`, a pop of the oldest `struct obs_task_info` and the call of
/// its task.
pub trait Queue {
    /// A popped task and its argument, owned until run.
    type Task;
    /// Take `task_mutex`.
    fn lock(&mut self);
    /// Release `task_mutex`.
    fn unlock(&mut self);
    /// Whether the queue holds a task (`tasks.size != 0`), under the mutex.
    fn pending(&mut self) -> bool;
    /// Pop the oldest task, under the mutex.
    fn pop(&mut self) -> Self::Task;
    /// Run a popped task, with the mutex released.
    fn run(&mut self, task: Self::Task);
}

/// Run every queued task, oldest first, including tasks queued meanwhile.
pub fn execute(queue: &mut impl Queue) {
    queue.lock();
    while queue.pending() {
        let task = queue.pop();
        queue.unlock();
        queue.run(task);
        queue.lock();
    }
    queue.unlock();
}
