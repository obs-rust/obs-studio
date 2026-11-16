//! Opaque context and callback boundary; no private C layout is mirrored.
use crate::stencil::{Backend, Command, dispatch};
use std::ffi::{CStr, c_char, c_void};
use std::ptr::NonNull;

unsafe extern "C" {
    fn obs_graphics_stencil_context(function: *const c_char) -> *mut c_void;
    fn obs_graphics_stencil_test(context: *mut c_void, enable: bool);
    fn obs_graphics_stencil_write(context: *mut c_void, enable: bool);
    fn obs_graphics_stencil_function(context: *mut c_void, side: i32, test: i32);
    fn obs_graphics_stencil_op(context: *mut c_void, side: i32, fail: i32, zfail: i32, zpass: i32);
}

struct Context(NonNull<c_void>);
impl Backend for Context {
    fn test(&mut self, enable: bool) {
        // SAFETY: Context is created only from the validated current live context.
        unsafe { obs_graphics_stencil_test(self.0.as_ptr(), enable) }
    }
    fn write(&mut self, enable: bool) {
        // SAFETY: Context is created only from the validated current live context.
        unsafe { obs_graphics_stencil_write(self.0.as_ptr(), enable) }
    }
    fn function(&mut self, side: i32, test: i32) {
        // SAFETY: C retains callback signatures and the validated context layout.
        unsafe { obs_graphics_stencil_function(self.0.as_ptr(), side, test) }
    }
    fn op(&mut self, side: i32, fail: i32, zfail: i32, zpass: i32) {
        // SAFETY: C retains callback signatures and the validated context layout.
        unsafe { obs_graphics_stencil_op(self.0.as_ptr(), side, fail, zfail, zpass) }
    }
}

unsafe fn apply(name: &CStr, command: Command) {
    // SAFETY: caller owns the current graphics context through this synchronous call;
    // C validates it and emits the original diagnostic before any callback access.
    let mut context =
        NonNull::new(unsafe { obs_graphics_stencil_context(name.as_ptr()) }).map(Context);
    dispatch(context.as_mut(), command);
}

/// Enable or disable stencil testing.
/// # Safety
/// The caller must own the current graphics context, if present.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_enable_stencil_test(enable: bool) {
    // SAFETY: forwarded caller contract; no pointer escapes the call.
    unsafe { apply(c"gs_enable_stencil_test", Command::Test(enable)) }
}

/// Enable or disable stencil writes.
/// # Safety
/// The caller must own the current graphics context, if present.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_enable_stencil_write(enable: bool) {
    // SAFETY: forwarded caller contract; no pointer escapes the call.
    unsafe { apply(c"gs_enable_stencil_write", Command::Write(enable)) }
}

/// Select stencil comparison without changing enum values.
/// # Safety
/// The caller must own the current graphics context, if present.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_stencil_function(side: i32, test: i32) {
    // SAFETY: forwarded caller contract; no pointer escapes the call.
    unsafe { apply(c"gs_stencil_function", Command::Function { side, test }) }
}

/// Select stencil operations, preserving fail/zfail/zpass order.
/// # Safety
/// The caller must own the current graphics context, if present.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_stencil_op(side: i32, fail: i32, zfail: i32, zpass: i32) {
    // SAFETY: forwarded caller contract; no pointer escapes the call.
    unsafe {
        apply(
            c"gs_stencil_op",
            Command::Op {
                side,
                fail,
                zfail,
                zpass,
            },
        )
    }
}
