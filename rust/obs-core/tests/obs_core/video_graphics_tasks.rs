//! Tier 1: every assertion of test_video_graphics_tasks.c, without FFI or
//! threads, over a model queue that logs each call.
use obs_core::video_graphics_tasks::{Queue, execute};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Lock,
    Unlock,
    Pending,
    Pop,
    Run(u32),
}

/// A queue of task ids. Running task `id` records it, then appends
/// `queued[id]` to the queue the way another producer would (taking the
/// mutex, which the drain must not hold).
#[derive(Default)]
pub struct Model {
    pub tasks: VecDeque<u32>,
    pub queued: Vec<Vec<u32>>,
    pub locked: bool,
    pub values: Vec<u32>,
    pub calls: Vec<Call>,
}
impl Queue for Model {
    type Task = u32;
    fn lock(&mut self) {
        assert!(!self.locked, "task_mutex taken twice");
        self.locked = true;
        self.calls.push(Call::Lock);
    }
    fn unlock(&mut self) {
        assert!(self.locked, "task_mutex released unheld");
        self.locked = false;
        self.calls.push(Call::Unlock);
    }
    fn pending(&mut self) -> bool {
        assert!(self.locked, "tasks.size read unlocked");
        self.calls.push(Call::Pending);
        !self.tasks.is_empty()
    }
    fn pop(&mut self) -> u32 {
        assert!(self.locked, "task popped unlocked");
        self.calls.push(Call::Pop);
        self.tasks.pop_front().expect("pop only after pending")
    }
    fn run(&mut self, task: u32) {
        assert!(!self.locked, "task run with task_mutex held");
        self.calls.push(Call::Run(task));
        self.values.push(task);
        let children = self.queued.get(task as usize).cloned().unwrap_or_default();
        self.tasks.extend(children);
    }
}

/// The C's calls for a drain that runs `ran`, in order: the lock, then per
/// task the size check, pop, unlock, run and lock again, then the last size
/// check and the unlock.
pub fn expected_calls(ran: &[u32]) -> Vec<Call> {
    let mut calls = vec![Call::Lock];
    for &task in ran {
        calls.extend([
            Call::Pending,
            Call::Pop,
            Call::Unlock,
            Call::Run(task),
            Call::Lock,
        ]);
    }
    calls.extend([Call::Pending, Call::Unlock]);
    calls
}

/// graphics_fifo: queued tasks run in the order queued, each once.
#[test]
fn graphics_fifo() {
    let mut queue = Model {
        tasks: (1..=4).collect(),
        ..Model::default()
    };
    execute(&mut queue);
    assert_eq!(queue.values, [1, 2, 3, 4]);
    assert!(queue.tasks.is_empty());
    assert!(!queue.locked);
    assert_eq!(queue.calls, expected_calls(&[1, 2, 3, 4]));
}

/// graphics_callback_enqueue: a task queued while another runs (here task 5,
/// queued by task 1's producer) runs in the same pass, after the tasks
/// queued before it; the mutex is released around every call.
#[test]
fn graphics_callback_enqueue() {
    let mut queue = Model {
        tasks: [1, 4].into(),
        queued: vec![vec![], vec![5]],
        ..Model::default()
    };
    execute(&mut queue);
    assert_eq!(queue.values, [1, 4, 5]);
    assert!(queue.tasks.is_empty());
    assert!(!queue.locked);
    assert_eq!(queue.calls, expected_calls(&[1, 4, 5]));
}

/// An empty queue is checked once under the mutex; a later drain runs what
/// was queued since.
#[test]
fn graphics_empty_then_second_drain() {
    let mut queue = Model::default();
    execute(&mut queue);
    assert!(queue.values.is_empty());
    assert_eq!(queue.calls, expected_calls(&[]));
    queue.calls.clear();
    queue.tasks.push_back(9);
    execute(&mut queue);
    assert_eq!(queue.values, [9]);
    assert!(!queue.locked);
    assert_eq!(queue.calls, expected_calls(&[9]));
}

/// A chain of tasks that each queue the next drains in one pass.
#[test]
fn graphics_chain_drains_in_one_pass() {
    let mut queue = Model {
        tasks: [0].into(),
        queued: (1..=8).map(|next| vec![next]).collect(),
        ..Model::default()
    };
    execute(&mut queue);
    assert_eq!(queue.values, (0..=8).collect::<Vec<_>>());
    assert_eq!(queue.calls, expected_calls(&queue.values.clone()));
}
