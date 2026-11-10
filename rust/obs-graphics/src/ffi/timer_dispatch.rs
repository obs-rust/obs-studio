//! Thin ABI conversion; private graphics layouts stay in C.
use crate::timer_dispatch::{self, Command, Host};
use std::ffi::{c_char, c_void};
type Callback = unsafe extern "C" fn(*mut c_void);
type DataCallback = unsafe extern "C" fn(*mut c_void, *mut u64) -> bool;
unsafe extern "C" {
    fn obs_timer_context() -> *mut c_void;
    fn obs_timer_debug(function: *const c_char, reason: *const c_char);
    fn obs_timer_destroy_field(graphics: *mut c_void) -> Callback;
    fn obs_timer_begin_field(graphics: *mut c_void) -> Callback;
    fn obs_timer_end_field(graphics: *mut c_void) -> Callback;
    fn obs_timer_data_field(graphics: *mut c_void) -> DataCallback;
}
struct Adapter(*mut c_void);
impl Host<*mut c_void> for Adapter {
    type Output = *mut u64;
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, function: &'static str, reason: &'static str) {
        let function = match function {
            "gs_timer_destroy" => c"gs_timer_destroy",
            "gs_timer_begin" => c"gs_timer_begin",
            "gs_timer_end" => c"gs_timer_end",
            "gs_timer_get_data" => c"gs_timer_get_data",
            _ => unreachable!("static timer function"),
        };
        let reason = match reason {
            "called while not in a graphics context" => c"called while not in a graphics context",
            "Null 'timer' parameter" => c"Null 'timer' parameter",
            "Null 'ticks' parameter" => c"Null 'ticks' parameter",
            _ => unreachable!("static timer diagnostic"),
        };
        // SAFETY: both strings remain live throughout synchronous logging.
        unsafe { obs_timer_debug(function.as_ptr(), reason.as_ptr()) };
    }
    fn command(&mut self, command: Command, timer: *mut c_void) {
        // SAFETY: core validated context and handle; backend owns the timer.
        unsafe {
            let callback = match command {
                Command::Destroy => obs_timer_destroy_field(self.0),
                Command::Begin => obs_timer_begin_field(self.0),
                Command::End => obs_timer_end_field(self.0),
            };
            callback(timer);
        }
    }
    fn data(&mut self, timer: *mut c_void, ticks: *mut u64) -> bool {
        // SAFETY: core checked context and both pointers; caller supplies writable output.
        unsafe { obs_timer_data_field(self.0)(timer, ticks) }
    }
}
unsafe fn command(timer: *mut c_void, command: Command) {
    // SAFETY: reads the current thread context without borrowing its storage.
    let mut host = Adapter(unsafe { obs_timer_context() });
    timer_dispatch::command(&mut host, (!timer.is_null()).then_some(timer), command);
}
/// Release a timer through its backend.
/// # Safety
/// A non-null timer belongs to the current graphics context and is not used afterward.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_timer_destroy(timer: *mut c_void) {
    // SAFETY: caller provides the backend ownership contract.
    unsafe { command(timer, Command::Destroy) };
}
/// Begin a backend timer.
/// # Safety
/// A non-null timer belongs to the current graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_timer_begin(timer: *mut c_void) {
    // SAFETY: caller provides a live backend timer.
    unsafe { command(timer, Command::Begin) };
}
/// End a backend timer.
/// # Safety
/// A non-null timer belongs to the current graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_timer_end(timer: *mut c_void) {
    // SAFETY: caller provides a live backend timer.
    unsafe { command(timer, Command::End) };
}
/// Forward a backend read, including any output write on a false result.
/// # Safety
/// Non-null timer and ticks must be a live backend timer and writable u64, respectively.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_timer_get_data(timer: *mut c_void, ticks: *mut u64) -> bool {
    // SAFETY: reads this thread's graphics context without retaining a borrow.
    let mut host = Adapter(unsafe { obs_timer_context() });
    timer_dispatch::get_data(
        &mut host,
        (!timer.is_null()).then_some(timer),
        (!ticks.is_null()).then_some(ticks),
    )
}
