//! `oracle_*` build of `libobs/obs-view-api.c.h` (oracle/view.c), and the
//! controls of its test doubles: the hidden view accessors and the source
//! calls, which the Rust shims (`obs_core::ffi::view`) also call under
//! cargo test.

use core::ffi::{c_char, c_int, c_longlong, c_void};

/// One traced step: event, the source's name (NULL for none) and a number
/// (channel, view type or mix index).
pub type OracleViewTrace =
    Option<unsafe extern "C" fn(event: *const c_char, name: *const c_char, value: c_longlong)>;

/// `bool (*enum_proc)(void *, struct obs_video_info *)`.
pub type OracleEnumProc =
    Option<unsafe extern "C" fn(param: *mut c_void, ovi: *mut c_void) -> bool>;

/// A test source, passed as `obs_source_t *`.
#[repr(C)]
#[derive(Debug)]
pub struct OracleViewSource {
    pub name: *const c_char,
    pub removed: bool,
    pub dying: bool,
}

unsafe extern "C" {
    pub fn oracle_obs_view_init(view: *mut c_void, view_type: u32) -> bool;
    pub fn oracle_obs_view_create() -> *mut c_void;
    pub fn oracle_obs_view_free(view: *mut c_void);
    pub fn oracle_obs_view_destroy(view: *mut c_void);
    pub fn oracle_obs_view_get_source(view: *mut c_void, channel: u32) -> *mut c_void;
    pub fn oracle_obs_view_set_source(view: *mut c_void, channel: u32, source: *mut c_void);
    pub fn oracle_obs_view_render(view: *mut c_void);
    pub fn oracle_obs_view_add(view: *mut c_void) -> *mut c_void;
    pub fn oracle_obs_view_add2(view: *mut c_void, ovi: *mut c_void) -> *mut c_void;
    pub fn oracle_obs_view_remove(view: *mut c_void);
    pub fn oracle_obs_view_enum_video_info(
        view: *mut c_void,
        enum_proc: OracleEnumProc,
        param: *mut c_void,
    );

    pub fn oracle_view_set_trace(hook: OracleViewTrace);
    /// Traces the activation event still open on this thread, if any
    /// (`obs_source_activate()` / `obs_source_deactivate()` on a view test
    /// source; see oracle/view.c).
    pub fn oracle_view_flush();
    /// Clears the mixes; `fail_mutex` makes `obs_view_mutex_init` fail;
    /// `main_mix_info` is what `obs_view_main_mix_info` returns.
    pub fn oracle_view_reset(fail_mutex: bool, main_mix_info: *mut c_void);
    pub fn oracle_view_mix_num() -> usize;
    pub fn oracle_view_mix_at(i: usize) -> *mut c_void;
    pub fn oracle_view_channel_at(view: *mut c_void, channel: usize) -> *mut c_void;
    pub fn oracle_view_type_of(view: *mut c_void) -> c_int;
    pub fn obs_view_struct_size() -> usize;
}
