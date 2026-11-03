//! `oracle_*` builds of `libobs/obs-source-transition-size.c.h`
//! (oracle/source_transition_size.c): the size, child matrices and tick of
//! a transition, renamed, over the transition seam
//! (`libobs/obs-source-transition-seam.c.h`) compiled under its own names,
//! which the Rust shims call too. The file's calls that read a child or
//! touch a render target go to the recording stand-ins `oracle_size_*`,
//! which the parity test hands the Rust shims as well: a child is an opaque
//! pointer with a width and a height, a render target an opaque pointer,
//! and every call is traced. Sources cross as opaque pointers.
//!
//! The graph is process-wide: a test holds its own lock around every use.

use core::ffi::{c_char, c_int, c_void};

/// A traced call's kind: `obs_source_get_width`.
pub const WIDTH: c_int = 1;
/// `obs_source_get_height`.
pub const HEIGHT: c_int = 2;
/// `gs_texrender_reset`.
pub const RESET: c_int = 3;

unsafe extern "C" {
    /// A test transition named `name` (a zeroed `struct obs_source` with the
    /// transition type and both transition mutexes); NULL once the graph
    /// holds 8. Freed by [`oracle_size_reset`].
    pub fn oracle_size_transition_new(name: *const c_char) -> *mut c_void;
    /// Frees every test transition and forgets the children and the trace.
    pub fn oracle_size_reset();
    /// Sets the width and height a child reports, adding it when new; false
    /// once the graph knows 16.
    pub fn oracle_size_set_child(source: *mut c_void, cx: u32, cy: u32) -> bool;
    /// How many calls the trace holds (at most 128).
    pub fn oracle_size_traced() -> usize;
    /// The trace's call `i`: its kind and the pointer it was handed.
    pub fn oracle_size_event(i: usize, kind: *mut c_int, a: *mut *mut c_void);

    /// The stand-in for `obs_source_get_width`.
    pub fn oracle_size_get_width(source: *mut c_void) -> u32;
    /// The stand-in for `obs_source_get_height`.
    pub fn oracle_size_get_height(source: *mut c_void) -> u32;
    /// The stand-in for `gs_texrender_reset`.
    pub fn oracle_size_texrender_reset(texrender: *mut c_void);

    /// `transition_get_cx`.
    pub fn oracle_transition_get_cx(tr: *mut c_void) -> u32;
    /// `transition_get_cy`.
    pub fn oracle_transition_get_cy(tr: *mut c_void) -> u32;
    /// `transition_recalculate_matrices`.
    pub fn oracle_transition_recalculate_matrices(transition: *mut c_void);
    /// `transition_recalculate_size`.
    pub fn oracle_transition_recalculate_size(transition: *mut c_void);
    /// `obs_transition_tick`.
    pub fn oracle_obs_transition_tick(transition: *mut c_void, t: f32);
}

/// `struct matrix4`, as the seam copies it.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Matrix4(pub [f32; 16]);

/// The transition seam's accessors (`libobs/obs-source-transition-seam.h`),
/// compiled in the oracle under their own names, for tests to set up and
/// read a test transition's slots, sizes, scale, alignment, manual fields,
/// render targets, matrices and locks.
pub mod seam {
    use core::ffi::{c_int, c_void};

    use super::Matrix4;

    unsafe extern "C" {
        pub fn obs_source_transition_seam_get_sources(
            transition: *const c_void,
            idx: usize,
        ) -> *mut c_void;
        pub fn obs_source_transition_seam_set_sources(
            transition: *mut c_void,
            idx: usize,
            value: *mut c_void,
        );
        pub fn obs_source_transition_seam_get_cx(transition: *const c_void) -> u32;
        pub fn obs_source_transition_seam_set_cx(transition: *mut c_void, value: u32);
        pub fn obs_source_transition_seam_get_cy(transition: *const c_void) -> u32;
        pub fn obs_source_transition_seam_set_cy(transition: *mut c_void, value: u32);
        pub fn obs_source_transition_seam_get_actual_cx(transition: *const c_void) -> u32;
        pub fn obs_source_transition_seam_set_actual_cx(transition: *mut c_void, value: u32);
        pub fn obs_source_transition_seam_get_actual_cy(transition: *const c_void) -> u32;
        pub fn obs_source_transition_seam_set_actual_cy(transition: *mut c_void, value: u32);
        pub fn obs_source_transition_seam_get_scale_type(transition: *const c_void) -> c_int;
        pub fn obs_source_transition_seam_set_scale_type(transition: *mut c_void, value: c_int);
        pub fn obs_source_transition_seam_get_alignment(transition: *const c_void) -> u32;
        pub fn obs_source_transition_seam_set_alignment(transition: *mut c_void, value: u32);
        pub fn obs_source_transition_seam_get_mode(transition: *const c_void) -> c_int;
        pub fn obs_source_transition_seam_set_mode(transition: *mut c_void, value: c_int);
        pub fn obs_source_transition_seam_get_manual_val(transition: *const c_void) -> f32;
        pub fn obs_source_transition_seam_set_manual_val(transition: *mut c_void, value: f32);
        pub fn obs_source_transition_seam_get_manual_target(transition: *const c_void) -> f32;
        pub fn obs_source_transition_seam_set_manual_target(transition: *mut c_void, value: f32);
        pub fn obs_source_transition_seam_get_manual_torque(transition: *const c_void) -> f32;
        pub fn obs_source_transition_seam_set_manual_torque(transition: *mut c_void, value: f32);
        pub fn obs_source_transition_seam_get_manual_clamp(transition: *const c_void) -> f32;
        pub fn obs_source_transition_seam_set_manual_clamp(transition: *mut c_void, value: f32);
        pub fn obs_source_transition_seam_get_texrender(
            transition: *const c_void,
            idx: usize,
        ) -> *mut c_void;
        pub fn obs_source_transition_seam_set_texrender(
            transition: *mut c_void,
            idx: usize,
            value: *mut c_void,
        );
        pub fn obs_source_transition_seam_get_matrix(
            transition: *const c_void,
            idx: usize,
            out: *mut Matrix4,
        );
        pub fn obs_source_transition_seam_set_matrix(
            transition: *mut c_void,
            idx: usize,
            value: *const Matrix4,
        );
        pub fn obs_source_transition_seam_lock_textures(transition: *mut c_void) -> c_int;
        pub fn obs_source_transition_seam_trylock(transition: *mut c_void) -> c_int;
        pub fn obs_source_transition_seam_unlock(transition: *mut c_void) -> c_int;
        pub fn obs_source_transition_seam_trylock_textures(transition: *mut c_void) -> c_int;
        pub fn obs_source_transition_seam_unlock_textures(transition: *mut c_void) -> c_int;
    }
}
