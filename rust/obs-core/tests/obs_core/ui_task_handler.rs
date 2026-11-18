//! Tier 1 mirrors test_ui_task_handler.c through safe host operations.
use obs_core::ui_task_handler::{self, Host};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Task {
    Bootstrap,
    Count,
}
#[derive(Debug, PartialEq, Eq)]
struct Dispatch {
    handler: u8,
    task: Task,
    data: Option<u8>,
    wait: bool,
}
#[derive(Default)]
struct Queue {
    handler: Option<u8>,
    calls: Vec<Dispatch>,
    tasks: usize,
    logs: Vec<&'static str>,
    reenter: bool,
    order: Vec<&'static str>,
}
impl Queue {
    fn queue(&mut self, task: Task, data: Option<u8>, wait: bool) {
        if let Some(handler) = self.handler {
            self.calls.push(Dispatch {
                handler,
                task,
                data,
                wait,
            });
            if std::mem::take(&mut self.reenter) {
                self.queue(Task::Count, Some(7), true);
            }
        } else {
            self.logs
                .push("UI task could not be queued, there's no UI task handler!");
        }
    }
    fn run(&mut self, index: usize) {
        match self.calls[index].task {
            Task::Bootstrap => ui_task_handler::mark_ui_thread(),
            Task::Count => self.tasks += 1,
        }
    }
    fn bootstrap(&self, index: usize, handler: u8) {
        assert_eq!(
            self.calls[index],
            Dispatch {
                handler,
                task: Task::Bootstrap,
                data: None,
                wait: false
            }
        );
    }
}
impl Host for Queue {
    type Handler = Option<u8>;
    fn store(&mut self, handler: Self::Handler) {
        self.order.push("store");
        self.handler = handler;
    }
    fn queue_bootstrap(&mut self) {
        self.order.push("queue");
        self.queue(Task::Bootstrap, None, false);
    }
}

#[test]
fn test_ui_task_handler_install_replace_clear_and_dispatch() {
    // A fresh thread gives the same initially unmarked state as C's fixture.
    std::thread::spawn(|| {
        let mut q = Queue {
            reenter: true,
            ..Queue::default()
        };
        assert!(!ui_task_handler::is_ui_thread());
        ui_task_handler::install(&mut q, Some(1));
        assert_eq!(q.order, ["store", "queue"]);
        assert_eq!(q.calls.len(), 2);
        q.bootstrap(0, 1);
        assert_eq!(
            q.calls[1],
            Dispatch {
                handler: 1,
                task: Task::Count,
                data: Some(7),
                wait: true
            }
        );
        assert_eq!(q.tasks, 0);
        assert!(!ui_task_handler::is_ui_thread());
        q.run(0);
        assert!(ui_task_handler::is_ui_thread());
        q.run(1);
        assert_eq!(q.tasks, 1);
        ui_task_handler::install(&mut q, Some(2));
        assert_eq!(q.calls.len(), 3);
        q.bootstrap(2, 2);
        q.queue(Task::Count, Some(7), false);
        assert_eq!(q.calls.len(), 4);
        assert_eq!(
            q.calls[3],
            Dispatch {
                handler: 2,
                task: Task::Count,
                data: Some(7),
                wait: false
            }
        );
        assert_eq!(q.tasks, 1);
        q.run(2);
        q.run(3);
        assert_eq!(q.tasks, 2);
        ui_task_handler::install(&mut q, None);
        assert_eq!(
            q.logs,
            ["UI task could not be queued, there's no UI task handler!"]
        );
        assert!(ui_task_handler::is_ui_thread());
        q.queue(Task::Count, Some(7), false);
        q.queue(Task::Count, Some(7), true);
        assert_eq!(q.logs.len(), 3);
        assert_eq!(q.calls.len(), 4);
        assert_eq!(q.tasks, 2);
        ui_task_handler::install(&mut q, Some(1));
        assert_eq!(q.calls.len(), 5);
        q.bootstrap(4, 1);
        q.run(4);
        assert!(ui_task_handler::is_ui_thread());
        assert_eq!(q.logs.len(), 3);
        // Thread-local marking never marks the parent thread.
        std::thread::spawn(|| assert!(!ui_task_handler::is_ui_thread()))
            .join()
            .unwrap();
    })
    .join()
    .unwrap();
}
