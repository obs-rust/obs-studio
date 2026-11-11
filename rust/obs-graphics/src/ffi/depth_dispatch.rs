//! Thin public ABI shims; private graphics fields remain in C.
use crate::depth_dispatch::{self, Command, Host};
use std::ffi::{c_char, c_uint, c_void};
type EnableCallback = unsafe extern "C" fn(*mut c_void, bool);
type FunctionCallback = unsafe extern "C" fn(*mut c_void, c_uint);
unsafe extern "C" {
    fn obs_depth_context() -> *mut c_void;
    fn obs_depth_device(graphics: *mut c_void) -> *mut c_void;
    fn obs_depth_enable_field(graphics: *mut c_void) -> EnableCallback;
    fn obs_depth_function_field(graphics: *mut c_void) -> FunctionCallback;
    fn obs_depth_debug(function: *const c_char);
}
struct Adapter(*mut c_void);
impl Host for Adapter {
    fn has_context(&self) -> bool {
        !self.0.is_null()
    }
    fn debug(&mut self, function: &'static str, _reason: &'static str) {
        let function = match function {
            "gs_enable_depth_test" => c"gs_enable_depth_test",
            "gs_depth_function" => c"gs_depth_function",
            _ => unreachable!("static depth-state function"),
        };
        // SAFETY: function is a static terminated string, consumed synchronously.
        unsafe { obs_depth_debug(function.as_ptr()) };
    }
    fn dispatch(&mut self, command: Command) {
        // SAFETY: the core checked context before any field access; backend owns the device.
        unsafe {
            let device = obs_depth_device(self.0);
            match command {
                Command::Enable(enable) => obs_depth_enable_field(self.0)(device, enable),
                Command::Function(test) => obs_depth_function_field(self.0)(device, test),
            }
        }
    }
}
fn dispatch(command: Command) {
    // SAFETY: reads this thread's current context without retaining a borrow.
    let mut host = Adapter(unsafe { obs_depth_context() });
    depth_dispatch::command(&mut host, command);
}
/// Forward a depth-test enable state to the current backend.
/// # Safety
/// An active context must contain a live device and its matching callback table.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_enable_depth_test(enable: bool) {
    dispatch(Command::Enable(enable));
}
/// Forward a depth comparison value unchanged to the current backend.
/// # Safety
/// An active context must contain a live device and its matching callback table.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_depth_function(test: c_uint) {
    dispatch(Command::Function(test));
}
