//! `oracle_*` build of `libobs/obs-source-volume.c.h`
//! (oracle/source_volume.c): the source volume snap and apply decisions,
//! renamed. The C's `bool` results are declared as `u8`, the byte the C
//! returns, so a parity test compares them without assuming they are 0 or
//! 1; the enum is declared as `u32`, the width the C returns it in.

unsafe extern "C" {
    /// `float obs_source_volume_snap(bool muted, float volume)`.
    pub fn oracle_obs_source_volume_snap(muted: bool, volume: f32) -> f32;
    /// `bool obs_source_volume_action_due(uint64_t action_ts, uint64_t
    /// audio_ts, uint64_t duration)`.
    pub fn oracle_obs_source_volume_action_due(action_ts: u64, audio_ts: u64, duration: u64) -> u8;
    /// `enum obs_source_volume_op obs_source_volume_step(float vol,
    /// uint32_t mixers)`.
    pub fn oracle_obs_source_volume_step(vol: f32, mixers: u32) -> u32;
    /// `bool obs_source_volume_mixes(uint32_t audio_mixers, uint32_t mixers,
    /// size_t mix)`.
    pub fn oracle_obs_source_volume_mixes(audio_mixers: u32, mixers: u32, mix: usize) -> u8;
    /// `OBS_SOURCE_VOLUME_SKIP` (0), `_CLEAR` (1) or `_MULTIPLY` (2).
    pub fn oracle_source_volume_step_value(which: i32) -> u32;
    /// `sizeof(enum obs_source_volume_op)`.
    pub fn oracle_source_volume_step_size() -> u32;
}
