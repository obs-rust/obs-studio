//! `oracle_*` build of `libobs/obs-module-lists.c.h`
//! (oracle/module_lists.c): the ten module list calls, renamed, with the
//! module list seam under its own names, over a stand-in core that is a
//! thread-local of the oracle (each thread has its own, or none).
//!
//! The core and modules are untyped pointers: a `struct obs_core *` and
//! `obs_module_t *`s the oracle makes and frees. `blog()` is the oracle's
//! recording stand-in, [`oracle_module_lists_blog`], on both sides.

use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    pub fn oracle_obs_log_loaded_modules();
    pub fn oracle_obs_module_add_source(module: *mut c_void, id: *const c_char);
    pub fn oracle_obs_module_add_output(module: *mut c_void, id: *const c_char);
    pub fn oracle_obs_module_add_encoder(module: *mut c_void, id: *const c_char);
    pub fn oracle_obs_module_add_service(module: *mut c_void, id: *const c_char);
    pub fn oracle_obs_find_module_file(module: *mut c_void, file: *const c_char) -> *mut c_char;
    pub fn oracle_obs_module_get_config_path(
        module: *mut c_void,
        file: *const c_char,
    ) -> *mut c_char;
    pub fn oracle_obs_add_module_path(bin: *const c_char, data: *const c_char);
    pub fn oracle_obs_add_safe_module(name: *const c_char);
    pub fn oracle_obs_add_disabled_module(name: *const c_char);

    /// Appends the line to the thread's log: its level, then `format` with
    /// each `%s` filled in (`(null)` for NULL).
    pub fn oracle_module_lists_blog(level: c_int, format: *const c_char, ...);
    /// Copies up to `cap` bytes of the thread's log to `out` and empties it.
    pub fn oracle_module_lists_take_log(out: *mut c_char, cap: usize) -> usize;

    /// A zeroed core with a copy of `config_path`.
    pub fn oracle_module_lists_core_new(config_path: *const c_char) -> *mut c_void;
    /// Makes `core` (null for none) the thread's core.
    pub fn oracle_module_lists_set_core(core: *mut c_void);
    /// Frees the core, its lists and their strings, not its modules.
    pub fn oracle_module_lists_core_free(core: *mut c_void);
    /// A zeroed module with copies of its strings; a `loaded` one goes at
    /// the front of the core's list of loaded modules.
    pub fn oracle_module_lists_module_new(
        core: *mut c_void,
        file: *const c_char,
        data_path: *const c_char,
        name: *const c_char,
        loaded: bool,
    ) -> *mut c_void;
    /// Frees the module, its lists and their strings.
    pub fn oracle_module_lists_module_free(module: *mut c_void);
    /// Every list of `core` and of the `count` modules, with their sizes
    /// and capacities, written to `out`.
    pub fn oracle_module_lists_state(
        core: *const c_void,
        modules: *const *mut c_void,
        count: usize,
        out: *mut c_char,
        cap: usize,
    ) -> usize;
    /// `struct obs_module_path`'s size, alignment and field offsets, the
    /// size of its type, `OBS_MODULE_TYPE_UNDEFINED` and `LOG_INFO`.
    pub fn oracle_module_lists_layout(layout: *mut usize);
}
