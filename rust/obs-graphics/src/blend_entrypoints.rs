//! Blend entry-point orchestration. Storage and reset decisions remain in state_stacks.
use crate::state_stacks::{self, BlendState, blend};

/// A validated graphics context's storage and ordered device operations.
pub trait Context {
    fn current(&self) -> BlendState;
    fn push(&mut self, state: BlendState);
    fn pop(&mut self) -> Option<BlendState>;
    fn enable(&mut self, value: bool);
    fn factors(&mut self, state: BlendState);
    fn operation(&mut self, value: u32);
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
    context.factors(state);
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
        context.factors(BlendState {
            enabled: true,
            src_c: blend::SRCALPHA,
            dest_c: blend::INVSRCALPHA,
            src_a: blend::ONE,
            dest_a: blend::INVSRCALPHA,
            op: blend::OP_ADD,
        });
    }
    if steps.op {
        context.operation(blend::OP_ADD);
    }
}
