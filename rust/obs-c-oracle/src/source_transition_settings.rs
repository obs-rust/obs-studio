//! `oracle_*` builds of `libobs/obs-source-transition-settings.c.h`
//! (oracle/source_transition_settings.c): the settings of a transition,
//! renamed, over the transition seam (`libobs/obs-source-transition-seam.c.h`)
//! compiled under its own names, which the Rust shims call too. A test
//! transition is a zeroed `struct obs_source` with its type, its
//! `transition_mutex` and up to two children that have only a name. Sources
//! and `obs_data_t` cross as opaque pointers, the enums as `int`.

use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    /// A test transition of source type `source_type`. A child kind is 0
    /// for an empty slot, 1 for a child named by its `name`, 2 for a child
    /// whose `context.name` is NULL. Free it with
    /// [`oracle_transition_settings_free`].
    pub fn oracle_transition_settings_new(
        source_type: c_int,
        child_a: c_int,
        name_a: *const c_char,
        child_b: c_int,
        name_b: *const c_char,
    ) -> *mut c_void;
    /// Frees a test transition, its children and its mutex.
    pub fn oracle_transition_settings_free(transition: *mut c_void);

    /// `obs_transition_is_active`.
    pub fn oracle_obs_transition_is_active(transition: *mut c_void) -> bool;
    /// `obs_transition_set_scale_type`.
    pub fn oracle_obs_transition_set_scale_type(transition: *mut c_void, scale_type: c_int);
    /// `obs_transition_get_scale_type`.
    pub fn oracle_obs_transition_get_scale_type(transition: *const c_void) -> c_int;
    /// `obs_transition_set_alignment`.
    pub fn oracle_obs_transition_set_alignment(transition: *mut c_void, alignment: u32);
    /// `obs_transition_get_alignment`.
    pub fn oracle_obs_transition_get_alignment(transition: *const c_void) -> u32;
    /// `obs_transition_set_size`.
    pub fn oracle_obs_transition_set_size(transition: *mut c_void, cx: u32, cy: u32);
    /// `obs_transition_get_size`.
    pub fn oracle_obs_transition_get_size(transition: *const c_void, cx: *mut u32, cy: *mut u32);
    /// `obs_transition_save`.
    pub fn oracle_obs_transition_save(transition: *mut c_void, data: *mut c_void);
    /// `obs_transition_enable_fixed`.
    pub fn oracle_obs_transition_enable_fixed(transition: *mut c_void, enable: bool, duration: u32);
    /// `obs_transition_fixed`.
    pub fn oracle_obs_transition_fixed(transition: *mut c_void) -> bool;
}

/// The transition seam's accessors (`libobs/obs-source-transition-seam.h`),
/// compiled in the oracle under their own names, for tests to set up and
/// read a test transition's fields.
pub mod seam {
    use core::ffi::{c_int, c_void};

    unsafe extern "C" {
        pub fn obs_source_transition_seam_set_transitioning_video(
            transition: *mut c_void,
            value: bool,
        );
        pub fn obs_source_transition_seam_set_transitioning_audio(
            transition: *mut c_void,
            value: bool,
        );
        pub fn obs_source_transition_seam_get_mode(transition: *const c_void) -> c_int;
        pub fn obs_source_transition_seam_set_mode(transition: *mut c_void, value: c_int);
        pub fn obs_source_transition_seam_get_scale_type(transition: *const c_void) -> c_int;
        pub fn obs_source_transition_seam_set_scale_type(transition: *mut c_void, value: c_int);
        pub fn obs_source_transition_seam_get_alignment(transition: *const c_void) -> u32;
        pub fn obs_source_transition_seam_set_alignment(transition: *mut c_void, value: u32);
        pub fn obs_source_transition_seam_get_cx(transition: *const c_void) -> u32;
        pub fn obs_source_transition_seam_set_cx(transition: *mut c_void, value: u32);
        pub fn obs_source_transition_seam_get_cy(transition: *const c_void) -> u32;
        pub fn obs_source_transition_seam_set_cy(transition: *mut c_void, value: u32);
        pub fn obs_source_transition_seam_get_use_fixed_duration(transition: *const c_void)
        -> bool;
        pub fn obs_source_transition_seam_set_use_fixed_duration(
            transition: *mut c_void,
            value: bool,
        );
        pub fn obs_source_transition_seam_get_fixed_duration(transition: *const c_void) -> u32;
        pub fn obs_source_transition_seam_set_fixed_duration(transition: *mut c_void, value: u32);
        pub fn obs_source_transition_seam_trylock(transition: *mut c_void) -> c_int;
        pub fn obs_source_transition_seam_unlock(transition: *mut c_void) -> c_int;
    }
}
