//! Private graphics queue bridge. C owns the mutex and deque layouts.
use super::audio_callback::obs_task_info;
use crate::video_graphics_tasks::{self, Queue};
use std::ffi::c_void;

unsafe extern "C" {
    fn obs_graphics_tasks_lock(video: *mut c_void);
    fn obs_graphics_tasks_unlock(video: *mut c_void);
    fn obs_graphics_tasks_pending(video: *mut c_void) -> bool;
    fn obs_graphics_tasks_pop(video: *mut c_void, info: *mut obs_task_info);
}

struct Graphics(*mut c_void);
impl Queue for Graphics {
    type Task = obs_task_info;
    fn lock(&mut self) {
        // SAFETY: the entry point requires a live initialized video queue.
        unsafe { obs_graphics_tasks_lock(self.0) }
    }
    fn unlock(&mut self) {
        // SAFETY: the safe drain holds the mutex here.
        unsafe { obs_graphics_tasks_unlock(self.0) }
    }
    fn pending(&mut self) -> bool {
        // SAFETY: the safe drain holds the mutex during queue reads.
        unsafe { obs_graphics_tasks_pending(self.0) }
    }
    fn pop(&mut self) -> obs_task_info {
        let mut info = obs_task_info {
            task: None,
            param: std::ptr::null_mut(),
        };
        // SAFETY: the drain checked nonempty under the mutex; info is writable.
        unsafe { obs_graphics_tasks_pop(self.0, &mut info) };
        info
    }
    fn run(&mut self, info: obs_task_info) {
        // SAFETY: obs_queue_task stores a callable task and its live parameter;
        // the safe drain has released the mutex before invoking it.
        unsafe { info.task.expect("queued graphics callback")(info.param) }
    }
}

/// Drain graphics work on the graphics thread.
///
/// # Safety
/// `video` must be the live `obs_core_video` with an initialized queue mutex.
/// Every queued callback and argument must remain valid until invoked.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_graphics_tasks_execute(video: *mut c_void) {
    video_graphics_tasks::execute(&mut Graphics(video));
}
