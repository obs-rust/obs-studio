//! `oracle_*` builds of `libobs/obs-source-transition-stop.c.h`
//! (oracle/source_transition_stop.c): the clock read and stop of a
//! transition, renamed. They run on the slots oracle's graph
//! (`crate::source_transition_slots`: zeroed `struct obs_source` test
//! sources with a type, a name and both transition mutexes, a reference
//! count each, active-child links and a trace), reading the fields and
//! taking `transition_mutex` directly, as the C does; the Rust shims reach
//! the same fields and lock through the transition seam. The C's libobs
//! calls are the slots oracle's `obs_source_release()` and
//! `obs_source_remove_active_child()` stand-ins, the switch oracle's
//! `obs_source_dosignal()` (`crate::source_transition_switch`), and
//! [`oracle_stop_video_time`] (the frame time [`oracle_stop_reset`] sets),
//! each traced in order; [`oracle_stop_plugin`] gives a test transition a
//! recording `transition_stop` callback. The frame time and the callback
//! trace whether the watched transition's `transition_mutex` was held (1)
//! or free (0). The parity test hands the Rust shims the same stand-ins.
//! Sources cross as opaque pointers.
//!
//! A test holds `crate::source_transition_slots::graph_lock` around every
//! use, and calls [`oracle_stop_reset`] with each graph.

use core::ffi::{c_int, c_void};

/// The trace kind of a read of the video frame time: its result is whether
/// the watched transition's lock was held.
pub const VIDEO_TIME: c_int = 12;
/// The plugin's `transition_stop`: its result is whether the watched
/// transition's lock was held.
pub const PLUGIN: c_int = 13;

unsafe extern "C" {
    /// `transition_calc_time`.
    pub fn oracle_transition_calc_time(transition: *mut c_void, ts: u64) -> f32;
    /// `transition_get_video_time`.
    pub fn oracle_transition_get_video_time(transition: *mut c_void) -> f32;
    /// `obs_transition_get_time`.
    pub fn oracle_obs_transition_get_time(transition: *mut c_void) -> f32;
    /// `transition_stop`.
    pub fn oracle_transition_stop(transition: *mut c_void);
    /// `transition_handle_stop`.
    pub fn oracle_transition_handle_stop(transition: *mut c_void);
    /// `obs_transition_force_stop`.
    pub fn oracle_obs_transition_force_stop(transition: *mut c_void);

    /// Watches `transition` (whose lock the stand-ins trace) and sets the
    /// video frame time.
    pub fn oracle_stop_reset(watched: *mut c_void, video_time: u64);
    /// Gives a test transition the recording `transition_stop` callback.
    pub fn oracle_stop_plugin(transition: *mut c_void);
    /// Locks `transition_mutex`, as the renders do around the helpers.
    pub fn oracle_stop_lock(transition: *mut c_void) -> c_int;
    /// Unlocks `transition_mutex`.
    pub fn oracle_stop_unlock(transition: *mut c_void) -> c_int;
    /// 1 when `transition_mutex` is held, 0 when it is free.
    pub fn oracle_stop_held(transition: *mut c_void) -> c_int;

    /// The stand-in for `obs_video_stats_frame_time(&obs->video.stats)`.
    pub fn oracle_stop_video_time() -> u64;
}
