//! The C ABI of the blend entry points: the four setters, push, pop and
//! reset, over a validated thread-local context.
//!
//! The context read with its debug line, the blend state and stack, and the
//! four device calls are C adapters (`libobs/graphics/blend-entrypoints-fields.c.h`)
//! that keep the private graphics layout and the device ABI in C: one
//! context read per call and one C call per device call, nothing cached or
//! allocated.
use super::state_stacks::DarrayStack;
use crate::blend_entrypoints::{self, Context};
use crate::state_stacks::{self, BlendState};
use core::ffi::{c_char, c_void};
use core::ptr::NonNull;
use obs_util::ffi::darray::darray;

unsafe extern "C" {
    fn obs_graphics_blend_context(function: *const c_char) -> *mut c_void;
    fn obs_graphics_blend_current(context: *mut c_void) -> *mut BlendState;
    fn obs_graphics_blend_stack(context: *mut c_void) -> *mut darray;
    fn obs_graphics_blend_enable_blending(context: *mut c_void, value: bool);
    fn obs_graphics_blend_function(context: *mut c_void, src: u32, dest: u32);
    fn obs_graphics_blend_function_separate(
        context: *mut c_void,
        sc: u32,
        dc: u32,
        sa: u32,
        da: u32,
    );
    fn obs_graphics_blend_op(context: *mut c_void, value: u32);
}
struct Native(NonNull<c_void>);
impl Native {
    fn context(function: &core::ffi::CStr) -> Option<Self> {
        // SAFETY: static function name; the adapter validates the current context.
        NonNull::new(unsafe { obs_graphics_blend_context(function.as_ptr()) }).map(Self)
    }
    fn stack(&mut self) -> DarrayStack<'_, BlendState> {
        // SAFETY: Native is constructed only for a live, nonnull current context.
        // The private adapter returns its exclusively accessed blend-state darray.
        unsafe { DarrayStack::new(&mut *obs_graphics_blend_stack(self.0.as_ptr())) }
    }
}
impl Context for Native {
    fn current(&self) -> BlendState {
        // SAFETY: the adapter returns the current context's valid blend state.
        unsafe { *obs_graphics_blend_current(self.0.as_ptr()) }
    }
    fn current_mut(&mut self) -> &mut BlendState {
        // SAFETY: the adapter returns the current context's valid blend state,
        // which only this thread's graphics calls touch while it is current.
        unsafe { &mut *obs_graphics_blend_current(self.0.as_ptr()) }
    }
    fn push(&mut self, state: BlendState) {
        state_stacks::state_push(&mut self.stack(), state);
    }
    fn pop(&mut self) -> Option<BlendState> {
        state_stacks::state_pop(&mut self.stack())
    }
    fn enable(&mut self, value: bool) {
        // SAFETY: the validated context is current; the device is the backend's.
        unsafe { obs_graphics_blend_enable_blending(self.0.as_ptr(), value) };
    }
    fn function(&mut self, src: u32, dest: u32) {
        // SAFETY: the validated context is current; values retain the C enums.
        unsafe { obs_graphics_blend_function(self.0.as_ptr(), src, dest) };
    }
    fn factors(&mut self, src_c: u32, dest_c: u32, src_a: u32, dest_a: u32) {
        // SAFETY: the validated context is current; values retain the C enums.
        unsafe {
            obs_graphics_blend_function_separate(self.0.as_ptr(), src_c, dest_c, src_a, dest_a)
        };
    }
    fn operation(&mut self, value: u32) {
        // SAFETY: the validated context is current; value retains the C enum.
        unsafe { obs_graphics_blend_op(self.0.as_ptr(), value) };
    }
}

/// Turn blending on or off, storing it first.
#[unsafe(no_mangle)]
pub extern "C" fn gs_enable_blending(enable: bool) {
    blend_entrypoints::enable_blending(Native::context(c"gs_enable_blending").as_mut(), enable);
}
/// Set one blend factor pair for colour and alpha alike, storing it first.
#[unsafe(no_mangle)]
pub extern "C" fn gs_blend_function(src: u32, dest: u32) {
    blend_entrypoints::blend_function(Native::context(c"gs_blend_function").as_mut(), src, dest);
}
/// Set the colour and alpha blend factors, storing them first.
#[unsafe(no_mangle)]
pub extern "C" fn gs_blend_function_separate(src_c: u32, dest_c: u32, src_a: u32, dest_a: u32) {
    blend_entrypoints::blend_function_separate(
        Native::context(c"gs_blend_function_separate").as_mut(),
        src_c,
        dest_c,
        src_a,
        dest_a,
    );
}
/// Set the blend operation; the device gets it as stored.
#[unsafe(no_mangle)]
pub extern "C" fn gs_blend_op(op: u32) {
    blend_entrypoints::blend_op(Native::context(c"gs_blend_op").as_mut(), op);
}

/// Save the current context's blend state.
#[unsafe(no_mangle)]
pub extern "C" fn gs_blend_state_push() {
    blend_entrypoints::push(Native::context(c"gs_blend_state_push").as_mut());
}
/// Restore the most recently saved blend state.
#[unsafe(no_mangle)]
pub extern "C" fn gs_blend_state_pop() {
    blend_entrypoints::pop(Native::context(c"gs_blend_state_pop").as_mut());
}
/// Restore default blending with the original conditional callback sequence.
#[unsafe(no_mangle)]
pub extern "C" fn gs_reset_blend_state() {
    blend_entrypoints::reset(Native::context(c"gs_reset_blend_state").as_mut());
}
