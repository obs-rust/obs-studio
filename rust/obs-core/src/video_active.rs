//! Safe core of `libobs/obs-video-active.c.h`: `obs_video_active()`, which
//! answers whether any video mix has a raw consumer or a GPU encoder.
//!
//! Under the core's `mixes_mutex`, the mix count is read once and the mixes
//! are walked in order; the answer is true at the first mix whose
//! `raw_active` count, or else whose `gpu_encoder_active` count, is above
//! zero (a negative count is inactive: characterized, not endorsed), and no
//! later count is read. The mutex is released on every path. Nothing is
//! written and no reference is taken. The list, its mutex and the counters
//! stay with the caller.

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

/// The core's mix list as the query sees it: `obs->video.mixes_mutex`,
/// `mixes.num`, `mixes.array[i]` and each mix's two counters.
pub trait Mixes {
    /// A `struct obs_core_video_mix *`, borrowed for the counter reads.
    type Mix;
    fn lock(&mut self);
    fn unlock(&mut self);
    /// One read of `mixes.num`.
    fn count(&mut self) -> usize;
    /// One read of `mixes.array[index]`, for `index < count()`.
    fn at(&mut self, index: usize) -> Self::Mix;
    /// One atomic load of the mix's `raw_active`.
    fn raw_active(&mut self, mix: &Self::Mix) -> i64;
    /// One atomic load of the mix's `gpu_encoder_active`.
    fn gpu_encoder_active(&mut self, mix: &Self::Mix) -> i64;
}

/// Whether any mix has a raw consumer or a GPU encoder counted above zero.
#[allow(clippy::todo)]
pub fn video_active<M: Mixes>(mixes: &mut M) -> bool {
    let _ = mixes;
    todo!("obs_video_active safe core (#5436)")
}
