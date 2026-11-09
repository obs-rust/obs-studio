//! Tier 1 mirrors test_video_graphics_tasks.c without FFI or worker threads.
use obs_core::video_graphics_tasks::{Queue, execute};
use std::collections::VecDeque;

#[derive(Default)]
struct Graphics {
    tasks: VecDeque<u32>,
    values: Vec<u32>,
    locked: bool,
    callback_enqueue: bool,
    locks: usize,
}
impl Queue for Graphics {
    type Task = u32;
    fn lock(&mut self) {
        assert!(!self.locked);
        self.locked = true;
        self.locks += 1;
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        self.locked = false;
    }
    fn pending(&mut self) -> bool {
        assert!(self.locked);
        !self.tasks.is_empty()
    }
    fn pop(&mut self) -> u32 {
        assert!(self.locked);
        self.tasks.pop_front().unwrap()
    }
    fn run(&mut self, task: u32) {
        assert!(
            !self.locked,
            "producer must acquire the mutex during a callback"
        );
        self.values.push(task);
        if self.callback_enqueue && task == 1 {
            // Public dispatch from the graphics thread is inline (unchanged C).
            self.values.push(2);
            // A distinct producer appends before the parent callback returns.
            self.tasks.push_back(5);
            self.values.push(3);
        }
    }
}

#[test]
fn graphics_fifo_matches_c() {
    let mut queue = Graphics {
        tasks: (1..=4).collect(),
        ..Graphics::default()
    };
    execute(&mut queue);
    assert_eq!(queue.values.len(), 4);
    assert_eq!(queue.values, [1, 2, 3, 4]);
    assert!(queue.tasks.is_empty());
    assert!(!queue.locked);
}

#[test]
fn graphics_callback_enqueue_matches_c() {
    let mut queue = Graphics {
        tasks: [1, 4].into(),
        callback_enqueue: true,
        ..Graphics::default()
    };
    execute(&mut queue);
    assert_eq!(queue.values.len(), 5);
    assert_eq!(queue.values, [1, 2, 3, 4, 5]);
    assert!(queue.tasks.is_empty());
    assert!(!queue.locked);
    assert_eq!(queue.locks, 4);
}

#[test]
fn graphics_empty_and_second_drain() {
    let mut queue = Graphics::default();
    execute(&mut queue);
    assert_eq!(queue.locks, 1);
    assert!(queue.values.is_empty());
    assert!(!queue.locked);
    queue.tasks.push_back(9);
    execute(&mut queue);
    assert_eq!(queue.values, [9]);
    assert!(!queue.locked);
}
