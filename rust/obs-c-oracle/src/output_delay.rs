//! `oracle_*` build of `libobs/obs-output-delay.c.h`
//! (oracle/output_delay.c).
//!
//! The output, packets and packet times are untyped pointers: callers lay
//! them out themselves (the layout getters below say how the C sees them),
//! and define the accessors and libobs calls the file makes, which keep
//! their libobs names, plus its clock, `oracle_output_delay_clock_ns`.

use core::ffi::c_void;

/// Index of each value in `oracle_output_delay_value`, in the C's order.
pub const VALUE_NAMES: [&str; 11] = [
    "DELAY_MSG_PACKET",
    "DELAY_MSG_START",
    "DELAY_MSG_STOP",
    "OBS_OUTPUT_ENCODED",
    "OBS_OUTPUT_VIDEO",
    "OBS_OUTPUT_DELAY_PRESERVE",
    "LOG_WARNING",
    "LOG_DEBUG",
    "OBS_ENCODER_AUDIO",
    "OBS_ENCODER_VIDEO",
    "sizeof(long)",
];

/// Index of each entry in `oracle_output_delay_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 60] = [
    "sizeof(struct encoder_packet)",
    "alignof(struct encoder_packet)",
    "offsetof(struct encoder_packet, data)",
    "offsetof(struct encoder_packet, size)",
    "offsetof(struct encoder_packet, pts)",
    "offsetof(struct encoder_packet, dts)",
    "offsetof(struct encoder_packet, timebase_num)",
    "offsetof(struct encoder_packet, timebase_den)",
    "offsetof(struct encoder_packet, type)",
    "offsetof(struct encoder_packet, keyframe)",
    "offsetof(struct encoder_packet, dts_usec)",
    "offsetof(struct encoder_packet, sys_dts_usec)",
    "offsetof(struct encoder_packet, priority)",
    "offsetof(struct encoder_packet, drop_priority)",
    "offsetof(struct encoder_packet, track_idx)",
    "offsetof(struct encoder_packet, encoder)",
    "sizeof(struct encoder_packet_time)",
    "alignof(struct encoder_packet_time)",
    "offsetof(struct encoder_packet_time, pts)",
    "offsetof(struct encoder_packet_time, cts)",
    "offsetof(struct encoder_packet_time, fer)",
    "offsetof(struct encoder_packet_time, ferc)",
    "offsetof(struct encoder_packet_time, pir)",
    "sizeof(struct delay_data)",
    "alignof(struct delay_data)",
    "offsetof(struct delay_data, msg)",
    "offsetof(struct delay_data, ts)",
    "offsetof(struct delay_data, packet)",
    "offsetof(struct delay_data, packet_time_valid)",
    "offsetof(struct delay_data, packet_time)",
    "sizeof(struct obs_output_delay_fields)",
    "alignof(struct obs_output_delay_fields)",
    "offsetof(struct obs_output_delay_fields, delay_mutex)",
    "offsetof(struct obs_output_delay_fields, delay_data)",
    "offsetof(struct obs_output_delay_fields, active_delay_ns)",
    "offsetof(struct obs_output_delay_fields, delay_sec)",
    "offsetof(struct obs_output_delay_fields, delay_flags)",
    "offsetof(struct obs_output_delay_fields, delay_cur_flags)",
    "offsetof(struct obs_output_delay_fields, delay_restart_refs)",
    "offsetof(struct obs_output_delay_fields, delay_active)",
    "offsetof(struct obs_output_delay_fields, delay_capturing)",
    "offsetof(struct obs_output_delay_fields, delay_callback)",
    "offsetof(struct obs_output_delay_fields, starting)",
    "offsetof(struct obs_output_delay_fields, reconnecting)",
    "offsetof(struct obs_output_delay_fields, flags)",
    "offsetof(struct obs_output_delay_fields, name)",
    "sizeof(enum delay_msg)",
    "sizeof(enum obs_encoder_type)",
    "sizeof(bool)",
    "sizeof(long)",
    "alignof(long)",
    "sizeof(encoded_callback_t)",
    "sizeof(struct deque)",
    "alignof(struct deque)",
    "offsetof(struct deque, data)",
    "offsetof(struct deque, size)",
    "offsetof(struct deque, start_pos)",
    "offsetof(struct deque, end_pos)",
    "offsetof(struct deque, capacity)",
    "sizeof(pthread_mutex_t *)",
];

unsafe extern "C" {
    pub fn oracle_obs_output_set_delay(output: *mut c_void, delay_sec: u32, flags: u32);
    pub fn oracle_obs_output_get_delay(output: *const c_void) -> u32;
    pub fn oracle_obs_output_get_active_delay(output: *const c_void) -> u32;
    pub fn oracle_process_delay(data: *mut c_void, packet: *mut c_void, packet_time: *mut c_void);
    pub fn oracle_obs_output_delay_start(output: *mut c_void) -> bool;
    pub fn oracle_obs_output_delay_stop(output: *mut c_void);
    pub fn oracle_obs_output_cleanup_delay(output: *mut c_void);
    /// The value named `VALUE_NAMES[which]`; -1 out of range.
    pub fn oracle_output_delay_value(which: usize) -> i64;
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_output_delay_layout(which: usize) -> usize;
    /// A `bzalloc`ed, initialized `pthread_mutex_t` (null on failure).
    pub fn oracle_output_delay_mutex_create() -> *mut c_void;
    pub fn oracle_output_delay_mutex_destroy(mutex: *mut c_void);
    /// Whether some thread holds `mutex` (a failed trylock).
    pub fn oracle_output_delay_mutex_held(mutex: *mut c_void) -> bool;
}
