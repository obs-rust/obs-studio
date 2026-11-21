//! Safe core of `libobs/obs-encoder-group-num-started.c.h`:
//! `obs_encoder_group_num_started()`, the number of a group's encoders that
//! have started.
//!
//! The group's mutex is taken once, the count read once while it is held,
//! and the mutex released once; the value read under the lock is returned,
//! whatever the count becomes after the unlock. The full `uint32_t` range
//! passes through unchanged: no other group state clamps or gates it. The
//! group itself, its mutex and its count stay with the caller; the group is
//! never NULL (the C precondition), so there is no NULL case to decide.

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

/// An encoder group as the count query sees it: `group->mutex` and
/// `group->num_encoders_started`.
pub trait StartedCount {
    /// Take the group's mutex.
    fn lock(&mut self);
    /// One read of `num_encoders_started`.
    fn read(&mut self) -> u32;
    /// Release the group's mutex.
    fn unlock(&mut self);
}

/// The group's started-encoder count, read under its mutex.
#[allow(clippy::todo)]
pub fn num_started<G: StartedCount>(_group: &mut G) -> u32 {
    todo!("RED: obs_encoder_group_num_started (#5319)")
}
