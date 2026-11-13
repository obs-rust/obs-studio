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
    let _ = (context, enable);
    todo!()
}

/// `gs_blend_function`: one pair for the colour and the alpha factors.
pub fn blend_function<C: Context>(context: Option<&mut C>, src: u32, dest: u32) {
    let _ = (context, src, dest);
    todo!()
}

/// `gs_blend_function_separate`.
pub fn blend_function_separate<C: Context>(
    context: Option<&mut C>,
    src_c: u32,
    dest_c: u32,
    src_a: u32,
    dest_a: u32,
) {
    let _ = (context, src_c, dest_c, src_a, dest_a);
    todo!()
}

/// `gs_blend_op`: the device gets the operation as stored.
pub fn blend_op<C: Context>(context: Option<&mut C>, op: u32) {
    let _ = (context, op);
    todo!()
}

/// Save the current state without invoking the device.
pub fn push<C: Context>(context: Option<&mut C>) {
    if let Some(context) = context {
        context.push(context.current());
    }
}
/// Restore a saved state, always invoking enable, factors, then operation.
pub fn pop<C: Context>(context: Option<&mut C>) {
    let Some(context) = context else { return };
    let Some(state) = context.pop() else { return };
    context.enable(state.enabled);
    context.factors(state.src_c, state.dest_c, state.src_a, state.dest_a);
    context.operation(state.op);
}
/// Reset only the components requested by the existing reset-decision helper.
pub fn reset<C: Context>(context: Option<&mut C>) {
    let Some(context) = context else { return };
    let steps = state_stacks::blend_reset_steps(&context.current());
    if steps.enable {
        context.enable(true);
    }
    if steps.function {
        context.factors(
            blend::SRCALPHA,
            blend::INVSRCALPHA,
            blend::ONE,
            blend::INVSRCALPHA,
        );
    }
    if steps.op {
        context.operation(blend::OP_ADD);
    }
}
