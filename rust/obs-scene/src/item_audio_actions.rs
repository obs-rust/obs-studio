//! Safe core of `libobs/obs-scene-item-audio-actions.c.h`: the scene item's
//! side of the visibility audio ramp.
//!
//! [`crate::audio_actions`] applies an item's queued shows and hides to a
//! tick's gain buffer. This is the item around it, as the C has it:
//! - [`apply_scene_item_audio_actions`] takes the item's `actions_mutex`,
//!   hands the queue and a copy of the item's visibility to the core, writes
//!   the copy back and unlocks. Only then does it drop one active reference
//!   per hide the core returned, and remove the item's source from its
//!   scene's active children when the count reaches 0: the removal takes
//!   other locks, so it never runs under `actions_mutex`;
//! - [`apply_scene_item_volume`] copies the first queued action under the
//!   lock and, after the unlock, applies the tick only when that action is
//!   due ([`crate::audio_actions::action_due`]);
//! - [`process_all_audio_actions`] applies every queued action, whatever
//!   its time, by calling [`apply_scene_item_volume`] with `ts` 0 and no
//!   buffer until it returns false.
//!
//! The item is reached only through [`ItemAudio`]: the C shim implements it
//! over the live field adapter `libobs/obs-scene-item-audio-actions-fields.c.h`,
//! so `struct obs_scene_item` is never mirrored here.
//!
//! Characterized, not endorsed (`test_scene_audio_action_steps`), and kept
//! as the C has them: every hide drops a reference, without bound (more
//! hides than references take the count below 0, and the child is removed
//! only when a drop lands on 0); the window looks at the first action only.
//! The core's own intentional differences (a rate of 0, `ts` 0 with a
//! buffer) are documented in [`crate::audio_actions`].

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::audio_actions::{Gains, ItemAction};

/// The fields and calls of one scene item that the item side uses, each the
/// C's operation on the live item.
pub trait ItemAudio {
    /// `pthread_mutex_lock(&item->actions_mutex)`.
    fn lock(&self);
    /// `pthread_mutex_unlock(&item->actions_mutex)`.
    fn unlock(&self);
    /// `os_atomic_load_bool(&item->visible)`.
    fn visible(&self) -> bool;
    /// `os_atomic_set_bool(&item->visible, visible)`.
    fn set_visible(&self, visible: bool);
    /// The first queued action, `item->audio_actions.array[0]`, or `None`
    /// when the queue is empty. Called with the lock held.
    fn first_action(&self) -> Option<ItemAction>;
    /// `obs_scene_audio_actions_apply(&item->audio_actions, visible, ts,
    /// sample_rate, buf)`: applies the queue to `*visible` and `buf`,
    /// erases the actions it consumed and returns the hides among them.
    /// Called with the lock held.
    fn apply_queue(
        &self,
        visible: &mut bool,
        ts: u64,
        sample_rate: usize,
        buf: Option<&mut Gains>,
    ) -> usize;
    /// `os_atomic_dec_long(&item->active_refs) == 0`: drops one active
    /// reference and says whether that took the count to 0.
    fn release_active_ref(&self) -> bool;
    /// `obs_source_remove_active_child(item->parent->source, item->source)`.
    fn remove_active_child(&self);
}

/// `apply_scene_item_audio_actions()`: applies the item's queued actions
/// that fall in the tick at `ts` (all of them when `ts` is 0) to its
/// visibility and to `buf`, then drops one active reference per hide.
#[expect(clippy::todo, reason = "RED: the port follows")]
pub fn apply_scene_item_audio_actions<I: ItemAudio + ?Sized>(
    item: &I,
    buf: Option<&mut Gains>,
    ts: u64,
    sample_rate: usize,
) {
    let _ = (item, buf, ts, sample_rate);
    todo!()
}

/// `apply_scene_item_volume()`: when the first queued action is due in the
/// tick at `ts`, applies the tick ([`apply_scene_item_audio_actions`]) and
/// returns true; otherwise touches nothing and returns false.
#[expect(clippy::todo, reason = "RED: the port follows")]
pub fn apply_scene_item_volume<I: ItemAudio + ?Sized>(
    item: &I,
    buf: Option<&mut Gains>,
    ts: u64,
    sample_rate: usize,
) -> bool {
    let _ = (item, buf, ts, sample_rate);
    todo!()
}

/// `process_all_audio_actions()`: applies every queued action now.
#[expect(clippy::todo, reason = "RED: the port follows")]
pub fn process_all_audio_actions<I: ItemAudio + ?Sized>(item: &I, sample_rate: usize) {
    let _ = (item, sample_rate);
    todo!()
}
