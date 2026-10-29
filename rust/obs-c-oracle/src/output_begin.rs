//! `oracle_*` build of `libobs/obs-output-begin.c.h`
//! (oracle/output_begin.c): the output begin-capture plan, renamed.

unsafe extern "C" {
    /// Undefined for an `fps_num` of 0: never call it with that.
    pub fn oracle_obs_output_begin_video_interval(
        fps_num: u32,
        fps_den: u32,
        frame_rate_divisor: u32,
    ) -> u64;

    /// Undefined for a `sample_rate` of 0 and, on MSVC x64, for a quotient
    /// above 64 bits: never call it with those.
    pub fn oracle_obs_output_begin_audio_interval(sample_rate: u32, frame_size: usize) -> u64;

    /// Undefined for an interval of 0: never pass one.
    pub fn oracle_obs_output_begin_batch_size(intervals: *const u64, num: usize) -> usize;

    pub fn oracle_obs_output_begin_pair_video(video_active: bool) -> bool;

    pub fn oracle_obs_output_begin_pair_audio(audio_active: bool, paired_num: usize) -> bool;
}
