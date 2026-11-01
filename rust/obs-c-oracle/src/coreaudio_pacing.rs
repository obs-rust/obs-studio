//! `oracle_*` build of `libobs/audio-monitoring/osx/coreaudio-pacing.c.h`
//! (oracle/coreaudio_pacing.c).

unsafe extern "C" {
    pub fn oracle_coreaudio_monitor_buffer_size(channels: u32, rate: u32) -> usize;
    pub fn oracle_coreaudio_monitor_wait_size(buffer_size: usize) -> usize;
    pub fn oracle_coreaudio_monitor_can_fill(queued: usize, buffer_size: usize) -> bool;
    pub fn oracle_coreaudio_monitor_needs_volume(vol: f32) -> bool;
    pub fn oracle_coreaudio_monitor_wait_over(queued: usize, wait_size: usize) -> bool;
    pub fn oracle_coreaudio_monitor_all_back(empty_bytes: usize, ptr_size: usize) -> bool;
    pub fn oracle_coreaudio_monitor_recover_failed_enqueue() -> bool;
    /// `COREAUDIO_MONITOR_BUFFERS`.
    pub fn oracle_coreaudio_monitor_buffers() -> usize;
}
