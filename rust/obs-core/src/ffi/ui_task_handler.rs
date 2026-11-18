//! Thin bridge to C-owned core storage and the existing public task queue.
use crate::ui_task_handler::{self, Host};
use core::ffi::{c_int, c_void};

pub type Task = Option<unsafe extern "C" fn(*mut c_void)>;
pub type Handler = Option<unsafe extern "C" fn(Task, *mut c_void, bool)>;

unsafe extern "C" {
    fn obs_core_ui_task_handler_core() -> *mut c_void;
    fn obs_core_ui_task_handler_field(core: *mut c_void) -> *mut Handler;
    fn obs_queue_task(
        kind: c_int,
        task: unsafe extern "C" fn(*mut c_void),
        data: *mut c_void,
        wait: bool,
    );
}

struct Calls {
    core: *mut c_void,
}
impl Host for Calls {
    type Handler = Handler;
    fn store(&mut self, handler: Handler) {
        // SAFETY: access() checked the live core before this field adapter.
        unsafe { obs_core_ui_task_handler_field(self.core).write(handler) };
    }
    fn queue_bootstrap(&mut self) {
        // SAFETY: the callback ignores its NULL parameter, has static lifetime,
        // and the caller keeps OBS initialized throughout dispatch.
        unsafe { obs_queue_task(0, set_ui_thread, core::ptr::null_mut(), false) };
    }
}

unsafe extern "C" fn set_ui_thread(_unused: *mut c_void) {
    ui_task_handler::mark_ui_thread();
}

/// Install or clear the handler, then enqueue the UI-thread bootstrap.
///
/// # Safety
/// OBS must be initialized and remain live; shutdown or other handler writes
/// must not race the call, matching the existing C API contract. A non-NULL
/// handler must accept the forwarded callback/data/wait arguments.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_set_ui_task_handler(handler: Handler) {
    // SAFETY: the global pointer query does not access any core field.
    let core = unsafe { obs_core_ui_task_handler_core() };
    assert!(!core.is_null(), "UI handler requires an initialized core");
    ui_task_handler::install(&mut Calls { core }, handler);
}

/// Private bridge used by C's obs_in_task_thread(OBS_TASK_UI).
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_is_ui_thread() -> bool {
    ui_task_handler::is_ui_thread()
}
