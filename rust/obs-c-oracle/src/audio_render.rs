//! `oracle_*` build of `libobs/obs-audio-render.c.h`
//! (oracle/audio_render.c). The C enums come back as `int`.

/// `struct obs_audio_monitor_dedup` as the C declares it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MonitorDedup {
    pub set: bool,
    pub active: bool,
    pub muted: bool,
    pub volume: f32,
    pub mixers: u32,
}

/// Index of each entry in `oracle_audio_render_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 18] = [
    "sizeof(enum obs_audio_tree_choice)",
    "OBS_AUDIO_TREE_NONE",
    "OBS_AUDIO_TREE_SKIP",
    "OBS_AUDIO_TREE_ADD",
    "OBS_AUDIO_TREE_DUPLICATE",
    "sizeof(enum obs_audio_backwards_choice)",
    "OBS_AUDIO_BACKWARDS_NONE",
    "OBS_AUDIO_BACKWARDS_PENDING",
    "OBS_AUDIO_BACKWARDS_IGNORE",
    "sizeof(struct obs_audio_monitor_dedup)",
    "offsetof(struct obs_audio_monitor_dedup, set)",
    "offsetof(struct obs_audio_monitor_dedup, active)",
    "offsetof(struct obs_audio_monitor_dedup, muted)",
    "offsetof(struct obs_audio_monitor_dedup, volume)",
    "offsetof(struct obs_audio_monitor_dedup, mixers)",
    "OBS_AUDIO_RENDER_TYPE_INPUT",
    "OBS_AUDIO_RENDER_SOURCE_AUDIO",
    "OBS_AUDIO_RENDER_SOURCE_COMPOSITE",
];

unsafe extern "C" {
    pub fn oracle_obs_audio_render_tree(
        removed: bool,
        present: bool,
        kind: u32,
        output_flags: u32,
        duplicated: bool,
    ) -> i32;
    pub fn oracle_obs_audio_render_silence(
        dedup: *const MonitorDedup,
        monitoring_enabled: bool,
        is_dedup_source: bool,
    ) -> bool;
    pub fn oracle_obs_audio_render_clear_mixes(dedup_mixers: u32, source_mixers: u32) -> u32;
    pub fn oracle_obs_audio_render_backwards(
        maxed: bool,
        audio_ts: u64,
        tick_start: u64,
        audio_render: bool,
    ) -> i32;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_audio_render_layout(which: usize) -> usize;
}
