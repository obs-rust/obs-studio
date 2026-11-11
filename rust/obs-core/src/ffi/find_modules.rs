//! Module discovery ABI and access to the existing C module-path seam.
use super::module_lists::obs_module_path;
use super::module_paths::{
    obs_find_module_callback2_t as Callback, obs_module_add_extension_t as Hook,
    obs_module_paths_find,
};
use crate::find_modules::{self, Paths};
use obs_util::ffi::{darray::darray, dstr::dstr};
use std::ffi::{c_char, c_int, c_void};
use std::marker::PhantomData;

/// Existing legacy public callback payload from `obs.h`.
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct obs_module_info {
    pub bin_path: *const c_char,
    pub data_path: *const c_char,
}

pub type LegacyCallback = Option<unsafe extern "C" fn(*mut c_void, *const obs_module_info)>;
pub type Find = unsafe extern "C" fn(
    *const c_char,
    *const c_char,
    c_int,
    *const c_char,
    Hook,
    Callback,
    *mut c_void,
);
pub type Extension = unsafe extern "C" fn() -> *const c_char;

/// Existing discovery dependencies, replaceable by callback-only oracle probes.
pub trait Calls {
    fn find() -> Find;
    fn extension() -> Extension;
    fn hook() -> Hook;
}

unsafe extern "C" {
    fn obs_module_lists_seam_has_core() -> bool;
    fn obs_module_lists_seam_module_paths() -> *mut darray;
    fn get_module_extension() -> *const c_char;
    fn obs_host_add_module_extension(path: *mut dstr);
}

enum Libobs {}
impl Calls for Libobs {
    #[inline(always)]
    fn find() -> Find {
        obs_module_paths_find
    }
    // Keep host-only references in the production traversal instantiation.
    // Standalone parity consumers instantiate Search with their own Calls.
    #[inline(always)]
    fn extension() -> Extension {
        get_module_extension
    }
    #[inline(always)]
    fn hook() -> Hook {
        Some(obs_host_add_module_extension)
    }
}

struct Search<C> {
    callback: Callback,
    param: *mut c_void,
    calls: PhantomData<C>,
}

impl<C: Calls> Paths for Search<C> {
    fn has_core(&self) -> bool {
        // SAFETY: the private seam checks the core without dereferencing NULL.
        unsafe { obs_module_lists_seam_has_core() }
    }
    fn count(&self) -> usize {
        // SAFETY: the core checked by find_with stays alive through traversal.
        unsafe { (*obs_module_lists_seam_module_paths()).num }
    }
    fn discover(&mut self, index: usize) {
        // SAFETY: index is in the live array. Copy only the fields needed by
        // discovery, retaining no array reference across a synchronous callback.
        unsafe {
            let paths = obs_module_lists_seam_module_paths();
            let path = (*paths).array.cast::<obs_module_path>().add(index);
            let (bin, data, kind) = ((*path).bin, (*path).data, (*path).module_type);
            C::find()(
                bin,
                data,
                kind,
                C::extension()(),
                C::hook(),
                self.callback,
                self.param,
            );
        }
    }
}

/// Run discovery with the existing core seam and the supplied host operations.
///
/// # Safety
/// The core and its paths remain valid during each access, including after
/// callbacks. Operations follow the libobs discovery contract and callback/param
/// are valid for synchronous calls. Callbacks may append paths.
pub unsafe fn find_with<C: Calls>(callback: Callback, param: *mut c_void) {
    find_modules::find(&mut Search::<C> {
        callback,
        param,
        calls: PhantomData,
    });
}

/// # Safety
/// Same core, path and callback lifetime contract as C `obs_find_modules2`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_find_modules2(callback: Callback, param: *mut c_void) {
    // SAFETY: the caller supplies the original C discovery contract.
    unsafe { find_with::<Libobs>(callback, param) };
}

/// Preserve the original ABI-compatible callback cast without copying info.
///
/// # Safety
/// Same contract as find_with, with a callback accepting the legacy prefix.
pub unsafe fn find_legacy_with<C: Calls>(callback: LegacyCallback, param: *mut c_void) {
    // SAFETY: both C function pointer types have the same calling convention
    // and pointer arguments. obs_module_info is the verified info2 ABI prefix.
    let callback = unsafe { std::mem::transmute::<LegacyCallback, Callback>(callback) };
    // SAFETY: the same caller-owned pointers and synchronous callback contract.
    unsafe { find_with::<C>(callback, param) };
}

/// # Safety
/// Same core, path and callback lifetime contract as C `obs_find_modules`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_find_modules(callback: LegacyCallback, param: *mut c_void) {
    // SAFETY: the caller supplies the original C discovery contract.
    unsafe { find_legacy_with::<Libobs>(callback, param) };
}
