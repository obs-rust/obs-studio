//! `oracle_*` builds of the rest of `libobs/obs-source-transition.c.h`
//! (oracle/source_transition.c): the audio render, init and free of a
//! transition, renamed. They run on the slots oracle's graph
//! (`crate::source_transition_slots`), reading the fields and taking
//! `transition_mutex` directly, as the C does; the Rust shims reach the
//! same fields and lock through the transition seam. The stop helpers the C
//! calls are the stop oracle's C (`crate::source_transition_stop`). Its
//! other libobs calls are the slots oracle's `obs_source_get_ref()` and
//! `obs_source_release()` stand-ins and the stand-ins here
//! ([`oracle_audio_removed`], [`oracle_audio_timestamp`],
//! [`oracle_audio_get_mix`], the texrender create and destroy, and the
//! graphics context's enter and leave), each traced in the slots oracle's
//! trace in order; the core's graphics context is [`oracle_audio_graphics`].
//! [`oracle_audio_child`] gives a child its audio state and a block of its
//! own, and [`oracle_audio_mix_a`] and [`oracle_audio_mix_b`] are the
//! type's mix callbacks, which record their calls ([`oracle_audio_mix_record`]).
//! The parity test hands the Rust shims the same stand-ins. Sources cross as
//! opaque pointers.
//!
//! A test holds `crate::source_transition_slots::graph_lock` around every
//! use, and calls [`oracle_audio_reset`] with each graph.

use core::ffi::{c_int, c_void};

/// The trace kind of `obs_source_removed()`: its result is the flag.
pub const REMOVED: c_int = 20;
/// `obs_source_get_audio_timestamp()`: its result is whether the watched
/// transition's lock was held.
pub const TIMESTAMP: c_int = 21;
/// `obs_source_get_audio_mix()`: its result is whether the lock was held.
pub const GET_MIX: c_int = 22;
/// `gs_texrender_create()`: its result is the id handed out times 100,
/// plus the format times 10, plus the depth format.
pub const TEX_CREATE: c_int = 23;
/// `gs_texrender_destroy()`: its result is the texrender's id (0 NULL).
pub const TEX_DESTROY: c_int = 24;
/// `gs_enter_context()`: its result is 1 for the core's context, 0 NULL.
pub const ENTER: c_int = 25;
/// `gs_leave_context()`.
pub const LEAVE: c_int = 26;

unsafe extern "C" {
    /// `obs_transition_init`.
    pub fn oracle_obs_transition_init(transition: *mut c_void) -> bool;
    /// `obs_transition_free`.
    pub fn oracle_obs_transition_free(transition: *mut c_void);
    /// `obs_transition_audio_render`.
    pub fn oracle_obs_transition_audio_render(
        transition: *mut c_void,
        ts_out: *mut u64,
        audio: *mut c_void,
        mixers: u32,
        channels: usize,
        sample_rate: usize,
        mix_a: Option<unsafe extern "C" fn(*mut c_void, f32) -> f32>,
        mix_b: Option<unsafe extern "C" fn(*mut c_void, f32) -> f32>,
    ) -> bool;

    /// Frees the blocks and watches `watched` (whose lock the stand-ins
    /// record); the next texrender creates hand out the ids `create_a`,
    /// then `create_b` (1 or 2, 0 for NULL).
    pub fn oracle_audio_reset(watched: *mut c_void, create_a: c_int, create_b: c_int);
    /// Gives `child` its audio timestamp, pending, duplicated and removed
    /// flags, and a block of its own from `seed`.
    pub fn oracle_audio_child(
        child: *mut c_void,
        ts: u64,
        pending: bool,
        duplicated: bool,
        removed: bool,
        seed: u32,
    );
    /// A block of `TOTAL_AUDIO_SIZE` bytes from `seed`, freed by the next
    /// reset.
    pub fn oracle_audio_block_new(seed: u32) -> *mut f32;
    /// The texrender of id 1 or 2, NULL for 0.
    pub fn oracle_audio_texrender(id: c_int) -> *mut c_void;
    /// The core's graphics context.
    pub fn oracle_audio_graphics() -> *mut c_void;
    /// A mix callback's calls, the hash of the times it got (and its data),
    /// and how many calls ran with the watched transition's lock held.
    pub fn oracle_audio_mix_record(which: c_int, calls: *mut i64, hash: *mut u64, held: *mut i64);
    /// From now on each mix callback call rewrites the watched
    /// transition's clock, audio flag and context data, as another thread
    /// may once the render lets the lock go.
    pub fn oracle_audio_clobber(on: bool);
    /// The type's mix callback for slot A.
    pub fn oracle_audio_mix_a(data: *mut c_void, t: f32) -> f32;
    /// The type's mix callback for slot B.
    pub fn oracle_audio_mix_b(data: *mut c_void, t: f32) -> f32;
    /// The layout of `struct obs_source_audio_mix` (0 size, 1 align, 2 the
    /// offset of `output[1]`) and `struct audio_output_data` (3 size, 4
    /// align, 5 the offset of `data[1]`).
    pub fn oracle_audio_layout(what: c_int) -> usize;
    /// `GS_RGBA` (0), `GS_ZS_NONE` (1), `OBS_ALIGN_LEFT | OBS_ALIGN_TOP`
    /// (2), `MAX_AUDIO_MIXES` (3), `MAX_AUDIO_CHANNELS` (4),
    /// `AUDIO_OUTPUT_FRAMES` (5), `TOTAL_AUDIO_SIZE` (6).
    pub fn oracle_audio_constant(what: c_int) -> c_int;
    /// Destroys (`make` false) or makes (true) both transition mutexes of
    /// a graph source.
    pub fn oracle_audio_mutexes(source: *mut c_void, make: bool);
    /// Reads and writes of the fields the seam has no accessor for: the
    /// alignment, and a child's audio timestamp.
    pub fn oracle_audio_alignment(transition: *mut c_void) -> u32;

    /// The stand-in for `obs_source_removed()`.
    pub fn oracle_audio_removed(source: *const c_void) -> bool;
    /// The stand-in for `obs_source_get_audio_timestamp()`.
    pub fn oracle_audio_timestamp(source: *const c_void) -> u64;
    /// The stand-in for `obs_source_get_audio_mix()`.
    pub fn oracle_audio_get_mix(source: *const c_void, audio: *mut c_void);
    /// The stand-in for `gs_texrender_create()`.
    pub fn oracle_audio_texrender_create(format: c_int, zsformat: c_int) -> *mut c_void;
    /// The stand-in for `gs_texrender_destroy()`.
    pub fn oracle_audio_texrender_destroy(texrender: *mut c_void);
    /// The stand-in for `gs_enter_context()`.
    pub fn oracle_audio_enter_context(graphics: *mut c_void);
    /// The stand-in for `gs_leave_context()`.
    pub fn oracle_audio_leave_context();
}
