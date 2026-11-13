//! The original C shutdown detachment
//! (`libobs/obs-shutdown-detach-reference.c.h`) and the production bridge's
//! calls (`libobs/obs-shutdown-detach.c.h`), over a real private UUID source
//! table. The source-table mutex, the libobs source and scene calls and the
//! destruction queue wait go to the hooks a test installs for its thread.
use core::ffi::c_void;

/// `obs_scene_enum_items()`'s callback.
pub type ItemCallback = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> bool;

/// `struct oracle_shutdown_detach_hooks`: sources are table indices, scenes
/// are their sources' indices and items are the model's tokens.
#[repr(C)]
pub struct Hooks {
    pub model: *mut c_void,
    pub lock: unsafe extern "C" fn(*mut c_void),
    pub unlock: unsafe extern "C" fn(*mut c_void),
    pub get_ref: unsafe extern "C" fn(*mut c_void, usize) -> bool,
    pub scene_from_source: unsafe extern "C" fn(*mut c_void, usize) -> bool,
    pub group_from_source: unsafe extern "C" fn(*mut c_void, usize) -> bool,
    /// The model, the scene's index and handle, the callback and its param:
    /// the hook calls the callback with the handle and each item's token.
    pub enum_items:
        unsafe extern "C" fn(*mut c_void, usize, *mut c_void, ItemCallback, *mut c_void),
    pub item_remove: unsafe extern "C" fn(*mut c_void, usize),
    pub release: unsafe extern "C" fn(*mut c_void, usize),
    pub wait: unsafe extern "C" fn(*mut c_void) -> bool,
}

unsafe extern "C" {
    /// A core whose UUID table holds `count` sources, inserted in `order`
    /// (each index once); NULL when out of memory.
    pub fn oracle_shutdown_detach_new(count: usize, order: *const usize) -> *mut c_void;
    pub fn oracle_shutdown_detach_free(fixture: *mut c_void);
    /// The fixture core's `struct obs_core_data`.
    pub fn oracle_shutdown_detach_data(fixture: *mut c_void) -> *mut c_void;
    /// The production bridge's `struct obs_shutdown_detach_calls`.
    pub fn oracle_shutdown_detach_calls() -> *const c_void;
    /// Route this thread's stand-ins to `hooks` over `fixture`; NULL clears.
    pub fn oracle_shutdown_detach_install(fixture: *mut c_void, hooks: *const Hooks);
    /// The original detachment over the fixture.
    pub fn oracle_shutdown_detach_original(fixture: *mut c_void);
}
