//! Safe core of `libobs/obs-canvas-clear-mix.c.h`: `obs_canvas_clear_mix()`,
//! which detaches a canvas's video mix and frees it if the core lists it.
//!
//! A canvas with no mix returns at once, reading nothing else. Otherwise the
//! mix list's mutex is taken; the list is walked in order, its length read
//! again before each step and the canvas's mix read again for each
//! comparison, as the C loop does; the first entry equal to it is erased
//! from the list and then freed, still under the lock, and the walk stops.
//! After the unlock the canvas's mix is cleared, whether or not the list held
//! it (an unregistered mix is detached, not freed: characterized, not
//! endorsed). The list, its mutex and the mix teardown stay with the caller.

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

/// The canvas's mix slot and the core's mix list as the clear sees them:
/// `canvas->mix`, `obs->video.mixes_mutex`, `mixes.num`, `mixes.array[i]`,
/// `da_erase()` and `obs_free_video_mix()`.
pub trait Mixes {
    /// A `struct obs_core_video_mix *`, compared by identity only.
    type Mix: Copy + PartialEq;
    /// One read of `canvas->mix`.
    fn canvas_mix(&mut self) -> Option<Self::Mix>;
    /// Clear `canvas->mix`.
    fn detach(&mut self);
    fn lock(&mut self);
    fn unlock(&mut self);
    /// One read of `mixes.num`.
    fn count(&mut self) -> usize;
    /// One read of `mixes.array[index]`, for `index < count()`.
    fn at(&mut self, index: usize) -> Option<Self::Mix>;
    /// Erase entry `index`, keeping the order of the rest.
    fn erase(&mut self, index: usize);
    /// Free a mix the list held.
    fn free(&mut self, mix: Option<Self::Mix>);
}

/// Detach the canvas's mix, freeing it when the list held it.
#[expect(clippy::todo, reason = "RED: the port lands in the next commit")]
pub fn clear_mix<M: Mixes>(mixes: &mut M) {
    let _ = mixes;
    todo!()
}
