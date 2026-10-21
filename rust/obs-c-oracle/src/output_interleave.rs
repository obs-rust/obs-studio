//! `oracle_*` build of `libobs/obs-output-interleave.c.h`
//! (oracle/output_interleave.c).
//!
//! The output, packets and packet times are untyped pointers: callers lay
//! them out themselves (the layout getters below say how the C sees them),
//! and define the accessors and libobs calls the file makes, which keep
//! their libobs names, plus its clock, `oracle_output_interleave_clock_ns`.
//! The `oracle_output_interleave_*` functions wrap the file's static ones.

use core::ffi::{c_int, c_void};

/// Index of each value in `oracle_output_interleave_value`, in the C's
/// order.
pub const VALUE_NAMES: [&str; 9] = [
    "OBS_ENCODER_AUDIO",
    "OBS_ENCODER_VIDEO",
    "KEYFRAME_TRACK_STATUS_NOT_SEEN",
    "KEYFRAME_TRACK_STATUS_SEEN",
    "KEYFRAME_TRACK_STATUS_SKIPPED",
    "LOG_WARNING",
    "LOG_DEBUG",
    "MAX_OUTPUT_VIDEO_ENCODERS",
    "MAX_OUTPUT_AUDIO_ENCODERS",
];

/// Index of each entry in `oracle_output_interleave_layout`, in the C's
/// order.
pub const LAYOUT_NAMES: [&str; 67] = [
    "sizeof(struct keyframe_group_data)",
    "alignof(struct keyframe_group_data)",
    "offsetof(struct keyframe_group_data, group_id)",
    "offsetof(struct keyframe_group_data, pts)",
    "offsetof(struct keyframe_group_data, required_tracks)",
    "offsetof(struct keyframe_group_data, seen_on_track)",
    "sizeof(enum keyframe_group_track_status)",
    "sizeof(struct packet_callback)",
    "alignof(struct packet_callback)",
    "offsetof(struct packet_callback, packet_cb)",
    "offsetof(struct packet_callback, param)",
    "sizeof(struct caption_text)",
    "alignof(struct caption_text)",
    "offsetof(struct caption_text, display_duration)",
    "offsetof(struct caption_text, next)",
    "offsetof(struct caption_text, text)",
    "sizeof(struct darray)",
    "alignof(struct darray)",
    "offsetof(struct darray, array)",
    "offsetof(struct darray, num)",
    "offsetof(struct darray, capacity)",
    "sizeof(obs_encoder_packet_array_t)",
    "sizeof(obs_encoder_packet_time_array_t)",
    "sizeof(obs_keyframe_group_array_t)",
    "sizeof(obs_packet_callback_array_t)",
    "sizeof(struct obs_output_interleave_fields)",
    "alignof(struct obs_output_interleave_fields)",
    "offsetof(struct obs_output_interleave_fields, interleaved_mutex)",
    "offsetof(struct obs_output_interleave_fields, interleaved_packets)",
    "offsetof(struct obs_output_interleave_fields, received_video)",
    "offsetof(struct obs_output_interleave_fields, received_audio)",
    "offsetof(struct obs_output_interleave_fields, video_offsets)",
    "offsetof(struct obs_output_interleave_fields, audio_offsets)",
    "offsetof(struct obs_output_interleave_fields, highest_audio_ts)",
    "offsetof(struct obs_output_interleave_fields, highest_video_ts)",
    "offsetof(struct obs_output_interleave_fields, video_encoders)",
    "offsetof(struct obs_output_interleave_fields, audio_encoders)",
    "offsetof(struct obs_output_interleave_fields, encoder_packet_times)",
    "offsetof(struct obs_output_interleave_fields, keyframe_group_tracking)",
    "offsetof(struct obs_output_interleave_fields, pkt_callbacks_mutex)",
    "offsetof(struct obs_output_interleave_fields, pkt_callbacks)",
    "offsetof(struct obs_output_interleave_fields, total_frames)",
    "offsetof(struct obs_output_interleave_fields, interleaver_max_batch_size)",
    "offsetof(struct obs_output_interleave_fields, active_delay_ns)",
    "offsetof(struct obs_output_interleave_fields, encoded_packet)",
    "offsetof(struct obs_output_interleave_fields, data)",
    "offsetof(struct obs_output_interleave_fields, name)",
    "sizeof(struct obs_output_caption_track_fields)",
    "alignof(struct obs_output_caption_track_fields)",
    "offsetof(struct obs_output_caption_track_fields, caption_mutex)",
    "offsetof(struct obs_output_caption_track_fields, caption_head)",
    "offsetof(struct obs_output_caption_track_fields, caption_timestamp)",
    "offsetof(struct obs_output_caption_track_fields, last_caption_timestamp)",
    "offsetof(struct obs_output_caption_track_fields, caption_data_size)",
    "sizeof(struct obs_encoder_interleave_info)",
    "alignof(struct obs_encoder_interleave_info)",
    "offsetof(struct obs_encoder_interleave_info, timebase_num)",
    "offsetof(struct obs_encoder_interleave_info, timebase_den)",
    "offsetof(struct obs_encoder_interleave_info, framesize)",
    "offsetof(struct obs_encoder_interleave_info, group)",
    "sizeof(struct encoder_packet)",
    "alignof(struct encoder_packet)",
    "sizeof(struct encoder_packet_time)",
    "alignof(struct encoder_packet_time)",
    "sizeof(int)",
    "sizeof(size_t)",
    "sizeof(bool)",
];

unsafe extern "C" {
    pub fn oracle_interleave_packets(
        data: *mut c_void,
        packet: *mut c_void,
        packet_time: *mut c_void,
    );
    pub fn oracle_get_encoder_index(output: *const c_void, packet: *mut c_void) -> usize;
    pub fn oracle_free_packets(output: *mut c_void);
    pub fn oracle_output_interleave_start_idx(output: *mut c_void) -> usize;
    pub fn oracle_output_interleave_prune_premature(output: *mut c_void) -> c_int;
    pub fn oracle_output_interleave_prune(output: *mut c_void) -> bool;
    pub fn oracle_output_interleave_initialize(output: *mut c_void) -> bool;
    pub fn oracle_output_interleave_resort(output: *mut c_void);
    pub fn oracle_output_interleave_discard_unused_audio(output: *mut c_void, dts_usec: i64);
    pub fn oracle_output_interleave_send(output: *mut c_void);
    pub fn oracle_output_interleave_check_group_keyframe(output: *mut c_void, packet: *mut c_void);
    pub fn oracle_output_interleave_count_streamable(output: *mut c_void) -> usize;
    pub fn oracle_output_interleave_apply_ept_offsets(output: *mut c_void);
    /// The value named `VALUE_NAMES[which]`; -1 out of range.
    pub fn oracle_output_interleave_value(which: usize) -> i64;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_output_interleave_layout(which: usize) -> usize;
}
