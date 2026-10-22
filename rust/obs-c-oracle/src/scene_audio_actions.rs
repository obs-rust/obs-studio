//! `oracle_*` build of `libobs/obs-scene-audio-actions.c.h`
//! (oracle/scene_audio_actions.c).
//!
//! The queue is a `DARRAY(struct item_action)`, here an [`OracleDarray`]
//! whose items are `struct item_action`. The oracle erases from it with the
//! C's `da_erase` (the `darray.h` inline) and never allocates.

use super::darray::OracleDarray;

/// Index of each entry in `oracle_scene_audio_actions_layout`, in the C's
/// order.
pub const LAYOUT_NAMES: [&str; 9] = [
    "sizeof(struct item_action)",
    "alignof(struct item_action)",
    "offsetof(struct item_action, visible)",
    "offsetof(struct item_action, timestamp)",
    "sizeof(obs_scene_item_action_array_t)",
    "offsetof(obs_scene_item_action_array_t, array)",
    "offsetof(obs_scene_item_action_array_t, num)",
    "offsetof(obs_scene_item_action_array_t, capacity)",
    "AUDIO_OUTPUT_FRAMES",
];

unsafe extern "C" {
    pub fn oracle_obs_scene_audio_actions_apply(
        actions: *mut OracleDarray,
        visible: *mut bool,
        ts: u64,
        sample_rate: usize,
        buf: *mut f32,
    ) -> usize;
    pub fn oracle_obs_scene_audio_action_due(timestamp: u64, ts: u64, sample_rate: usize) -> bool;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_scene_audio_actions_layout(which: usize) -> usize;
}
