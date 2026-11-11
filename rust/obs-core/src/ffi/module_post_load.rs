//! Thin ABI bridge using the existing opaque module-list seam.
use super::module_lists::{Libobs, Module, Seam};
use crate::module_post_load::{PostLoad, post_load};
use core::ffi::c_void;

type Callback = unsafe extern "C" fn();
unsafe extern "C" {
    fn obs_host_post_load_modules();
    fn obs_module_post_load_callback(module: *mut c_void) -> Option<Callback>;
}

impl PostLoad for Seam<Libobs> {
    type Callback = Callback;
    fn host(&mut self) {
        // SAFETY: the export's caller serializes the existing OBS host hook.
        unsafe { obs_host_post_load_modules() };
    }
    fn callback(&self, module: Module) -> Option<Callback> {
        // SAFETY: Module is non-NULL and borrowed from the live list. No private
        // record reference or callback-field reference survives a callback.
        unsafe { obs_module_post_load_callback(module.as_ptr()) }
    }
    fn call(&mut self, callback: Callback) {
        // SAFETY: the adapter returned the live module's optional plugin hook.
        unsafe { callback() };
    }
}

/// Dispatch plugin post-load hooks in the existing loaded-list order.
///
/// # Safety
/// The OBS core must be live; loading and dispatch must be serialized. Visited
/// nodes and callbacks remain valid across calls, including callback mutations.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_post_load_modules() {
    // SAFETY: the caller guarantees the core and nodes live across each access.
    post_load(&mut unsafe { Seam::<Libobs>::new() });
}
