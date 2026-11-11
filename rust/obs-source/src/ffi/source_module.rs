//! Private C layout and borrowed C-string boundary for source module queries.
use crate::module_lookup;
use core::ffi::{CStr, c_char, c_int, c_void};
use obs_util::module_registry::Registry;

unsafe extern "C" {
    fn obs_source_module_root(disabled: bool) -> *mut c_void;
    fn obs_source_module_next(module: *mut c_void) -> *mut c_void;
    fn obs_source_module_count(module: *mut c_void) -> usize;
    fn obs_source_module_id(module: *mut c_void, index: usize) -> *const c_char;
    fn obs_source_module_state(module: *mut c_void) -> c_int;
}

struct Modules;
impl Registry for Modules {
    type Handle = core::ptr::NonNull<c_void>;
    fn first(&self, disabled: bool) -> Option<Self::Handle> {
        // SAFETY: the public caller guarantees a live, externally synchronized core.
        core::ptr::NonNull::new(unsafe { obs_source_module_root(disabled) })
    }
    fn next(&self, module: Self::Handle) -> Option<Self::Handle> {
        // SAFETY: handles originate in the live module lists.
        core::ptr::NonNull::new(unsafe { obs_source_module_next(module.as_ptr()) })
    }
    fn id_count(&self, module: Self::Handle) -> usize {
        // SAFETY: a live non-NULL module; no ID storage is read.
        unsafe { obs_source_module_count(module.as_ptr()) }
    }
    fn id(&self, module: Self::Handle, index: usize) -> &[u8] {
        // SAFETY: the helper bounds index by count; registered strings remain live.
        unsafe { CStr::from_ptr(obs_source_module_id(module.as_ptr(), index)) }.to_bytes()
    }
    fn load_state(&self, module: Self::Handle) -> i32 {
        // SAFETY: a live non-NULL module, still borrowed from the core.
        unsafe { obs_source_module_state(module.as_ptr()) }
    }
}

unsafe fn id_bytes<'a>(id: *const c_char) -> Option<&'a [u8]> {
    // SAFETY: non-NULL input is a live NUL-terminated string by the ABI contract.
    if id.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(id) }.to_bytes())
    }
}

/// # Safety
/// The core is live and synchronized; id is a live C string. NULL is characterized
/// only when no registered ID is compared by the original C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_get_module(id: *const c_char) -> *mut c_void {
    // SAFETY: forwarded public input contract.
    module_lookup::get_module(&Modules, unsafe { id_bytes(id) })
        .map_or(core::ptr::null_mut(), core::ptr::NonNull::as_ptr)
}

/// # Safety
/// A non-NULL id is a live C string; non-built-in lookups require a live core.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_load_state(id: *const c_char) -> c_int {
    // SAFETY: forwarded public input contract; NULL does not access the core.
    module_lookup::load_state(&Modules, unsafe { id_bytes(id) })
}
