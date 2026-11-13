//! Original libobs entry points over a validated thread-local context.
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
    fn gs_enable_blending(value: bool);
    fn gs_blend_function(src: u32, dest: u32);
    fn gs_blend_function_separate(sc: u32, dc: u32, sa: u32, da: u32);
    fn gs_blend_op(value: u32);
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
        // SAFETY: the thread's context is valid, as required by the C entry point.
        unsafe { gs_enable_blending(value) };
    }
    fn function(&mut self, src: u32, dest: u32) {
        // SAFETY: the validated context is current; values retain the C enums.
        unsafe { gs_blend_function(src, dest) };
    }
    fn factors(&mut self, src_c: u32, dest_c: u32, src_a: u32, dest_a: u32) {
        // SAFETY: the validated context is current; values retain the C enums.
        unsafe { gs_blend_function_separate(src_c, dest_c, src_a, dest_a) };
    }
    fn operation(&mut self, value: u32) {
        // SAFETY: the validated context is current; value retains the C enum.
        unsafe { gs_blend_op(value) };
    }
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
