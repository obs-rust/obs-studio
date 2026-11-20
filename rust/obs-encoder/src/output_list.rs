//! Safe core of `libobs/obs-encoder-outputs.c.h`: `obs_encoder_add_output()`
//! and `obs_encoder_remove_output()`, which keep an encoder's list of the
//! outputs that use it.
//!
//! A missing encoder or output returns at once, before any lock or read.
//! Otherwise the list's mutex is taken, the list changed and the mutex
//! released, with every read and change between the two. Adding appends at
//! the end and accepts an output already listed; removing erases only the
//! first entry equal to the output, keeping the order of the rest, and an
//! output not listed leaves the list as it was. Duplicates and first-match
//! removal are characterized, not endorsed. The list, its storage and its
//! mutex stay with the caller.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// An encoder's output list as add and remove see it:
/// `encoder->outputs_mutex`, `outputs.num`, `outputs.array[i]`,
/// `da_push_back()` and `da_erase()`.
pub trait Outputs {
    /// A `struct obs_output *`, compared by identity only.
    type Output: Copy + PartialEq;
    /// Take the list's mutex.
    fn lock(&mut self);
    /// Release the list's mutex.
    fn unlock(&mut self);
    /// Append an entry at the end.
    fn push(&mut self, output: Self::Output);
    /// One read of `outputs.num`.
    fn count(&mut self) -> usize;
    /// One read of `outputs.array[index]`, for `index < count()`.
    fn at(&mut self, index: usize) -> Option<Self::Output>;
    /// Erase entry `index`, keeping the order of the rest.
    fn erase(&mut self, index: usize);
}

/// Append `output` to the encoder's list; nothing without both.
#[allow(clippy::todo)]
pub fn add_output<L: Outputs>(_list: Option<&mut L>, _output: Option<L::Output>) {
    todo!("RED: obs_encoder_add_output")
}

/// Erase the first entry equal to `output`; nothing without both.
#[allow(clippy::todo)]
pub fn remove_output<L: Outputs>(_list: Option<&mut L>, _output: Option<L::Output>) {
    todo!("RED: obs_encoder_remove_output")
}
