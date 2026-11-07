//! Private module layout boundary, checked against obs-internal.h in Tier 3.
//!
//! Handles refer to fully initialized libobs-created modules; C retains their
//! ownership and synchronization. Callback fields are layout placeholders only
//! and are never invoked here. Registered service arrays contain initialized ids.
use crate::service_module_lookup::{self, Module};
use core::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub struct ModuleIds {
    pub array: *mut *mut c_char,
    pub num: usize,
    pub capacity: usize,
}
#[repr(C)]
pub struct ObsModule {
    pub mod_name: *mut c_char,
    pub file: *const c_char,
    pub bin_path: *mut c_char,
    pub data_path: *mut c_char,
    pub module: *mut c_void,
    pub loaded: bool,
    pub load_state: c_int,
    pub load: Option<unsafe extern "C" fn()>,
    pub unload: Option<unsafe extern "C" fn()>,
    pub post_load: Option<unsafe extern "C" fn()>,
    pub set_locale: Option<unsafe extern "C" fn()>,
    pub get_string: Option<unsafe extern "C" fn()>,
    pub free_locale: Option<unsafe extern "C" fn()>,
    pub ver: Option<unsafe extern "C" fn()>,
    pub set_pointer: Option<unsafe extern "C" fn()>,
    pub name: Option<unsafe extern "C" fn()>,
    pub description: Option<unsafe extern "C" fn()>,
    pub author: Option<unsafe extern "C" fn()>,
    pub metadata: *mut c_void,
    pub next: *mut ObsModule,
    pub sources: ModuleIds,
    pub outputs: ModuleIds,
    pub encoders: ModuleIds,
    pub services: ModuleIds,
    pub module_type: c_int,
}

unsafe extern "C" {
    // The only C seam exposes the two global roots, without selecting a list.
    fn obs_service_module_roots(enabled: *mut *mut ObsModule, disabled: *mut *mut ObsModule);
}

/// # Safety
/// libobs global state is initialized and externally synchronized.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_module_first(disabled: bool) -> *mut ObsModule {
    let mut enabled = core::ptr::null_mut();
    let mut unavailable = core::ptr::null_mut();
    // SAFETY: local output pointers; caller guarantees live libobs globals.
    unsafe { obs_service_module_roots(&mut enabled, &mut unavailable) };
    service_module_lookup::first(disabled, enabled, unavailable)
}

/// # Safety
/// `module` is a live, non-null module.
unsafe fn view(module: *mut ObsModule) -> Module<*mut ObsModule> {
    // SAFETY: caller supplies a live module. No service storage is read here.
    let module = unsafe { &*module };
    Module {
        next: module.next,
        service_count: module.services.num,
        load_state: module.load_state,
    }
}

/// # Safety
/// `module` is a live, non-null module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_module_next(module: *mut ObsModule) -> *mut ObsModule {
    // SAFETY: forwarded caller contract.
    unsafe { view(module) }.next()
}
/// # Safety
/// `module` is a live, non-null module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_module_service_count(module: *mut ObsModule) -> usize {
    // SAFETY: forwarded caller contract.
    unsafe { view(module) }.service_count()
}
/// # Safety
/// `module` is live and `i` indexes its service array.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_module_service_id(
    module: *mut ObsModule,
    i: usize,
) -> *const c_char {
    // SAFETY: forwarded caller contract.
    {
        // SAFETY: caller guarantees i indexes initialized service storage.
        // Only the prefix through i is needed, independently of services.num.
        let ids = unsafe { core::slice::from_raw_parts((*module).services.array, i + 1) };
        service_module_lookup::service_id(ids, i).cast_const()
    }
}
/// # Safety
/// `module` is a live, non-null module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_service_module_load_state(module: *mut ObsModule) -> c_int {
    // SAFETY: forwarded caller contract.
    unsafe { view(module) }.load_state()
}
