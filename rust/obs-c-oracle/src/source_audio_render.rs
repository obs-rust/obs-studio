//! `oracle_*` build of `libobs/obs-source-audio-render.c.h`
//! (oracle/source_audio_render.c): the source audio render mix plan,
//! renamed. The C's `bool` results are declared as `u8`, the byte the C
//! returns, so a parity test compares them without assuming they are 0 or
//! 1; the enum is declared as `u32`, the width the C returns it in.

unsafe extern "C" {
    /// `enum obs_source_audio_render_route obs_source_audio_render_route(bool has_buffer, bool custom, bool has_data)`.
    pub fn oracle_obs_source_audio_render_route(
        has_buffer: bool,
        custom: bool,
        has_data: bool,
    ) -> u32;
    /// `bool obs_source_audio_render_custom_clear_before(uint32_t audio_mixers, uint32_t mixers, size_t mix)`.
    pub fn oracle_obs_source_audio_render_custom_clear_before(
        audio_mixers: u32,
        mixers: u32,
        mix: usize,
    ) -> u8;
    /// `uint64_t obs_source_audio_render_custom_ts(bool success, uint64_t ts)`.
    pub fn oracle_obs_source_audio_render_custom_ts(success: bool, ts: u64) -> u64;
    /// `bool obs_source_audio_render_custom_pending(bool success)`.
    pub fn oracle_obs_source_audio_render_custom_pending(success: bool) -> u8;
    /// `bool obs_source_audio_render_custom_unmixed(uint64_t audio_ts, uint32_t mixers)`.
    pub fn oracle_obs_source_audio_render_custom_unmixed(audio_ts: u64, mixers: u32) -> u8;
    /// `bool obs_source_audio_render_custom_in_tick(uint32_t mixers, size_t mix)`.
    pub fn oracle_obs_source_audio_render_custom_in_tick(mixers: u32, mix: usize) -> u8;
    /// `bool obs_source_audio_render_custom_clear_after(uint32_t audio_mixers, size_t mix)`.
    pub fn oracle_obs_source_audio_render_custom_clear_after(audio_mixers: u32, mix: usize) -> u8;
    /// `bool obs_source_audio_render_tick_unset(uint64_t audio_ts)`.
    pub fn oracle_obs_source_audio_render_tick_unset(audio_ts: u64) -> u8;
    /// `bool obs_source_audio_render_tick_short(size_t input_size, size_t size)`.
    pub fn oracle_obs_source_audio_render_tick_short(input_size: usize, size: usize) -> u8;
    /// `bool obs_source_audio_render_tick_stop(bool submix, size_t mix)`.
    pub fn oracle_obs_source_audio_render_tick_stop(submix: bool, mix: usize) -> u8;
    /// `bool obs_source_audio_render_tick_copy(bool submix, uint32_t audio_mixers, uint32_t mixers, size_t mix)`.
    pub fn oracle_obs_source_audio_render_tick_copy(
        submix: bool,
        audio_mixers: u32,
        mixers: u32,
        mix: usize,
    ) -> u8;
    /// `bool obs_source_audio_render_tick_volume(bool submix)`.
    pub fn oracle_obs_source_audio_render_tick_volume(submix: bool) -> u8;
    /// `bool obs_source_audio_render_tick_clear_main(uint32_t audio_mixers, uint32_t mixers)`.
    pub fn oracle_obs_source_audio_render_tick_clear_main(audio_mixers: u32, mixers: u32) -> u8;
    /// `OBS_SOURCE_AUDIO_RENDER_PENDING` (0), `_CUSTOM` (1) or `_TICK` (2).
    pub fn oracle_source_audio_render_route_value(which: i32) -> u32;
    /// `sizeof(enum obs_source_audio_render_route)`.
    pub fn oracle_source_audio_render_route_size() -> u32;
}
