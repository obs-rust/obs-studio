//! `oracle_*` builds of `libobs/obs-source-transition-slots.c.h`
//! (oracle/source_transition_slots.c): the child slots of a transition,
//! renamed, over the transition seam (`libobs/obs-source-transition-seam.c.h`)
//! compiled under its own names, which the Rust shims call too. The slots'
//! calls into the rest of libobs go to the recording stand-ins
//! `oracle_slots_*`, which the parity test hands the Rust shims as well: one
//! graph of test sources (zeroed `struct obs_source` with a type, a name and
//! both transition mutexes), each with a reference count, an active-child
//! link count per parent and child, and a trace of every call. Sources cross
//! as opaque pointers, the enums as `int`.
//!
//! The graph is process-wide, and the getters and manual controls oracle
//! (`crate::source_transition_access`) builds its test sources on it too:
//! every test that touches it holds [`graph_lock`] around every use, from
//! making the sources to [`oracle_slots_reset`], which frees them.

use core::ffi::{c_char, c_int, c_void};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The one lock on the process-wide graph, shared by every parity suite
/// that builds test sources on it (a reset frees every source, so two
/// suites holding different locks could free each other's).
pub fn graph_lock() -> MutexGuard<'static, ()> {
    static GRAPH: Mutex<()> = Mutex::new(());
    GRAPH.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A traced call's kind: `obs_source_get_ref` (its result is whether it
/// gave the source).
pub const GET_REF: c_int = 1;
/// `obs_source_release`.
pub const RELEASE: c_int = 2;
/// `obs_source_add_active_child` (its result is whether it worked).
pub const ADD: c_int = 3;
/// `obs_source_remove_active_child`.
pub const REMOVE: c_int = 4;

/// The enum callback, `obs_source_enum_proc_t`.
pub type EnumProc = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void);

unsafe extern "C" {
    /// A test source of type `source_type` named `name`, with one reference;
    /// NULL once the graph holds 16. Freed by [`oracle_slots_reset`].
    pub fn oracle_slots_source_new(source_type: c_int, name: *const c_char) -> *mut c_void;
    /// Frees every test source and forgets the graph and the trace.
    pub fn oracle_slots_reset();
    /// The references held on a test source, -1 for another pointer.
    pub fn oracle_slots_refs(source: *const c_void) -> i64;
    /// Sets the references held on a test source.
    pub fn oracle_slots_set_refs(source: *mut c_void, refs: i64);
    /// The active-child links from `parent` to `child`.
    pub fn oracle_slots_link(parent: *const c_void, child: *const c_void) -> i64;
    /// Sets the active-child links from `parent` to `child`.
    pub fn oracle_slots_set_link(parent: *mut c_void, child: *mut c_void, count: i64);
    /// Makes the add of `child` to `parent` fail, as for a child that
    /// contains it.
    pub fn oracle_slots_refuse(parent: *mut c_void, child: *mut c_void);
    /// How many calls the trace holds (at most 128).
    pub fn oracle_slots_traced() -> usize;
    /// The trace's call `i`: its kind, the sources it names and its result.
    pub fn oracle_slots_event(
        i: usize,
        kind: *mut c_int,
        a: *mut *mut c_void,
        b: *mut *mut c_void,
        result: *mut c_int,
    );

    /// The stand-in for `obs_source_get_ref`.
    pub fn oracle_slots_get_ref(source: *mut c_void) -> *mut c_void;
    /// The stand-in for `obs_source_release`.
    pub fn oracle_slots_release(source: *mut c_void);
    /// The stand-in for `obs_source_add_active_child`.
    pub fn oracle_slots_add_active_child(parent: *mut c_void, child: *mut c_void) -> bool;
    /// The stand-in for `obs_source_remove_active_child`.
    pub fn oracle_slots_remove_active_child(parent: *mut c_void, child: *mut c_void);
    /// The stand-in for `obs_source_get_name`: the test source's name.
    pub fn oracle_slots_get_name(source: *const c_void) -> *const c_char;

    /// `obs_transition_clear`.
    pub fn oracle_obs_transition_clear(transition: *mut c_void);
    /// `obs_transition_enum_sources`.
    pub fn oracle_obs_transition_enum_sources(
        transition: *mut c_void,
        cb: EnumProc,
        param: *mut c_void,
    );
    /// `obs_transition_swap_begin`.
    pub fn oracle_obs_transition_swap_begin(tr_dest: *mut c_void, tr_source: *mut c_void);
    /// `obs_transition_swap_end`.
    pub fn oracle_obs_transition_swap_end(tr_dest: *mut c_void, tr_source: *mut c_void);
}

/// The transition seam's accessors (`libobs/obs-source-transition-seam.h`),
/// compiled in the oracle under their own names, for tests to set up and
/// read a test transition's slots, flags, clock, render targets and locks.
pub mod seam {
    use core::ffi::{c_int, c_void};

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
        pub fn obs_source_transition_seam_get_source_active(
            transition: *const c_void,
            idx: usize,
        ) -> bool;
        pub fn obs_source_transition_seam_set_source_active(
            transition: *mut c_void,
            idx: usize,
            value: bool,
        );
        pub fn obs_source_transition_seam_get_transitioning_video(
            transition: *const c_void,
        ) -> bool;
        pub fn obs_source_transition_seam_set_transitioning_video(
            transition: *mut c_void,
            value: bool,
        );
        pub fn obs_source_transition_seam_get_transitioning_audio(
            transition: *const c_void,
        ) -> bool;
        pub fn obs_source_transition_seam_set_transitioning_audio(
            transition: *mut c_void,
            value: bool,
        );
        pub fn obs_source_transition_seam_get_start_time(transition: *const c_void) -> u64;
        pub fn obs_source_transition_seam_set_start_time(transition: *mut c_void, value: u64);
        pub fn obs_source_transition_seam_get_duration(transition: *const c_void) -> u64;
        pub fn obs_source_transition_seam_set_duration(transition: *mut c_void, value: u64);
        pub fn obs_source_transition_seam_get_mode(transition: *const c_void) -> c_int;
        pub fn obs_source_transition_seam_set_mode(transition: *mut c_void, value: c_int);
        pub fn obs_source_transition_seam_get_manual_val(transition: *const c_void) -> f32;
        pub fn obs_source_transition_seam_set_manual_val(transition: *mut c_void, value: f32);
        pub fn obs_source_transition_seam_get_manual_target(transition: *const c_void) -> f32;
        pub fn obs_source_transition_seam_set_manual_target(transition: *mut c_void, value: f32);
        pub fn obs_source_transition_seam_get_manual_torque(transition: *const c_void) -> f32;
        pub fn obs_source_transition_seam_set_manual_torque(transition: *mut c_void, value: f32);
        pub fn obs_source_transition_seam_get_texrender(
            transition: *const c_void,
            idx: usize,
        ) -> *mut c_void;
        pub fn obs_source_transition_seam_set_texrender(
            transition: *mut c_void,
            idx: usize,
            value: *mut c_void,
        );
        pub fn obs_source_transition_seam_trylock(transition: *mut c_void) -> c_int;
        pub fn obs_source_transition_seam_unlock(transition: *mut c_void) -> c_int;
        pub fn obs_source_transition_seam_trylock_textures(transition: *mut c_void) -> c_int;
        pub fn obs_source_transition_seam_unlock_textures(transition: *mut c_void) -> c_int;
    }
}
