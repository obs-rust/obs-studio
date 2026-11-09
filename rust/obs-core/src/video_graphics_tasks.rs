//! Graphics task drain; the adapter owns the C queue, mutex and callback ABI.

/// Operations on the graphics queue. Queue reads and pops require the lock.
pub trait Queue {
    /// A popped callback and its argument, owned until run.
    type Task;
    /// Acquire the queue mutex.
    fn lock(&mut self);
    /// Release the queue mutex.
    fn unlock(&mut self);
    /// Whether the current queue contains work.
    fn pending(&mut self) -> bool;
    /// Remove the oldest task while locked.
    fn pop(&mut self) -> Self::Task;
    /// Invoke the task without the queue mutex held.
    fn run(&mut self, task: Self::Task);
}

/// Drain the live queue, including work producers append during callbacks.
pub fn execute<Q: Queue>(queue: &mut Q) {
    let _ = queue;
    todo!("graphics task drain")
}
