//! Blend entry points: the setters from `state-blend-setters.c.h`
//! (`gs_enable_blending`, `gs_blend_function`, `gs_blend_function_separate`
//! and `gs_blend_op`) and the push, pop and reset from
//! `state-blend-entrypoints.c.h`. Storage and reset decisions remain in
//! `state_stacks`.
//!
//! A `None` context is a call made outside a graphics context: the adapter
//! that looked for one has written its debug line, and nothing else happens.
//! With one, each setter stores the new state first and then makes its one
//! device call; `gs_blend_function` stores its pair as both the colour and
//! the alpha factors, and `gs_blend_op` passes the device the operation as
//! stored.
use crate::state_stacks::{self, BlendState, blend};

/// A validated graphics context's blend storage and its device's blend calls.
pub trait Context {
    fn current(&self) -> BlendState;
    /// The context's current blend state, written in place by the setters.
    fn current_mut(&mut self) -> &mut BlendState;
    fn push(&mut self, state: BlendState);
    fn pop(&mut self) -> Option<BlendState>;
    /// The device's `device_enable_blending`.
    fn enable(&mut self, value: bool);
    /// The device's `device_blend_function`.
    fn function(&mut self, src: u32, dest: u32);
    /// The device's `device_blend_function_separate`.
    fn factors(&mut self, src_c: u32, dest_c: u32, src_a: u32, dest_a: u32);
    /// The device's `device_blend_op`.
    fn operation(&mut self, value: u32);
}

/// `gs_enable_blending`.
pub fn enable_blending<C: Context>(context: Option<&mut C>, enable: bool) {
    if let Some(context) = context {
        store_enabled(context, enable);
    }
}

/// `gs_blend_function`: one pair for the colour and the alpha factors.
pub fn blend_function<C: Context>(context: Option<&mut C>, src: u32, dest: u32) {
    let Some(context) = context else { return };
    let state = context.current_mut();
    state.src_c = src;
    state.dest_c = dest;
    state.src_a = src;
    state.dest_a = dest;
    context.function(src, dest);
}

/// `gs_blend_function_separate`.
pub fn blend_function_separate<C: Context>(
    context: Option<&mut C>,
    src_c: u32,
    dest_c: u32,
    src_a: u32,
    dest_a: u32,
) {
    if let Some(context) = context {
        store_factors(context, src_c, dest_c, src_a, dest_a);
    }
}

/// `gs_blend_op`: the device gets the operation as stored.
pub fn blend_op<C: Context>(context: Option<&mut C>, op: u32) {
    if let Some(context) = context {
        store_op(context, op);
    }
}

// The setters' bodies past the context check, which push and reset reach
// with the context they already validated, as the C calls the setters.
fn store_enabled<C: Context>(context: &mut C, enable: bool) {
    context.current_mut().enabled = enable;
    context.enable(enable);
}
fn store_factors<C: Context>(context: &mut C, src_c: u32, dest_c: u32, src_a: u32, dest_a: u32) {
    let state = context.current_mut();
    state.src_c = src_c;
    state.dest_c = dest_c;
    state.src_a = src_a;
    state.dest_a = dest_a;
    context.factors(src_c, dest_c, src_a, dest_a);
}
fn store_op<C: Context>(context: &mut C, op: u32) {
    context.current_mut().op = op;
    let stored = context.current().op;
    context.operation(stored);
}

/// Save the current state without invoking the device.
pub fn push<C: Context>(context: Option<&mut C>) {
    if let Some(context) = context {
        context.push(context.current());
    }
}
/// Restore a saved state through the setters: enable, factors, then operation.
pub fn pop<C: Context>(context: Option<&mut C>) {
    let Some(context) = context else { return };
    let Some(state) = context.pop() else { return };
    store_enabled(context, state.enabled);
    store_factors(
        context,
        state.src_c,
        state.dest_c,
        state.src_a,
        state.dest_a,
    );
    store_op(context, state.op);
}
/// Reset, through the setters, only the components the reset-decision helper asks for.
pub fn reset<C: Context>(context: Option<&mut C>) {
    let Some(context) = context else { return };
    let steps = state_stacks::blend_reset_steps(&context.current());
    if steps.enable {
        store_enabled(context, true);
    }
    if steps.function {
        store_factors(
            context,
            blend::SRCALPHA,
            blend::INVSRCALPHA,
            blend::ONE,
            blend::INVSRCALPHA,
        );
    }
    if steps.op {
        store_op(context, blend::OP_ADD);
    }
}
