//! `oracle_*` build of `libobs/obs-source-transition-restore.c.h`
//! (oracle/source_transition_restore.c): `obs_transition_load()`, renamed,
//! over the transition seam (`libobs/obs-source-transition-seam.c.h`)
//! compiled under its own names and the obs-data port, which the Rust shim
//! calls too. Its size and matrix recalculation is the sizing oracle's
//! ([`crate::source_transition_size`]), whose stand-ins give each child's
//! size. The load's calls that find, link or release a source go to the
//! recording stand-ins `oracle_restore_*`, which the parity test hands the
//! Rust shim as well: a source is an opaque pointer with a name, a
//! reference count, an active-child link count and whether the add refuses
//! it, and every call is traced. Sources cross as opaque pointers.
//!
//! The graph is process-wide: a test holds its own lock around every use.

use core::ffi::{c_char, c_int, c_void};

/// A traced call's kind: `obs_get_source_by_name` (its source is the one
/// it found, or NULL).
pub const BY_NAME: c_int = 1;
/// `obs_source_add_active_child` (its result is whether it worked).
pub const ADD: c_int = 2;
/// `obs_source_remove_active_child`.
pub const REMOVE: c_int = 3;
/// `obs_source_release`.
pub const RELEASE: c_int = 4;

unsafe extern "C" {
    /// Forgets every test source and the trace.
    pub fn oracle_restore_reset();
    /// Adds a test source with a name (NULL for none), its references and
    /// active-child links and whether the add refuses it; false once the
    /// graph holds 8.
    pub fn oracle_restore_add_source(
        source: *mut c_void,
        name: *const c_char,
        refs: i64,
        links: i64,
        refuses: bool,
    ) -> bool;
    /// The references held on a test source, -1 for another pointer.
    pub fn oracle_restore_refs(source: *const c_void) -> i64;
    /// The active-child links held on a test source, -1 for another pointer.
    pub fn oracle_restore_links(source: *const c_void) -> i64;
    /// How many calls the trace holds (at most 64).
    pub fn oracle_restore_traced() -> usize;
    /// The trace's call `i`: its kind, the source it names or returned, the
    /// parent of an add or remove, and an add's result.
    pub fn oracle_restore_event(
        i: usize,
        kind: *mut c_int,
        source: *mut *mut c_void,
        parent: *mut *mut c_void,
        result: *mut c_int,
    );

    /// The stand-in for `obs_get_source_by_name`: the first test source with
    /// the name, with a reference added.
    pub fn oracle_restore_get_source_by_name(name: *const c_char) -> *mut c_void;
    /// The stand-in for `obs_source_add_active_child`.
    pub fn oracle_restore_add_active_child(parent: *mut c_void, child: *mut c_void) -> bool;
    /// The stand-in for `obs_source_remove_active_child`.
    pub fn oracle_restore_remove_active_child(parent: *mut c_void, child: *mut c_void);
    /// The stand-in for `obs_source_release`.
    pub fn oracle_restore_release(source: *mut c_void);

    /// `obs_transition_load`.
    pub fn oracle_obs_transition_load(tr: *mut c_void, data: *mut c_void);
}

/// The transition seam's accessors this oracle's tests read on top of the
/// sizing's ([`crate::source_transition_size::seam`]).
pub mod seam {
    use core::ffi::c_void;

    unsafe extern "C" {
        pub fn obs_source_transition_seam_get_source_active(
            transition: *const c_void,
            idx: usize,
        ) -> bool;
        pub fn obs_source_transition_seam_set_source_active(
            transition: *mut c_void,
            idx: usize,
            value: bool,
        );
    }
}
