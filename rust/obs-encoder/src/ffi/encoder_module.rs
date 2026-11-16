//! Private C layout and borrowed C-string boundary for encoder module queries.
use crate::encoder_module;
use core::ffi::{CStr, c_char, c_int, c_void};
use obs_util::module_registry::{Query, Registry};

unsafe extern "C" {
    fn obs_encoder_module_root(disabled: bool) -> *mut c_void;
    fn obs_encoder_module_next(module: *mut c_void) -> *mut c_void;
    fn obs_encoder_module_count(module: *mut c_void) -> usize;
    fn obs_encoder_module_id(module: *mut c_void, index: usize) -> *const c_char;
    fn obs_encoder_module_state(module: *mut c_void) -> c_int;
}

// Constructed only by the unsafe ABI queries. The original C does not read the query
// at all when no registered ID exists, even for a non-NULL pointer.
struct LazyId(*const c_char);
impl Query for LazyId {
    fn matches(&self, candidate: &[u8]) -> bool {
        // SAFETY: the query contract requires a live C string once a comparison
        // is reached. No pointer access occurs for lists containing only empty nodes.
        candidate == unsafe { CStr::from_ptr(self.0) }.to_bytes()
    }
}

struct Modules;
impl Registry for Modules {
    type Handle = core::ptr::NonNull<c_void>;
    fn first(&self, disabled: bool) -> Option<Self::Handle> {
        // SAFETY: the public caller guarantees a live, externally synchronized core.
        core::ptr::NonNull::new(unsafe { obs_encoder_module_root(disabled) })
    }
    fn next(&self, module: Self::Handle) -> Option<Self::Handle> {
        // SAFETY: handles originate in the live module lists.
        core::ptr::NonNull::new(unsafe { obs_encoder_module_next(module.as_ptr()) })
    }
    fn id_count(&self, module: Self::Handle) -> usize {
        // SAFETY: a live non-NULL module; no ID storage is read.
        unsafe { obs_encoder_module_count(module.as_ptr()) }
    }
    fn id(&self, module: Self::Handle, index: usize) -> &[u8] {
        // SAFETY: the helper bounds index by count; registered strings remain live.
        unsafe { CStr::from_ptr(obs_encoder_module_id(module.as_ptr(), index)) }.to_bytes()
    }
    fn load_state(&self, module: Self::Handle) -> i32 {
        // SAFETY: a live non-NULL module, still borrowed from the core.
        unsafe { obs_encoder_module_state(module.as_ptr()) }
    }
}

/// # Safety
/// The core is live and synchronized. Once a registered ID is compared, id must
/// be a live non-NULL C string. Empty ID lists leave the query pointer unread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_get_module(id: *const c_char) -> *mut c_void {
    encoder_module::get_module(&Modules, LazyId(id))
        .map_or(core::ptr::null_mut(), core::ptr::NonNull::as_ptr)
}

/// # Safety
/// The core is live and synchronized. Once a registered ID is compared, id must
/// be a live non-NULL C string. Empty ID lists leave the query pointer unread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_load_state(id: *const c_char) -> c_int {
    encoder_module::load_state(&Modules, LazyId(id))
}
