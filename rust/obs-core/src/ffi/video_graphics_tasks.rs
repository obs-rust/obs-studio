//! C ABI for `libobs/obs-video-graphics-tasks.c.h`, the graphics thread's
//! task drain, declared in `libobs/obs-video-graphics-tasks.h` without
//! `EXPORT` (hidden, `rust-hidden.d`).
//!
//! The C bridge passes the core and its accessors for `task_mutex`,
//! `tasks.size` and a pop of one `struct obs_task_info`: the deque and its
//! mutex stay C-owned, and nothing here reads `struct obs_core`. The popped
//! task is called through its own function pointer, with no lock held and no
//! Rust borrow of the queue across the call.
use super::audio_callback::obs_task_info;
use crate::video_graphics_tasks::{self, Queue};
use core::ffi::c_void;

/// `struct obs_core *`, opaque here.
pub type Core = c_void;

pub type Lock = unsafe extern "C" fn(*mut Core);
pub type Pending = unsafe extern "C" fn(*mut Core) -> bool;
pub type Pop = unsafe extern "C" fn(*mut Core, *mut obs_task_info);

struct Bridge {
    core: *mut Core,
    lock: Lock,
    unlock: Lock,
    pending: Pending,
    pop: Pop,
}

impl Queue for Bridge {
    type Task = obs_task_info;
    fn lock(&mut self) {
        // SAFETY: the bridge's accessor locks this live core's task_mutex.
        unsafe { (self.lock)(self.core) }
    }
    fn unlock(&mut self) {
        // SAFETY: the drain calls this only after its own lock().
        unsafe { (self.unlock)(self.core) }
    }
    fn pending(&mut self) -> bool {
        // SAFETY: read under task_mutex, which lock() holds.
        unsafe { (self.pending)(self.core) }
    }
    fn pop(&mut self) -> obs_task_info {
        let mut info = obs_task_info {
            task: None,
            param: core::ptr::null_mut(),
        };
        // SAFETY: under task_mutex, right after pending() saw a task, so the
        // deque holds at least one whole obs_task_info; info is writable.
        unsafe { (self.pop)(self.core, &mut info) };
        info
    }
    fn run(&mut self, info: obs_task_info) {
        // The C calls info.task unchecked and crashes on a NULL one, which
        // obs_queue_task() does not refuse; like the audio task drain
        // (audio_callback), a NULL task is skipped.
        if let Some(task) = info.task {
            // SAFETY: a task obs_queue_task queued with its own parameter; the
            // drain released task_mutex before this call.
            unsafe { task(info.param) }
        }
    }
}

/// Run every queued graphics task, oldest first, including tasks queued
/// meanwhile.
///
/// # Safety
/// `core` is a live core and the accessors are the bridge's (or behave the
/// same) for it: `pending` and `pop` are called only under `lock`, and `pop`
/// only after `pending` was true. Every queued task and its parameter are
/// valid to call. None of them may unwind.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_video_graphics_tasks_execute(
    core: *mut Core,
    lock: Lock,
    unlock: Lock,
    pending: Pending,
    pop: Pop,
) {
    video_graphics_tasks::execute(&mut Bridge {
        core,
        lock,
        unlock,
        pending,
        pop,
    });
}
