//! Blend entry-point orchestration. Storage and reset decisions remain in state_stacks.
use crate::state_stacks::BlendState;

/// A validated graphics context's storage and ordered device operations.
pub trait Context {
    fn current(&self) -> BlendState;
    fn push(&mut self, state: BlendState);
    fn pop(&mut self) -> Option<BlendState>;
    fn enable(&mut self, value: bool);
    fn factors(&mut self, state: BlendState);
    fn operation(&mut self, value: u32);
}

pub fn push<C: Context>(_context: Option<&mut C>) { todo!() }
pub fn pop<C: Context>(_context: Option<&mut C>) { todo!() }
pub fn reset<C: Context>(_context: Option<&mut C>) { todo!() }
