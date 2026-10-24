//! `oracle_*` build of `libobs/obs-source-ptm-gate.c.h`
//! (oracle/source_ptm_gate.c): the push-to-mute and push-to-talk gate,
//! renamed. The C's `bool` results are declared as `u8`, the byte the C
//! returns, so a parity test compares them without assuming they are 0 or 1.

unsafe extern "C" {
    /// `uint64_t obs_source_ptm_gate_stop_time(uint64_t os_time, uint64_t
    /// delay_ms)`.
    pub fn oracle_obs_source_ptm_gate_stop_time(os_time: u64, delay_ms: u64) -> u64;
    /// `bool obs_source_ptm_gate_held(uint64_t os_time, uint64_t
    /// stop_time)`.
    pub fn oracle_obs_source_ptm_gate_held(os_time: u64, stop_time: u64) -> u8;
    /// `bool obs_source_ptm_gate_mutes(bool enabled, bool active)`.
    pub fn oracle_obs_source_ptm_gate_mutes(enabled: bool, active: bool) -> u8;
    /// `bool obs_source_ptm_gate_talk_mutes(bool enabled, bool active)`.
    pub fn oracle_obs_source_ptm_gate_talk_mutes(enabled: bool, active: bool) -> u8;
    /// `OBS_SOURCE_PTM_GATE_NS_PER_MS`.
    pub fn oracle_source_ptm_gate_ns_per_ms() -> u64;
}
