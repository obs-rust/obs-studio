//! `oracle_*` builds of `libobs/obs-source-transition-access.c.h`
//! (oracle/source_transition_access.c): the getters and manual controls of
//! a transition, renamed. They read and write the test sources of the slots
//! oracle (`crate::source_transition_slots`, zeroed `struct obs_source` with
//! a type, a name and both transition mutexes) directly, as the C does; the
//! Rust shims reach the same fields through the transition seam. The C's
//! `obs_source_get_ref()` is the slots oracle's recording stand-in
//! (`oracle_slots_get_ref`), and its `obs_transition_set()` is
//! [`oracle_access_set`], which records each call; the parity test hands
//! the Rust shims both. Sources cross as opaque pointers.
//!
//! The graph and the record are process-wide: a test holds the slots
//! oracle's `crate::source_transition_slots::graph_lock` around every use,
//! the same lock the slots parity suite holds.

use core::ffi::{c_int, c_void};

unsafe extern "C" {
    /// The stand-in for `obs_transition_set`: records the transition, the
    /// source and whether the transition's mutex was held (at most 16).
    pub fn oracle_access_set(transition: *mut c_void, source: *mut c_void);
    /// How many `obs_transition_set` calls were recorded.
    pub fn oracle_access_sets() -> usize;
    /// The recorded call `i`: the transition, the source and whether the
    /// mutex was held.
    pub fn oracle_access_set_call(
        i: usize,
        transition: *mut *mut c_void,
        source: *mut *mut c_void,
        locked: *mut bool,
    );
    /// Forgets the recorded calls.
    pub fn oracle_access_reset();

    /// `obs_transition_get_source`.
    pub fn oracle_obs_transition_get_source(transition: *mut c_void, target: c_int) -> *mut c_void;
    /// `obs_transition_get_active_source`.
    pub fn oracle_obs_transition_get_active_source(transition: *mut c_void) -> *mut c_void;
    /// `obs_transition_set_manual_torque`.
    pub fn oracle_obs_transition_set_manual_torque(
        transition: *mut c_void,
        torque: f32,
        clamp: f32,
    );
    /// `obs_transition_set_manual_time`.
    pub fn oracle_obs_transition_set_manual_time(transition: *mut c_void, t: f32);
}

/// The transition seam's manual clamp accessors
/// (`libobs/obs-source-transition-seam.h`), compiled in the oracle under
/// their own names; the rest are in `crate::source_transition_slots::seam`.
pub mod seam {
    use core::ffi::c_void;

    unsafe extern "C" {
        pub fn obs_source_transition_seam_get_manual_clamp(transition: *const c_void) -> f32;
        pub fn obs_source_transition_seam_set_manual_clamp(transition: *mut c_void, value: f32);
    }
}
