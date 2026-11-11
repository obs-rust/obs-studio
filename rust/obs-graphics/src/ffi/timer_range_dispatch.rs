//! Thin ABI conversion; private graphics layouts and ownership stay in C/backend.
use crate::timer_range_dispatch::{self, Command, Host};
use std::ffi::{c_char, c_void};
type DestroyCallback = unsafe extern "C" fn(*mut c_void);
type CommandCallback = unsafe extern "C" fn(*mut c_void) -> bool;
type DataCallback = unsafe extern "C" fn(*mut c_void, *mut bool, *mut u64) -> bool;
unsafe extern "C" {
    fn obs_timer_range_context() -> *mut c_void;
    fn obs_timer_range_debug(function: *const c_char, reason: *const c_char);
    fn obs_timer_range_destroy_field(graphics: *mut c_void) -> DestroyCallback;
    fn obs_timer_range_begin_field(graphics: *mut c_void) -> CommandCallback;
    fn obs_timer_range_end_field(graphics: *mut c_void) -> CommandCallback;
    fn obs_timer_range_data_field(graphics: *mut c_void) -> DataCallback;
}
struct Adapter(*mut c_void);
impl Host<*mut c_void> for Adapter {
    type Disjoint = *mut bool;
    type Frequency = *mut u64;
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, function: &'static str, reason: &'static str) {
        let function = match function {
            "gs_timer_range_destroy" => c"gs_timer_range_destroy",
            "gs_timer_range_begin" => c"gs_timer_range_begin",
            "gs_timer_range_end" => c"gs_timer_range_end",
            "gs_timer_range_get_data" => c"gs_timer_range_get_data",
            _ => unreachable!("static timer-range function"),
        };
        let reason = match reason {
            "called while not in a graphics context" => c"called while not in a graphics context",
            "Null 'disjoint' parameter" => c"Null 'disjoint' parameter",
            "Null 'frequency' parameter" => c"Null 'frequency' parameter",
            _ => unreachable!("static timer-range diagnostic"),
        };
        // SAFETY: both strings remain live throughout synchronous logging.
        unsafe { obs_timer_range_debug(function.as_ptr(), reason.as_ptr()) };
    }
    fn command(&mut self, command: Command, range: *mut c_void) {
        // SAFETY: core validated context and handle; backend owns the range.
        unsafe {
            match command {
                Command::Destroy => obs_timer_range_destroy_field(self.0)(range),
                Command::Begin => {
                    obs_timer_range_begin_field(self.0)(range);
                }
                Command::End => {
                    obs_timer_range_end_field(self.0)(range);
                }
            }
        }
    }
    fn data(&mut self, range: *mut c_void, disjoint: *mut bool, frequency: *mut u64) -> bool {
        // SAFETY: core checked context and outputs. Backend accepts range, including NULL.
        unsafe { obs_timer_range_data_field(self.0)(range, disjoint, frequency) }
    }
}
unsafe fn command(range: *mut c_void, command: Command) {
    // SAFETY: reads this thread's context without borrowing its storage.
    let mut host = Adapter(unsafe { obs_timer_range_context() });
    timer_range_dispatch::command(&mut host, (!range.is_null()).then_some(range), command);
}
/// Release a range through its backend.
/// # Safety
/// A non-null range belongs to the current context and is not used afterward.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_timer_range_destroy(range: *mut c_void) {
    // SAFETY: caller provides the backend ownership contract.
    unsafe { command(range, Command::Destroy) };
}
/// Begin a backend range, discarding its boolean result.
/// # Safety
/// A non-null range belongs to the current graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_timer_range_begin(range: *mut c_void) {
    // SAFETY: caller supplies a live backend range.
    unsafe { command(range, Command::Begin) };
}
/// End a backend range, discarding its boolean result.
/// # Safety
/// A non-null range belongs to the current graphics context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_timer_range_end(range: *mut c_void) {
    // SAFETY: caller supplies a live backend range.
    unsafe { command(range, Command::End) };
}
/// Forward a read, including NULL range and any writes on false.
/// # Safety
/// Range must satisfy the active backend's contract; non-null outputs are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_timer_range_get_data(
    range: *mut c_void,
    disjoint: *mut bool,
    frequency: *mut u64,
) -> bool {
    // SAFETY: reads the current thread context without retaining a borrow.
    let mut host = Adapter(unsafe { obs_timer_range_context() });
    timer_range_dispatch::get_data(
        &mut host,
        range,
        (!disjoint.is_null()).then_some(disjoint),
        (!frequency.is_null()).then_some(frequency),
    )
}
