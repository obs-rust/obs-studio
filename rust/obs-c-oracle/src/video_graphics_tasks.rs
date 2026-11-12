//! The original C graphics task drain
//! (`libobs/obs-video-graphics-tasks-reference.c.h`) and the production
//! bridge's queue accessors, over a real private core.
use core::ffi::c_void;

/// `obs_task_t`; the pop writes a `struct obs_task_info` through a `void *`.
pub type Task = unsafe extern "C" fn(*mut c_void);

unsafe extern "C" {
    /// A core with an empty graphics task queue; NULL when out of memory.
    pub fn oracle_graphics_tasks_new() -> *mut c_void;
    pub fn oracle_graphics_tasks_free(core: *mut c_void);
    /// Append a task the way another producer does (take task_mutex, push,
    /// release); false, with nothing pushed, while task_mutex is held.
    pub fn oracle_graphics_tasks_push(core: *mut c_void, task: Task, param: *mut c_void) -> bool;
    /// The number of queued tasks.
    pub fn oracle_graphics_tasks_size(core: *mut c_void) -> usize;
    /// Whether the core's task_mutex is held.
    pub fn oracle_graphics_tasks_locked(core: *mut c_void) -> bool;
    /// The original drain over `core`.
    pub fn oracle_graphics_tasks_original(core: *mut c_void);
    pub fn oracle_graphics_tasks_lock(core: *mut c_void);
    pub fn oracle_graphics_tasks_unlock(core: *mut c_void);
    pub fn oracle_graphics_tasks_pending(core: *mut c_void) -> bool;
    pub fn oracle_graphics_tasks_pop(core: *mut c_void, info: *mut c_void);
}
