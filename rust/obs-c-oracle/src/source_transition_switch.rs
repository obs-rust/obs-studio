//! `oracle_*` builds of `libobs/obs-source-transition-switch.c.h`
//! (oracle/source_transition_switch.c): the start and set of a transition,
//! renamed. They run on the slots oracle's graph
//! (`crate::source_transition_slots`: zeroed `struct obs_source` test
//! sources with a type, a name and both transition mutexes, a reference
//! count each, active-child links and a trace), reading the fields directly
//! as the C does; the Rust shims reach the same fields through the
//! transition seam. The C's libobs calls are the slots oracle's recording
//! stand-ins and the ones here: [`oracle_switch_enum_full_tree`] (the trees
//! a test registers with [`oracle_switch_tree_add`]),
//! [`oracle_switch_dosignal`], [`oracle_switch_now`] (a clock stepping
//! 1000 ns a call) and the size and matrix recalculation, each traced in
//! order with the slots oracle's calls; the parity test hands the Rust
//! shims all of them. Sources cross as opaque pointers, the mode as `int`.
//!
//! A test holds `crate::source_transition_slots::graph_lock` around every
//! use, and calls [`oracle_switch_reset`] with each graph.

use core::ffi::{c_char, c_int, c_void};

/// The trace kind of a walk of the root's full tree.
pub const WALK: c_int = 5;
/// A signal: its result is 1 for the start's pair, 2 for the stop's.
pub const SIGNAL: c_int = 6;
/// A read of the clock.
pub const NOW: c_int = 7;
/// `transition_recalculate_size`.
pub const SIZE: c_int = 8;
/// `transition_recalculate_matrices`.
pub const MATRICES: c_int = 9;
/// The plugin's `transition_start`: its result is the re-entry it makes.
pub const PLUGIN: c_int = 10;
/// The walk visiting a parent and child.
pub const VISIT: c_int = 11;

/// A start the plugin re-enters with: `obs_transition_start`'s shape.
pub type StartFn = unsafe extern "C" fn(*mut c_void, c_int, u32, *mut c_void) -> bool;
/// A set the plugin re-enters with: `obs_transition_set`'s shape.
pub type SetFn = unsafe extern "C" fn(*mut c_void, *mut c_void);
/// `obs_source_enum_proc_t`.
pub type EnumProc = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void);

unsafe extern "C" {
    /// `obs_transition_start`.
    pub fn oracle_obs_transition_start(
        transition: *mut c_void,
        mode: c_int,
        duration_ms: u32,
        dest: *mut c_void,
    ) -> bool;
    /// `obs_transition_set`.
    pub fn oracle_obs_transition_set(transition: *mut c_void, source: *mut c_void);

    /// Forgets the trees, the re-entry and the clock.
    pub fn oracle_switch_reset();
    /// The walk of `root` visits `parent` and `child` (at most 16 pairs).
    pub fn oracle_switch_tree_add(root: *mut c_void, parent: *mut c_void, child: *mut c_void);
    /// Gives a test transition the recording `transition_start` callback.
    pub fn oracle_switch_plugin(transition: *mut c_void);
    /// Arms the plugin's one re-entry: a start (`kind` 1, through `start`)
    /// or a set (`kind` 2, through `set`) of `dest`.
    pub fn oracle_switch_reenter(
        kind: c_int,
        mode: c_int,
        duration_ms: u32,
        dest: *mut c_void,
        start: StartFn,
        set: SetFn,
    );

    /// The stand-in for `obs_source_enum_full_tree`.
    pub fn oracle_switch_enum_full_tree(
        source: *mut c_void,
        callback: EnumProc,
        param: *mut c_void,
    );
    /// The stand-in for `obs_source_dosignal`.
    pub fn oracle_switch_dosignal(
        source: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    );
    /// The stand-in for `os_gettime_ns`.
    pub fn oracle_switch_now() -> u64;
    /// The stand-in for `transition_recalculate_size`.
    pub fn oracle_switch_recalculate_size(transition: *mut c_void);
    /// The stand-in for `transition_recalculate_matrices`.
    pub fn oracle_switch_recalculate_matrices(transition: *mut c_void);
}
